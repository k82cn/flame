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

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use self::host_shim::HostShim;
use self::wasm_shim::WasmShim;

use crate::executor::Executor;
use common::apis::{
    ApplicationContext, Event, EventOwner, SessionContext, Shim as ShimType, TaskContext,
    TaskOutput, TaskResult,
};
use common::FlameError;

pub type ShimPtr = Arc<Mutex<dyn Shim>>;

pub async fn new(executor: &Executor, app: &ApplicationContext) -> Result<ShimPtr, FlameError> {
    match app.shim {
        ShimType::Wasm => Ok(WasmShim::new_ptr(executor, app).await?),
        ShimType::Host => Ok(HostShim::new_ptr(executor, app).await?),
        _ => Ok(HostShim::new_ptr(executor, app).await?),
    }
}

#[async_trait]
pub trait EventHandler: Send + Sync + 'static {
    async fn on_event(&mut self, owner: EventOwner, event: Event) -> Result<(), FlameError>;
}

type EventHandlerPtr = Arc<Mutex<dyn EventHandler>>;

#[async_trait]
pub trait Shim: Send + Sync + 'static {
    async fn on_session_enter(&mut self, ctx: &SessionContext) -> Result<(), FlameError>;
    async fn on_task_invoke(&mut self, ctx: &TaskContext) -> Result<TaskResult, FlameError>;
    async fn on_session_leave(&mut self) -> Result<(), FlameError>;

    async fn watch_event(&mut self, event_handler: EventHandlerPtr) -> Result<(), FlameError>;
}
