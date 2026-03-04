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

use std::fs;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};

use async_trait::async_trait;
use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use tonic::transport::Channel;
use tonic::transport::{Endpoint, Uri};
use tonic::Request;
use tower::service_fn;

use ::rpc::flame as rpc;
use rpc::instance_client::InstanceClient;
use rpc::EmptyRequest;

use crate::shims::{ExecutorWorkDir, Shim};
use common::apis::{SessionContext, TaskContext, TaskResult, TaskState};
use common::FlameError;
use stdng::{logs::TraceFn, trace_fn};

pub struct GrpcShim {
    client: Option<InstanceClient<Channel>>,
    endpoint: String,
}

impl GrpcShim {
    pub fn new(work_dir: &ExecutorWorkDir) -> Result<Self, FlameError> {
        trace_fn!("GrpcShim::new");

        Ok(Self {
            client: None,
            endpoint: work_dir.socket().to_string_lossy().to_string(),
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

    pub fn close(&mut self) {
        if self.client.take().is_some() {
            tracing::debug!("Closed gRPC connection to service at <{}>", self.endpoint);
        }
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

            // Convert rpc::TaskResult to TaskResult
            // The From trait handles return_code != 0 by setting TaskState::Failed
            let task_result: TaskResult = output.into();

            // Log error if task failed
            if task_result.state == TaskState::Failed {
                let error_msg = task_result.message.as_deref().unwrap_or("Task failed");
                tracing::error!("Task failed: {}", error_msg);
            }

            return Ok(task_result);
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
            let resp = client.on_session_leave(req).await?;
            tracing::debug!("on_session_leave response: {:?}", resp);
            let output = resp.into_inner();
            if output.return_code != 0 {
                tracing::error!("on_session_leave failed: {:?}", output);
                return Err(FlameError::Internal(output.message.unwrap_or_default()));
            }
        } else {
            tracing::error!("no connection to service at <{}>", self.endpoint);
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
