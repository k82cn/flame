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

use crate::model::{ALL_NODE, OPEN_SESSION};
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
