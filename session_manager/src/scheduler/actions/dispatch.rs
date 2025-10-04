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
use stdng::collections::BinaryHeap;

use crate::model::{
    ExecutorInfoPtr, IDLE_EXECUTOR, OPEN_SESSION, RELEASING_EXECUTOR, VOID_EXECUTOR,
};
use crate::scheduler::actions::{Action, ActionPtr};
use crate::scheduler::plugins::ssn_order_fn;
use crate::scheduler::Context;

use crate::FlameError;
use common::{trace::TraceFn, trace_fn};

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
        let mut releasing_executors = ss.find_executors(RELEASING_EXECUTOR)?;

        loop {
            if open_ssns.is_empty() {
                break;
            }

            let ssn = open_ssns.pop().unwrap();

            if !ctx.is_underused(&ssn)? {
                tracing::debug!("Session <{}> is not underused, skip it.", ssn.id);
                continue;
            }

            tracing::debug!(
                "Session <{}> is underused, start to allocate resources.",
                &ssn.id
            );

            // Allocate idle executors to underused sessions.
            let mut exec: Option<ExecutorInfoPtr> = None;
            for (_, e) in idle_executors.iter_mut() {
                if ctx.is_available(e, &ssn)? {
                    exec = Some(e.clone());
                    break;
                }
            }

            if let Some(exec) = exec {
                tracing::debug!("Bind executor <{}> for session <{}>.", exec.id, ssn.id);
                ctx.bind_session(&exec, &ssn).await?;
                idle_executors.remove(&exec.id);

                open_ssns.push(ssn);
                continue;
            }

            // Pipeline void executors to underused sessions.
            for (_, e) in void_executors.iter_mut() {
                if ctx.is_available(e, &ssn)? {
                    exec = Some(e.clone());
                    break;
                }
            }

            if let Some(exec) = exec {
                tracing::debug!("Pipeline executor <{}> for session <{}>.", exec.id, ssn.id);

                ctx.pipeline_session(&exec, &ssn).await?;
                void_executors.remove(&exec.id);

                open_ssns.push(ssn);
                continue;
            }
        }

        Ok(())
    }
}
