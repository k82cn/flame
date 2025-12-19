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

mod async_queue;

use std::{pin::Pin, sync::Arc};

use futures::Stream;
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::{transport::Server, Request, Response, Status};

use chrono::Utc;

use self::rpc::instance_server::{Instance, InstanceServer};
use crate::apis::flame as rpc;

use crate::apis::{CommonData, FlameError, TaskInput, TaskOutput};

const FLAME_INSTANCE_ENDPOINT: &str = "FLAME_INSTANCE_ENDPOINT";

pub struct ApplicationContext {
    pub name: String,
    pub image: Option<String>,
    pub command: Option<String>,
}

pub struct SessionContext {
    pub session_id: String,
    pub application: ApplicationContext,
    pub common_data: Option<CommonData>,

    event_queue: async_queue::AsyncQueue<rpc::WatchEventResponse>,
}

pub struct TaskContext {
    pub task_id: String,
    pub session_id: String,
    pub input: Option<TaskInput>,

    event_queue: async_queue::AsyncQueue<rpc::WatchEventResponse>,
}

#[tonic::async_trait]
pub trait FlameService: Send + Sync + 'static {
    async fn on_session_enter(&self, _: SessionContext) -> Result<(), FlameError>;
    async fn on_task_invoke(&self, _: TaskContext) -> Result<Option<TaskOutput>, FlameError>;
    async fn on_session_leave(&self) -> Result<(), FlameError>;
}

pub type FlameServicePtr = Arc<dyn FlameService>;

struct ShimService {
    service: FlameServicePtr,
    event_queue: async_queue::AsyncQueue<rpc::WatchEventResponse>,
}

#[tonic::async_trait]
impl Instance for ShimService {
    type WatchEventStream =
        Pin<Box<dyn Stream<Item = Result<rpc::WatchEventResponse, Status>> + Send>>;

    async fn on_session_enter(
        &self,
        req: Request<rpc::SessionContext>,
    ) -> Result<Response<rpc::Result>, Status> {
        tracing::debug!("ShimService::on_session_enter");

        let req = req.into_inner();
        let event_queue = self.event_queue.clone();
        let resp = self
            .service
            .on_session_enter(SessionContext::from((req, event_queue)))
            .await;

        match resp {
            Ok(_) => Ok(Response::new(rpc::Result {
                return_code: 0,
                message: None,
            })),
            Err(e) => Ok(Response::new(rpc::Result {
                return_code: -1,
                message: Some(e.to_string()),
            })),
        }
    }

    async fn on_task_invoke(
        &self,
        req: Request<rpc::TaskContext>,
    ) -> Result<Response<rpc::TaskResult>, Status> {
        tracing::debug!("ShimService::on_task_invoke");
        let req = req.into_inner();
        let event_queue = self.event_queue.clone();
        let resp = self
            .service
            .on_task_invoke(TaskContext::from((req, event_queue)))
            .await;

        match resp {
            Ok(data) => Ok(Response::new(rpc::TaskResult {
                return_code: 0,
                output: data.map(|d| d.into()),
                message: None,
            })),
            Err(e) => Ok(Response::new(rpc::TaskResult {
                return_code: -1,
                output: None,
                message: Some(e.to_string()),
            })),
        }
    }

    async fn on_session_leave(
        &self,
        _: Request<rpc::EmptyRequest>,
    ) -> Result<Response<rpc::Result>, Status> {
        tracing::debug!("ShimService::on_session_leave");
        let resp = self.service.on_session_leave().await;

        self.event_queue.close().await?;

        match resp {
            Ok(_) => Ok(Response::new(rpc::Result {
                return_code: 0,
                message: None,
            })),
            Err(e) => Ok(Response::new(rpc::Result {
                return_code: -1,
                message: Some(e.to_string()),
            })),
        }
    }

    async fn watch_event(
        &self,
        _: Request<rpc::EmptyRequest>,
    ) -> Result<Response<Self::WatchEventStream>, Status> {
        tracing::debug!("ShimService::watch_event");

        let (tx, rx) = mpsc::channel(128);

        let event_queue = self.event_queue.clone();

        tokio::spawn(async move {
            loop {
                let event = event_queue.pop_front().await;
                match event {
                    Some(event) => {
                        if let Err(e) = tx.send(Result::<_, Status>::Ok(event)).await {
                            tracing::debug!("Failed to send Event: {e}");
                            break;
                        }
                    }
                    None => {
                        break;
                    }
                }
            }
        });

        let output_stream = ReceiverStream::new(rx);
        Ok(Response::new(
            Box::pin(output_stream) as Self::WatchEventStream
        ))
    }
}

pub async fn run(service: impl FlameService) -> Result<(), Box<dyn std::error::Error>> {
    let shim_service = ShimService {
        service: Arc::new(service),
        event_queue: async_queue::AsyncQueue::new(),
    };

    let endpoint = std::env::var(FLAME_INSTANCE_ENDPOINT)
        .map_err(|_| FlameError::InvalidConfig("FLAME_INSTANCE_ENDPOINT not found".to_string()))?;

    let uds_stream = UnixListenerStream::new(UnixListener::bind(endpoint)?);

    Server::builder()
        .add_service(InstanceServer::new(shim_service))
        .serve_with_incoming(uds_stream)
        .await?;

    Ok(())
}

impl From<rpc::ApplicationContext> for ApplicationContext {
    fn from(ctx: rpc::ApplicationContext) -> Self {
        Self {
            name: ctx.name.clone(),
            image: ctx.image.clone(),
            command: ctx.command.clone(),
        }
    }
}

impl
    From<(
        rpc::SessionContext,
        async_queue::AsyncQueue<rpc::WatchEventResponse>,
    )> for SessionContext
{
    fn from(
        (ctx, event_queue): (
            rpc::SessionContext,
            async_queue::AsyncQueue<rpc::WatchEventResponse>,
        ),
    ) -> Self {
        SessionContext {
            session_id: ctx.session_id.clone(),
            application: ctx.application.map(ApplicationContext::from).unwrap(),
            common_data: ctx.common_data.map(|data| data.into()),
            event_queue,
        }
    }
}

impl
    From<(
        rpc::TaskContext,
        async_queue::AsyncQueue<rpc::WatchEventResponse>,
    )> for TaskContext
{
    fn from(
        (ctx, event_queue): (
            rpc::TaskContext,
            async_queue::AsyncQueue<rpc::WatchEventResponse>,
        ),
    ) -> Self {
        TaskContext {
            task_id: ctx.task_id.clone(),
            session_id: ctx.session_id.clone(),
            input: ctx.input.map(|data| data.into()),
            event_queue,
        }
    }
}

impl TaskContext {
    pub fn record_event(&self, code: i32, message: Option<String>) -> Result<(), FlameError> {
        self.event_queue.push_back(rpc::WatchEventResponse {
            owner: Some(rpc::EventOwner {
                session_id: self.session_id.clone(),
                task_id: Some(self.task_id.clone()),
            }),
            event: Some(rpc::Event {
                code,
                message,
                creation_time: Utc::now().timestamp(),
            }),
        })?;

        Ok(())
    }
}

impl SessionContext {
    pub fn record_event(&self, code: i32, message: Option<String>) -> Result<(), FlameError> {
        self.event_queue.push_back(rpc::WatchEventResponse {
            owner: Some(rpc::EventOwner {
                session_id: self.session_id.clone(),
                task_id: None,
            }),
            event: Some(rpc::Event {
                code,
                message,
                creation_time: Utc::now().timestamp(),
            }),
        })?;

        Ok(())
    }
}
