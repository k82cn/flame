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
use common::{trace::TraceFn, trace_fn, FlameError, FLAME_WORKING_DIRECTORY};

pub struct GrpcShim {
    client: Option<InstanceClient<Channel>>,
    working_directory: String,
    endpoint: String,
}

const RUST_LOG: &str = "RUST_LOG";
const DEFAULT_SVC_LOG_LEVEL: &str = "info";

impl GrpcShim {
    pub async fn new(executor: &Executor, app: &ApplicationContext) -> Result<Self, FlameError> {
        trace_fn!("GrpcShim::new");

        let working_directory = env::current_dir()
            .unwrap_or(Path::new(FLAME_WORKING_DIRECTORY).to_path_buf())
            .join(executor.id.as_str());
        let endpoint = working_directory.join("fsi.sock");

        // Create executor working directory for shims.
        fs::create_dir_all(working_directory.clone())
            .map_err(|e| FlameError::Internal(format!("failed to create shim directory: {e}")))?;

        Ok(Self {
            client: None,
            working_directory: working_directory.to_string_lossy().to_string(),
            endpoint: endpoint.to_string_lossy().to_string(),
        })
    }

    pub fn endpoint(&self) -> &str {
        self.endpoint.as_str()
    }

    pub async fn connect(&mut self) -> Result<(), FlameError> {
        trace_fn!("GrpcShim::connect");

        WaitForSvcSocketFuture::new(self.endpoint.clone()).await?;
        tracing::debug!("Try to connect to service at <{}>", self.endpoint);

        let channel = Endpoint::try_from("http://[::]:50051")
            .unwrap()
            .connect_with_connector({
                let service_addr = self.endpoint.clone();

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
                FlameError::Network(format!(
                    "failed to connect to service at <{}>: {e}",
                    self.endpoint
                ))
            })?;

        self.client = Some(InstanceClient::new(channel));

        Ok(())
    }
}

impl Drop for GrpcShim {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.endpoint);
        let _ = std::fs::remove_dir_all(&self.working_directory);
    }
}

#[async_trait]
impl Shim for GrpcShim {
    async fn on_session_enter(&mut self, ctx: &SessionContext) -> Result<(), FlameError> {
        trace_fn!("GrpcShim::on_session_enter");

        if let Some(ref mut client) = self.client {
            let req = Request::new(rpc::SessionContext::from(ctx.clone()));
            tracing::debug!("req: {:?}", req);
            let resp = client.on_session_enter(req).await?;
            let output = resp.into_inner();
            if output.return_code != 0 {
                return Err(FlameError::Internal(output.message.unwrap_or_default()));
            }
        } else {
            return Err(FlameError::Internal(format!(
                "no connection to service at <{}>",
                self.endpoint
            )));
        }

        Ok(())
    }

    async fn on_task_invoke(&mut self, ctx: &TaskContext) -> Result<TaskResult, FlameError> {
        trace_fn!("GrpcShim::on_task_invoke");

        if let Some(ref mut client) = self.client {
            let req = Request::new(rpc::TaskContext::from(ctx.clone()));
            tracing::debug!("req: {:?}", req);
            let resp = client.on_task_invoke(req).await?;
            let output = resp.into_inner();
            if output.return_code != 0 {
                return Err(FlameError::Internal(output.message.unwrap_or_default()));
            }

            return Ok(output.into());
        } else {
            return Err(FlameError::Internal(format!(
                "no connection to service at <{}>",
                self.endpoint
            )));
        }
    }

    async fn on_session_leave(&mut self) -> Result<(), FlameError> {
        trace_fn!("GrpcShim::on_session_leave");

        if let Some(ref mut client) = self.client {
            let req = Request::new(EmptyRequest::default());
            tracing::debug!("req: {:?}", req);
            let resp = client.on_session_leave(req).await?;
            let output = resp.into_inner();
            if output.return_code != 0 {
                return Err(FlameError::Internal(output.message.unwrap_or_default()));
            }
        } else {
            return Err(FlameError::Internal(format!(
                "no connection to service at <{}>",
                self.endpoint
            )));
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
