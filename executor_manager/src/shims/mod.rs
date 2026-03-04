/*
Copyright 2023 The Flame Authors.
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

mod grpc_shim;
mod host_shim;
mod wasm_shim;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use self::host_shim::HostShim;
use self::wasm_shim::WasmShim;

use crate::executor::Executor;
use common::apis::{
    ApplicationContext, SessionContext, Shim as ShimType, TaskContext, TaskOutput, TaskResult,
};
use common::{FlameError, FLAME_WORKING_DIRECTORY};

pub type ShimPtr = Arc<Mutex<dyn Shim>>;

/// Represents the executor's working directory with cleanup management.
/// Directory structure:
///   top_dir/                     - Process working directory, stdout/stderr logs
///   top_dir/work/<app_name>/     - App-specific directory for tmp, cache
///   top_dir/work/<executor_id>.sock - Socket for gRPC communication
/// Cleanup:
///   - top_dir: cleaned up only if auto-generated
///   - app_dir: always cleaned up
///   - socket: always cleaned up
pub struct ExecutorWorkDir {
    /// Top-level working directory (process runs here)
    top_dir: PathBuf,
    /// Application working directory: top_dir/work/<app-name> (for logs, tmp, cache)
    app_dir: PathBuf,
    /// Socket path: top_dir/work/<executor_id>.sock
    socket: PathBuf,
    /// If true, top_dir was auto-generated and should be cleaned up on release.
    auto_dir: bool,
}

impl ExecutorWorkDir {
    /// Create an ExecutorWorkDir from application context and executor ID.
    pub fn new(app: &ApplicationContext, executor_id: &str) -> Result<Self, FlameError> {
        let (top_dir, auto_dir) = match &app.working_directory {
            Some(wd) if !wd.is_empty() => (Path::new(wd).to_path_buf(), false),
            _ => (
                env::current_dir()
                    .unwrap_or(Path::new(FLAME_WORKING_DIRECTORY).to_path_buf())
                    .join(executor_id),
                true,
            ),
        };

        let work_dir = top_dir.join("work");
        let app_dir = work_dir.join(&app.name);
        // Socket in work dir with executor_id ensures uniqueness per executor
        let socket = work_dir.join(format!("{}.sock", executor_id));

        // Create top_dir if auto-generated
        if auto_dir {
            fs::create_dir_all(&top_dir).map_err(|e| {
                FlameError::Internal(format!(
                    "failed to create top working directory {}: {e}",
                    top_dir.display()
                ))
            })?;
        }

        // Always create work dir (needed for socket)
        fs::create_dir_all(&work_dir).map_err(|e| {
            FlameError::Internal(format!(
                "failed to create work directory {}: {e}",
                work_dir.display()
            ))
        })?;

        // Always create app_dir (for logs, tmp, cache)
        fs::create_dir_all(&app_dir).map_err(|e| {
            FlameError::Internal(format!(
                "failed to create app working directory {}: {e}",
                app_dir.display()
            ))
        })?;

        Ok(Self {
            top_dir,
            app_dir,
            socket,
            auto_dir,
        })
    }

    /// Returns the application working directory path (for tmp, cache).
    pub fn app_dir(&self) -> &Path {
        &self.app_dir
    }

    /// Returns the directory where the process should run (always top_dir).
    pub fn process_dir(&self) -> &Path {
        &self.top_dir
    }

    /// Returns the socket path for gRPC communication.
    pub fn socket(&self) -> &Path {
        &self.socket
    }
}

impl Drop for ExecutorWorkDir {
    fn drop(&mut self) {
        // Always cleanup socket file
        if self.socket.exists() {
            if let Err(e) = fs::remove_file(&self.socket) {
                tracing::warn!(
                    "Failed to remove socket file {}: {}",
                    self.socket.display(),
                    e
                );
            } else {
                tracing::debug!("Removed socket file: {}", self.socket.display());
            }
        }

        // Always cleanup app_dir
        if self.app_dir.exists() {
            if let Err(e) = fs::remove_dir_all(&self.app_dir) {
                tracing::warn!(
                    "Failed to remove app working directory {}: {}",
                    self.app_dir.display(),
                    e
                );
            } else {
                tracing::debug!("Removed app working directory: {}", self.app_dir.display());
            }
        }

        // Cleanup top_dir only if auto-generated
        if self.auto_dir && self.top_dir.exists() {
            if let Err(e) = fs::remove_dir_all(&self.top_dir) {
                tracing::warn!(
                    "Failed to remove executor working directory {}: {}",
                    self.top_dir.display(),
                    e
                );
            } else {
                tracing::debug!(
                    "Removed executor working directory: {}",
                    self.top_dir.display()
                );
            }
        }
    }
}

pub async fn new(executor: &Executor, app: &ApplicationContext) -> Result<ShimPtr, FlameError> {
    match app.shim {
        ShimType::Wasm => Ok(WasmShim::new_ptr(executor, app).await?),
        ShimType::Host => Ok(HostShim::new_ptr(executor, app).await?),
        _ => Ok(HostShim::new_ptr(executor, app).await?),
    }
}

#[async_trait]
pub trait Shim: Send + Sync + 'static {
    async fn on_session_enter(&mut self, ctx: &SessionContext) -> Result<(), FlameError>;
    async fn on_task_invoke(&mut self, ctx: &TaskContext) -> Result<TaskResult, FlameError>;
    async fn on_session_leave(&mut self) -> Result<(), FlameError>;
}
