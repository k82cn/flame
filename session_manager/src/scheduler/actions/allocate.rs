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

use crate::model::{SessionInfoPtr, SnapShot, ALL_NODE, OPEN_SESSION};
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

        tracing::debug!(
            "AllocateAction: {} open sessions, {} nodes available",
            open_ssns.len(),
            nodes.len()
        );

        let node_order_fn = node_order_fn(ctx);

        // Allocate executors for open sessions on nodes.
        loop {
            if open_ssns.is_empty() {
                break;
            }

            let ssn = open_ssns.pop().unwrap();

            let is_underused = ctx.is_underused(&ssn)?;
            if !is_underused {
                tracing::debug!(
                    "Session <{}> is NOT underused (pending={:?}, running={:?}), skipping allocation",
                    ssn.id,
                    ssn.tasks_status.get(&common::apis::TaskState::Pending),
                    ssn.tasks_status.get(&common::apis::TaskState::Running)
                );
                continue;
            }

            tracing::debug!(
                "Session <{}> IS underused (pending={:?}, running={:?}), attempting allocation",
                ssn.id,
                ssn.tasks_status.get(&common::apis::TaskState::Pending),
                ssn.tasks_status.get(&common::apis::TaskState::Running)
            );

            // Explicit max_instances check (safety guard)
            // The fairshare plugin caches allocated count from snapshot at the start of the cycle.
            // Within a single cycle, if we allocate multiple executors, the cached count doesn't update.
            // To prevent over-allocation, we count actual executors from the current snapshot.
            if let Some(max_instances) = ssn.max_instances {
                let all_executors = ss.find_executors(None)?;
                let current_count = all_executors
                    .values()
                    .filter(|e| e.ssn_id.as_ref() == Some(&ssn.id))
                    .count();
                if current_count >= max_instances as usize {
                    tracing::debug!(
                        "Session <{}> has reached max_instances limit: {} >= {}",
                        ssn.id,
                        current_count,
                        max_instances
                    );
                    continue; // Already at max limit
                }
            }

            // If there're still some executors in pipeline, skip allocate new executor to the session.
            let pipelined_executors = ss.pipelined_executors(ssn.clone())?;
            if !pipelined_executors.is_empty() {
                tracing::debug!("Skip allocate resources for session <{}> because there are <{}> executors in pipeline.", ssn.id, pipelined_executors.len());
                continue;
            }

            let mut allocated = false;
            for node in nodes.iter() {
                tracing::debug!(
                    "Checking node <{}> for session <{}> allocation",
                    node.name,
                    ssn.id
                );

                let is_allocatable = ctx.is_allocatable(node, &ssn)?;
                if !is_allocatable {
                    tracing::debug!(
                        "Node <{}> is NOT allocatable for session <{}> (node may be at capacity)",
                        node.name,
                        ssn.id
                    );
                    continue;
                }

                tracing::info!(
                    "Allocating executor for session <{}> on node <{}>",
                    ssn.id,
                    node.name
                );

                ctx.create_executor(node, &ssn).await?;
                allocated = true;

                nodes.sort_by(|a, b| node_order_fn.cmp(a, b));
                open_ssns.push(ssn.clone());

                break;
            }

            if !allocated {
                tracing::warn!(
                    "Failed to allocate executor for underused session <{}>: no allocatable nodes found (total nodes: {})",
                    ssn.id,
                    nodes.len()
                );
            }
        }

        Ok(())
    }
}
