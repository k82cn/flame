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
use std::fs::{self, create_dir_all, File};
use std::future::Future;
use std::os::unix::process::CommandExt;
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
use crate::shims::{Shim, ShimPtr};
use common::apis::{ApplicationContext, SessionContext, TaskContext, TaskOutput, TaskResult};
use common::{trace::TraceFn, trace_fn, FlameError, FLAME_EXECUTOR_ID, FLAME_WORKING_DIRECTORY};

pub struct HostShim {
    session_context: Option<SessionContext>,
    client: InstanceClient<Channel>,
    child: tokio::process::Child,
    service_socket: String,
    working_directory: String,
}

const RUST_LOG: &str = "RUST_LOG";
const DEFAULT_SVC_LOG_LEVEL: &str = "info";

impl HostShim {
    pub async fn new_ptr(
        executor: &Executor,
        app: &ApplicationContext,
    ) -> Result<ShimPtr, FlameError> {
        trace_fn!("HostShim::new_ptr");

        let working_directory = format!("/tmp/flame/shim/{}", executor.id);
        let service_socket = format!("{working_directory}/fsi.sock");

        // Create executor working directory for shims.
        fs::create_dir_all(working_directory.clone())
            .map_err(|e| FlameError::Internal(format!("failed to create shim directory: {e}")))?;

        let command = app.command.clone().unwrap_or_default();
        let args = app.arguments.clone();
        let log_level = env::var(RUST_LOG).unwrap_or(String::from(DEFAULT_SVC_LOG_LEVEL));
        let mut envs = app.environments.clone();
        envs.insert(RUST_LOG.to_string(), log_level);
        envs.insert(FLAME_EXECUTOR_ID.to_string(), executor.id.clone());

        tracing::debug!(
            "Try to start service by command <{command}> with args <{args:?}> and envs <{envs:?}>"
        );

        // Spawn child process
        let mut cmd = tokio::process::Command::new(&command);

        let cur_dir = app
            .working_directory
            .clone()
            .unwrap_or(FLAME_WORKING_DIRECTORY.to_string());

        tracing::debug!("Current directory of application instance: {cur_dir}");

        let mut child = cmd
            .envs(envs)
            .args(args)
            .current_dir(cur_dir)
            .process_group(0)
            .spawn()
            .map_err(|e| {
                FlameError::InvalidConfig(format!(
                    "failed to start service by command <{command}>: {e}"
                ))
            })?;

        let service_id = child.id().unwrap_or_default();
        tracing::debug!("The service <{service_id}> was started, waiting for registering.");

        WaitForSvcSocketFuture::new(service_socket.clone()).await?;
        tracing::debug!("Try to connect to service <{service_id}> at <{service_socket}>");

        let channel = Endpoint::try_from("http://[::]:50051")
            .unwrap()
            .connect_with_connector({
                let service_addr = service_socket.clone();

                service_fn(move |_: Uri| {
                    let service_addr = service_addr.clone();
                    async move {
                        UnixStream::connect(service_addr)
                            .await
                            .map(TokioIo::new)
                            .map_err(std::io::Error::other)
                    }
                })
            })
            .await
            .map_err(|e| {
                FlameError::Network(format!("failed to connect to service <{service_id}>: {e}"))
            })?;

        let client = InstanceClient::new(channel);

        Ok(Arc::new(Mutex::new(Self {
            session_context: None,
            client,
            child,
            service_socket,
            working_directory,
        })))
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

        let _ = std::fs::remove_file(&self.service_socket);
        let _ = std::fs::remove_dir_all(&self.working_directory);

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

        let req = Request::new(rpc::SessionContext::from(ctx.clone()));
        tracing::debug!("req: {:?}", req);
        let resp = self.client.on_session_enter(req).await?;
        let output = resp.into_inner();
        if output.return_code != 0 {
            return Err(FlameError::Internal(output.message.unwrap_or_default()));
        }

        Ok(())
    }

    async fn on_task_invoke(&mut self, ctx: &TaskContext) -> Result<TaskResult, FlameError> {
        trace_fn!("HostShim::on_task_invoke");

        let req = Request::new(rpc::TaskContext::from(ctx.clone()));
        tracing::debug!("req: {:?}", req);
        let resp = self.client.on_task_invoke(req).await?;
        let output = resp.into_inner();

        Ok(output.into())
    }

    async fn on_session_leave(&mut self) -> Result<(), FlameError> {
        trace_fn!("HostShim::on_session_leave");

        let resp = self
            .client
            .on_session_leave(Request::new(EmptyRequest::default()))
            .await?;

        let output = resp.into_inner();
        if output.return_code != 0 {
            return Err(FlameError::Internal(output.message.unwrap_or_default()));
        }

        Ok(())
    }
}

struct WaitForSvcSocketFuture {
    path: String,
}

impl WaitForSvcSocketFuture {
    pub fn new(path: String) -> Self {
        Self { path }
    }
}

impl Future for WaitForSvcSocketFuture {
    type Output = Result<(), FlameError>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        if fs::exists(&self.path).unwrap_or(false) {
            Poll::Ready(Ok(()))
        } else {
            ctx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}
