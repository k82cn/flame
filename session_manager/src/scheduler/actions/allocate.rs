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

use crate::model::{ALL_NODE, BINDING_EXECUTOR, IDLE_EXECUTOR, OPEN_SESSION, VOID_EXECUTOR, SessionInfoPtr, SnapShot};
use crate::scheduler::actions::{Action, ActionPtr};
use crate::scheduler::plugins::node_order_fn;
use crate::scheduler::plugins::ssn_order_fn;
use crate::scheduler::Context;

use common::FlameError;

pub struct AllocateAction {}

impl AllocateAction {
    pub fn new_ptr() -> ActionPtr {
        Arc::new(AllocateAction {})
    }
}

#[async_trait::async_trait]
impl Action for AllocateAction {
    async fn execute(&self, ctx: &mut Context) -> Result<(), FlameError> {
        trace_fn!("AllocateAction::execute");
        let ss = ctx.snapshot.clone();

        ss.debug()?;

        let mut open_ssns = BinaryHeap::new(ssn_order_fn(ctx));
        let ssn_list = ss.find_sessions(OPEN_SESSION)?;
        for ssn in ssn_list.values() {
            open_ssns.push(ssn.clone());
        }

        let mut nodes = vec![];
        let node_list = ss.find_nodes(ALL_NODE)?;
        for node in node_list.values() {
            nodes.push(node.clone());
        }

        let node_order_fn = node_order_fn(ctx);

        // Allocate executors for open sessions on nodes.
        loop {
            if open_ssns.is_empty() {
                break;
            }

            let ssn = open_ssns.pop().unwrap();

            if !ctx.is_underused(&ssn)? {
                continue;
            }

            // if there are some exectors could be allocated to a session (VOID, IDLE, BINDING)
            // skip allocate new executor to the session
            // TODO(jinzhejz): this is a temporary solution, schedule only allocate ONE executor to a session each time.
            let dispatch_done = self.is_dispatch_executor_done(ss.clone(), ssn.clone())?;
            if !dispatch_done {
                tracing::debug!(
                    "Skip allocate resources for session <{}> because there are some exectors could be allocated to it.", 
                    ssn.id
                );
                continue;
            }

            for node in nodes.iter() {
                tracing::debug!(
                    "Start to allocate resources for session <{}> on node <{}>",
                    ssn.id,
                    node.name
                );

                if !ctx.is_allocatable(node, &ssn)? {
                    continue;
                }

                ctx.create_executor(node, &ssn).await?;

                nodes.sort_by(|a, b| node_order_fn.cmp(a, b));
                open_ssns.push(ssn.clone());

                break;
            }
        }

        Ok(())
    }
}

impl AllocateAction {
    fn is_dispatch_executor_done(&self, ss: Arc<SnapShot>, ssn: SessionInfoPtr) -> Result<bool, FlameError> {
        let void_exec = ss.find_executors(VOID_EXECUTOR)?;
        for exec in void_exec.iter() {
           if ssn.slots == exec.1.slots {
                return Ok(false);
            }
        }

        let idle_execs = ss.find_executors(IDLE_EXECUTOR)?;
        for exec in idle_execs.iter() {
            if ssn.slots == exec.1.slots {
                return Ok(false);
            }
        }

        let binding_execs = ss.find_executors(BINDING_EXECUTOR)?;
        for exec in binding_execs.iter() {
            if ssn.slots == exec.1.slots {
                return Ok(false);
            }
        }

        Ok(true)
    }
}