/*
Copyright 2025 The Flame Authors.
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at
    http://www.apache.org/licenses/LICENSE-2.0
Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

use std::env;
use std::fs::{self, create_dir_all, File, OpenOptions};
use std::future::Future;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::pin::Pin;
use std::process::{self, Command, Stdio};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use std::{thread, time};

use async_trait::async_trait;
use hyper_util::rt::TokioIo;
use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;
use std::collections::HashMap;
use stdng::{logs::TraceFn, trace_fn};
use tokio::net::UnixStream;
use tokio::sync::Mutex;
use tonic::transport::Channel;
use tonic::transport::{Endpoint, Uri};
use tonic::Request;
use tower::service_fn;

use ::rpc::flame as rpc;
use rpc::instance_client::InstanceClient;
use rpc::EmptyRequest;
use uuid::Uuid;

use crate::executor::Executor;
use crate::shims::grpc_shim::GrpcShim;
use crate::shims::{Shim, ShimPtr};
use common::apis::{ApplicationContext, SessionContext, TaskContext, TaskOutput, TaskResult};
use common::{FlameError, FLAME_CACHE_ENDPOINT, FLAME_INSTANCE_ENDPOINT, FLAME_WORKING_DIRECTORY};

struct HostInstance {
    child: tokio::process::Child,
    work_dir: std::path::PathBuf,
    socket_path: std::path::PathBuf,
}

impl HostInstance {
    fn new(
        child: tokio::process::Child,
        work_dir: std::path::PathBuf,
        socket_path: std::path::PathBuf,
    ) -> Self {
        Self {
            child,
            work_dir,
            socket_path,
        }
    }

    fn cleanup(&mut self) {
        // Kill the child process
        if let Some(id) = self.child.id() {
            let ig = Pid::from_raw(id as i32);
            killpg(ig, Signal::SIGTERM);
            tracing::debug!("Killed process group <{}>", id);
        } else {
            let _ = self.child.kill();
            tracing::debug!("Killed child process");
        }

        // Note: Working directory is preserved for debugging and log inspection
        // It can be cleaned up manually if needed

        // Cleanup socket file
        if self.socket_path.exists() {
            if let Err(e) = fs::remove_file(&self.socket_path) {
                tracing::warn!(
                    "Failed to remove socket file {}: {}",
                    self.socket_path.display(),
                    e
                );
            } else {
                tracing::debug!("Removed socket file: {}", self.socket_path.display());
            }
        }
    }
}

pub struct HostShim {
    instance: HostInstance,
    instance_client: GrpcShim,
}

const RUST_LOG: &str = "RUST_LOG";
const DEFAULT_SVC_LOG_LEVEL: &str = "info";

impl HostShim {
    pub async fn new_ptr(
        executor: &Executor,
        app: &ApplicationContext,
    ) -> Result<ShimPtr, FlameError> {
        trace_fn!("HostShim::new_ptr");

        let mut instance_client = GrpcShim::new(executor, app).await?;

        let instance = Self::launch_instance(app, executor, instance_client.endpoint())?;

        instance_client.connect().await?;

        Ok(Arc::new(Mutex::new(Self {
            instance,
            instance_client,
        })))
    }

    fn create_dir(path: &Path, name: &str) -> Result<(), FlameError> {
        create_dir_all(path).map_err(|e| {
            FlameError::Internal(format!(
                "failed to create {} directory {}: {e}",
                name,
                path.display()
            ))
        })
    }

    fn setup_working_directory(work_dir: &Path) -> Result<HashMap<String, String>, FlameError> {
        trace_fn!("HostShim::setup_working_directory");

        let tmp_dir = work_dir.join("tmp");
        let uv_cache_dir = work_dir.join(".uv");
        let pip_cache_dir = work_dir.join(".pip");

        tracing::debug!(
            "Working directory of application instance: {}",
            work_dir.display()
        );
        tracing::debug!(
            "Temporary directory of application instance: {}",
            tmp_dir.display()
        );
        tracing::debug!(
            "UV cache directory of application instance: {}",
            uv_cache_dir.display()
        );
        tracing::debug!(
            "PIP cache directory of application instance: {}",
            pip_cache_dir.display()
        );

        // Create the working, temporary, and cache directories if they don't exist
        Self::create_dir(&work_dir, "working")?;
        Self::create_dir(&tmp_dir, "temporary")?;
        Self::create_dir(&uv_cache_dir, "UV cache")?;
        Self::create_dir(&pip_cache_dir, "PIP cache")?;

        // Build environment variables for the application instance
        let mut envs = HashMap::new();
        envs.insert("TMPDIR".to_string(), tmp_dir.to_string_lossy().to_string());
        envs.insert("TEMP".to_string(), tmp_dir.to_string_lossy().to_string());
        envs.insert("TMP".to_string(), tmp_dir.to_string_lossy().to_string());
        envs.insert(
            "UV_CACHE_DIR".to_string(),
            uv_cache_dir.to_string_lossy().to_string(),
        );
        envs.insert(
            "PIP_CACHE_DIR".to_string(),
            pip_cache_dir.to_string_lossy().to_string(),
        );

        Ok(envs)
    }

    fn launch_instance(
        app: &ApplicationContext,
        executor: &Executor,
        endpoint: &str,
    ) -> Result<HostInstance, FlameError> {
        trace_fn!("HostShim::launch_instance");

        let command = app.command.clone().unwrap_or_default();
        let args = app.arguments.clone();
        let log_level = env::var(RUST_LOG).unwrap_or(String::from(DEFAULT_SVC_LOG_LEVEL));

        let mut envs = app.environments.clone();
        envs.insert(RUST_LOG.to_string(), log_level);
        envs.insert(FLAME_INSTANCE_ENDPOINT.to_string(), endpoint.to_string());
        if let Some(context) = &executor.context {
            if let Some(cache) = &context.cache {
                envs.insert(FLAME_CACHE_ENDPOINT.to_string(), cache.endpoint.clone());
            }
        }

        // Propagate HOME environment variable to ensure Python finds user site-packages
        // This is needed when flamepy is installed with --user flag for the flame user
        if let Ok(home) = env::var("HOME") {
            envs.entry("HOME".to_string()).or_insert(home);
        }

        tracing::debug!(
            "Try to start service by command <{command}> with args <{args:?}> and envs <{envs:?}>"
        );

        // Spawn child process
        let mut cmd = tokio::process::Command::new(&command);

        // If application doesn't specify working_directory, use executor manager's working directory with executor ID and application name
        let cur_dir = match app.working_directory.clone() {
            Some(wd) => Path::new(&wd).to_path_buf(),
            None => env::current_dir()
                .unwrap_or(Path::new(FLAME_WORKING_DIRECTORY).to_path_buf())
                .join(executor.id.as_str())
                .join("work")
                .join(&app.name),
        };

        let work_dir = cur_dir.clone();
        // Setup working directory and get environment overrides
        let wd_envs = Self::setup_working_directory(&work_dir)?;
        envs.extend(wd_envs);

        let log_out = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(true)
            .open(work_dir.join(format!("{}.out", executor.id)))
            .map_err(|e| FlameError::Internal(format!("failed to open stdout log file: {e}")))?;

        let log_err = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(true)
            .open(work_dir.join(format!("{}.err", executor.id)))
            .map_err(|e| FlameError::Internal(format!("failed to open stderr log file: {e}")))?;

        let child = cmd
            .envs(envs)
            .args(args)
            .current_dir(&work_dir)
            .stdout(Stdio::from(log_out))
            .stderr(Stdio::from(log_err))
            .process_group(0)
            .spawn()
            .map_err(|e| {
                FlameError::InvalidConfig(format!(
                    "failed to start service by command <{command}>: {e}"
                ))
            })?;

        let socket_path = Path::new(endpoint).to_path_buf();

        Ok(HostInstance::new(child, work_dir, socket_path))
    }
}

impl Drop for HostShim {
    fn drop(&mut self) {
        // Close gRPC connection first
        self.instance_client.close();
        // Then cleanup instance resources (kill process, remove files)
        self.instance.cleanup();
    }
}

#[async_trait]
impl Shim for HostShim {
    async fn on_session_enter(&mut self, ctx: &SessionContext) -> Result<(), FlameError> {
        trace_fn!("HostShim::on_session_enter");

        self.instance_client.on_session_enter(ctx).await
    }

    async fn on_task_invoke(&mut self, ctx: &TaskContext) -> Result<TaskResult, FlameError> {
        trace_fn!("HostShim::on_task_invoke");

        self.instance_client.on_task_invoke(ctx).await
    }

    async fn on_session_leave(&mut self) -> Result<(), FlameError> {
        trace_fn!("HostShim::on_session_leave");

        self.instance_client.on_session_leave().await
    }
}
