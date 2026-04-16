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
use crate::model::{ExecutorInfo, NodeInfoPtr, SessionInfoPtr, SnapShotPtr};
use crate::scheduler::plugins::PluginManagerPtr;
use common::FlameError;

struct PipelinedAllocation {
    node: NodeInfoPtr,
    ssn: SessionInfoPtr,
}

pub struct Statement {
    operations: Vec<PipelinedAllocation>,
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
            operations: Vec::new(),
            snapshot,
            plugins,
            controller,
        }
    }

    pub fn pipeline(&mut self, node: &NodeInfoPtr, ssn: &SessionInfoPtr) -> Result<(), FlameError> {
        self.plugins
            .on_pipeline_executor(node.clone(), ssn.clone())?;
        self.operations.push(PipelinedAllocation {
            node: node.clone(),
            ssn: ssn.clone(),
        });
        Ok(())
    }

    pub fn is_ready(&self, ssn: &SessionInfoPtr) -> Result<bool, FlameError> {
        self.plugins.is_ready(ssn)
    }

    pub async fn commit(self) -> Result<(), FlameError> {
        let batch_size = self.operations.len() as u32;
        for (idx, op) in self.operations.into_iter().enumerate() {
            let batch_index = if batch_size > 1 {
                Some(idx as u32)
            } else {
                None
            };
            let executor = self
                .controller
                .create_executor(op.node.name.clone(), op.ssn.id.clone(), batch_index)
                .await?;

            let exec_info = ExecutorInfo::from(&executor);
            self.snapshot.add_executor(Arc::new(exec_info))?;
        }
        Ok(())
    }

    pub fn discard(self) -> Result<(), FlameError> {
        for op in self.operations.into_iter().rev() {
            self.plugins.on_discard_executor(op.node, op.ssn)?;
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    pub fn len(&self) -> usize {
        self.operations.len()
    }
}
