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

use std::sync::Arc;

use crate::controller::ControllerPtr;
use crate::model::{
    ExecutorInfo, ExecutorInfoPtr, NodeInfoPtr, SessionInfoPtr, SnapShotPtr, ALL_NODE,
};
use crate::scheduler::plugins::PluginManagerPtr;
use common::apis::{ExecutorID, ExecutorState};
use common::FlameError;

struct PipelinedAllocation {
    node: NodeInfoPtr,
    ssn: SessionInfoPtr,
}

struct PipelinedBinding {
    executor: ExecutorInfoPtr,
    node: NodeInfoPtr,
    ssn: SessionInfoPtr,
}

pub struct Statement {
    allocations: Vec<PipelinedAllocation>,
    bindings: Vec<PipelinedBinding>,
    snapshot: SnapShotPtr,
    plugins: PluginManagerPtr,
    controller: ControllerPtr,
}

impl Statement {
    pub fn new(
        snapshot: SnapShotPtr,
        plugins: PluginManagerPtr,
        controller: ControllerPtr,
    ) -> Self {
        Statement {
            allocations: Vec::new(),
            bindings: Vec::new(),
            snapshot,
            plugins,
            controller,
        }
    }

    pub fn pipeline(&mut self, node: &NodeInfoPtr, ssn: &SessionInfoPtr) -> Result<(), FlameError> {
        self.plugins
            .on_pipeline_executor(node.clone(), ssn.clone())?;
        self.allocations.push(PipelinedAllocation {
            node: node.clone(),
            ssn: ssn.clone(),
        });
        Ok(())
    }

    pub fn bind(
        &mut self,
        executor: &ExecutorInfoPtr,
        ssn: &SessionInfoPtr,
    ) -> Result<(), FlameError> {
        let nodes = self.snapshot.find_nodes(ALL_NODE)?;
        let node = nodes.get(&executor.node).ok_or_else(|| {
            FlameError::Internal(format!(
                "Node {} not found for executor {}",
                executor.node, executor.id
            ))
        })?;

        self.plugins.on_bind_executor(node.clone(), ssn.clone())?;
        self.bindings.push(PipelinedBinding {
            executor: executor.clone(),
            node: node.clone(),
            ssn: ssn.clone(),
        });
        Ok(())
    }

    pub fn is_ready(&self, ssn: &SessionInfoPtr) -> Result<bool, FlameError> {
        self.plugins.is_ready(ssn)
    }

    pub async fn commit(self) -> Result<Vec<ExecutorID>, FlameError> {
        let mut bound_executor_ids = Vec::new();

        for op in self.allocations.into_iter() {
            let executor = self
                .controller
                .create_executor(op.node.name.clone(), op.ssn.id.clone())
                .await?;

            let exec_info = ExecutorInfo::from(&executor);
            self.snapshot.add_executor(Arc::new(exec_info))?;
            self.plugins.on_session_bind(op.ssn)?;
        }

        for binding in self.bindings.into_iter() {
            self.controller
                .bind_session(binding.executor.id.clone(), binding.ssn.id.clone())
                .await?;
            self.plugins.on_session_bind(binding.ssn.clone())?;
            self.snapshot
                .update_executor_state(binding.executor.clone(), ExecutorState::Binding)?;
            bound_executor_ids.push(binding.executor.id.clone());
        }

        Ok(bound_executor_ids)
    }

    pub fn discard(self) -> Result<(), FlameError> {
        for op in self.allocations.into_iter().rev() {
            self.plugins.on_discard_executor(op.node, op.ssn)?;
        }
        for binding in self.bindings.into_iter().rev() {
            self.plugins
                .on_discard_executor(binding.node, binding.ssn)?;
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.allocations.is_empty() && self.bindings.is_empty()
    }

    pub fn len(&self) -> usize {
        self.allocations.len() + self.bindings.len()
    }
}
