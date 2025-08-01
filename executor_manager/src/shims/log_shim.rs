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

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::shims::{Shim, ShimPtr};
use common::apis::{ApplicationContext, SessionContext, TaskContext, TaskOutput};
use common::{trace::TraceFn, trace_fn, FlameError};

#[derive(Clone)]
pub struct LogShim {
    session_context: Option<SessionContext>,
}

impl LogShim {
    pub fn new_ptr(_: &ApplicationContext) -> ShimPtr {
        trace_fn!("LogShim::new_ptr");

        Arc::new(Mutex::new(Self {
            session_context: None,
        }))
    }
}

#[async_trait]
impl Shim for LogShim {
    async fn on_session_enter(&mut self, ctx: &SessionContext) -> Result<(), FlameError> {
        trace_fn!("LogShim::on_session_enter");

        log::info!(
            "on_session_enter: Session: <{}>, Application: <{}>, Slots: <{}>",
            ctx.session_id,
            ctx.application.name,
            ctx.slots
        );
        self.session_context = Some(ctx.clone());

        Ok(())
    }

    async fn on_task_invoke(
        &mut self,
        ctx: &TaskContext,
    ) -> Result<Option<TaskOutput>, FlameError> {
        trace_fn!("LogShim::on_task_invoke");

        log::info!(
            "on_task_invoke: Task: <{}>, Session: <{}>",
            ctx.task_id,
            ctx.session_id
        );
        Ok(None)
    }

    async fn on_session_leave(&mut self) -> Result<(), FlameError> {
        trace_fn!("LogShim::on_session_leave");

        match &self.session_context {
            None => {
                log::info!("on_session_leave")
            }
            Some(ctx) => {
                log::info!(
                    "on_session_leave: Session: <{}>, Application: <{}>, Slots: <{}>",
                    ctx.session_id,
                    ctx.application.name,
                    ctx.slots
                );
            }
        }

        Ok(())
    }
}
