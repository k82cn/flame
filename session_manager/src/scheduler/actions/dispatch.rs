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

use stdng::collections::{BinaryHeap, Cmp};
use stdng::{logs::TraceFn, trace_fn};

use crate::model::{
    ExecutorInfoPtr, SessionInfoPtr, IDLE_EXECUTOR, OPEN_SESSION, UNBINDING_EXECUTOR, VOID_EXECUTOR,
};
use crate::scheduler::actions::{Action, ActionPtr};
use crate::scheduler::plugins::ssn_order_fn;
use crate::scheduler::statement::Statement;
use crate::scheduler::Context;

use crate::FlameError;

pub struct DispatchAction {}

impl DispatchAction {
    pub fn new_ptr() -> ActionPtr {
        Arc::new(DispatchAction {})
    }
}

#[async_trait::async_trait]
impl Action for DispatchAction {
    async fn execute(&self, ctx: &mut Context) -> Result<(), FlameError> {
        trace_fn!("DispatchAction::execute");
        let ss = ctx.snapshot.clone();

        ss.debug()?;

        let mut open_ssns = BinaryHeap::new(ssn_order_fn(ctx));
        let ssn_list = ss.find_sessions(OPEN_SESSION)?;
        for ssn in ssn_list.values() {
            open_ssns.push(ssn.clone());
        }

        let mut idle_executors = ss.find_executors(IDLE_EXECUTOR)?;
        let mut void_executors = ss.find_executors(VOID_EXECUTOR)?;
        let mut unbinding_executors = ss.find_executors(UNBINDING_EXECUTOR)?;

        tracing::debug!("Open sessions: <{:?}>", open_ssns.len());
        tracing::debug!("Idle executors: <{:?}>", idle_executors.len());
        tracing::debug!("Void executors: <{:?}>", void_executors.len());
        tracing::debug!("Unbinding executors: <{:?}>", unbinding_executors.len());

        loop {
            if open_ssns.is_empty() {
                break;
            }

            let ssn = open_ssns
                .pop()
                .expect("failed to pop open session: loop guard ensures non-empty");

            if !ctx.is_underused(&ssn)? {
                tracing::debug!("Session <{}> is not underused, skip it.", ssn.id);
                continue;
            }

            tracing::debug!(
                "Session <{}> is underused, start to allocate resources.",
                &ssn.id
            );

            let mut stmt = Statement::new(ss.clone(), ctx.plugins.clone(), ctx.controller.clone());

            for (_, exec) in idle_executors.iter() {
                if ctx.is_available(exec, &ssn)? {
                    stmt.bind(exec, &ssn)?;
                    break;
                }
            }

            if stmt.is_fulfilled(&ssn)? {
                tracing::debug!("Bind executor for session <{}>.", ssn.id);
                let bound_ids = stmt.commit().await?;
                for id in &bound_ids {
                    idle_executors.remove(id);
                }

                open_ssns.push(ssn);
                continue;
            } else if !stmt.is_empty() {
                tracing::debug!(
                    "Discarding unfulfilled binding for session <{}>: no available idle executors",
                    ssn.id
                );
                stmt.discard()?;
            }

            // Pipeline void/unbinding executors to underused sessions.
            // * For void executors, it means the executor is not registered; it'll be idle later.
            //   Pipeline it to the underused session to avoid over allocation.
            // * For unbinding executors, it means the executor is being unbound from a session.
            //   Pipeline it to the underused session to avoid over preemption.
            for exe_list in [&mut void_executors, &mut unbinding_executors] {
                let mut stmt =
                    Statement::new(ss.clone(), ctx.plugins.clone(), ctx.controller.clone());

                for (_, e) in exe_list.iter() {
                    if ctx.is_available(e, &ssn)? {
                        stmt.pipeline(e, &ssn)?;
                        if stmt.is_ready(&ssn)? {
                            break;
                        }
                    }
                }

                if stmt.is_ready(&ssn)? {
                    let pipelined_ids = stmt.commit().await?;
                    for id in &pipelined_ids {
                        tracing::debug!("Pipeline executor <{}> for session <{}>.", id, ssn.id);
                        exe_list.remove(id);
                    }
                    open_ssns.push(ssn.clone());
                    continue;
                } else if !stmt.is_empty() {
                    stmt.discard()?;
                }
            }
        }

        Ok(())
    }
}
