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

use crate::model::{ALL_NODE, OPEN_SESSION};

use crate::scheduler::actions::{Action, ActionPtr};
use crate::scheduler::plugins::node_order_fn;
use crate::scheduler::plugins::ssn_order_fn;
use crate::scheduler::Context;
use crate::FlameError;

use common::{trace::TraceFn, trace_fn};

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

        let mut nodes = BinaryHeap::new(node_order_fn(ctx));
        let node_list = ss.find_nodes(ALL_NODE)?;
        for node in node_list.values() {
            nodes.push(node.clone());
        }

        // Allocate executors for open sessions on nodes.
        loop {
            if open_ssns.is_empty() || nodes.is_empty() {
                break;
            }

            let ssn = open_ssns.pop().unwrap();
            let node = nodes.pop().unwrap();

            tracing::debug!(
                "Start to allocate resources for session <{}> on node <{}>",
                ssn.id,
                node.name
            );

            let is_underused = ctx.is_underused(&ssn)?;
            let is_allocatable = ctx.is_allocatable(&node, &ssn)?;

            match (is_underused, is_allocatable) {
                (true, true) => {
                    ctx.create_executor(&node, &ssn).await?;
                    nodes.push(node.clone());
                    open_ssns.push(ssn.clone());
                }
                (false, true) => {
                    nodes.push(node.clone());
                }
                (true, false) => {
                    open_ssns.push(ssn.clone());
                }
                (false, false) => {
                    tracing::debug!(
                        "Session <{}> is not underused and node <{}> is not allocatable, skip both.",
                        ssn.id,
                        node.name
                    );
                }
            }
        }

        Ok(())
    }
}
