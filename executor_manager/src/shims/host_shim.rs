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

pub struct HostShim {
    child: tokio::process::Child,
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

        let child = Self::launch_instance(app, executor, instance_client.endpoint())?;

        instance_client.connect().await?;

        Ok(Arc::new(Mutex::new(Self {
            child,
            instance_client,
        })))
    }

    fn launch_instance(
        app: &ApplicationContext,
        executor: &Executor,
        endpoint: &str,
    ) -> Result<tokio::process::Child, FlameError> {
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

        tracing::debug!(
            "Try to start service by command <{command}> with args <{args:?}> and envs <{envs:?}>"
        );

        // Spawn child process
        let mut cmd = tokio::process::Command::new(&command);

        // If application doesn't specify working_directory, use executor-specific directory
        let cur_dir = app.working_directory.clone().unwrap_or_else(|| {
            let executor_working_directory = env::current_dir()
                .unwrap_or(Path::new(FLAME_WORKING_DIRECTORY).to_path_buf())
                .join(executor.id.as_str());
            executor_working_directory.to_string_lossy().to_string()
        });

        tracing::debug!("Current directory of application instance: {cur_dir}");

        // Create the working directory if it doesn't exist
        create_dir_all(&cur_dir).map_err(|e| {
            FlameError::Internal(format!("failed to create working directory {cur_dir}: {e}"))
        })?;

        let log_file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(true)
            .open(format!("{cur_dir}/{}.log", executor.id))
            .map_err(|e| FlameError::Internal(format!("failed to open log file: {e}")))?;

        let mut child = cmd
            .envs(envs)
            .args(args)
            .current_dir(cur_dir)
            .stdout(Stdio::from(log_file.try_clone().map_err(|e| {
                FlameError::Internal(format!("failed to clone log file: {e}"))
            })?))
            .stderr(Stdio::from(log_file))
            .process_group(0)
            .spawn()
            .map_err(|e| {
                FlameError::InvalidConfig(format!(
                    "failed to start service by command <{command}>: {e}"
                ))
            })?;

        Ok(child)
    }
}

impl Drop for HostShim {
    fn drop(&mut self) {
        if let Some(id) = self.child.id() {
            let ig = Pid::from_raw(id as i32);
            killpg(ig, Signal::SIGTERM);
        } else {
            self.child.kill();
        }

        tracing::debug!(
            "The service <{}> was stopped",
            self.child.id().unwrap_or_default()
        );
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
