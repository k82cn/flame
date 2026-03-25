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

use crate::controller::nodes::NodeStates;
use crate::storage::StoragePtr;
use stdng::{lock_ptr, logs::TraceFn, trace_fn, MutexPtr};

use common::apis::{Node, NodePtr, NodeState};
use common::FlameError;

/// State handler for nodes in Ready state.
///
/// Ready state means the node is connected and operational. It can transition to:
/// - Unknown: if the node starts draining (drain)
pub struct ReadyState {
    pub storage: StoragePtr,
    pub node: NodePtr,
}

#[async_trait::async_trait]
impl NodeStates for ReadyState {
    async fn register_node(&self) -> Result<(), FlameError> {
        trace_fn!("ReadyState::register_node");

        // Node is already ready, this is a no-op (idempotent registration)
        tracing::debug!("Node is already in Ready state, ignoring register_node");
        Ok(())
    }

    async fn drain(&self) -> Result<(), FlameError> {
        trace_fn!("ReadyState::drain");

        let node_name = {
            let mut node = lock_ptr!(self.node)?;
            node.state = NodeState::Unknown;
            tracing::info!(
                "Node <{}> draining, transitioning from Ready to Unknown",
                node.name
            );
            node.name.clone()
        };

        // Persist the state change
        self.storage
            .update_node_state(&node_name, NodeState::Unknown)
            .await
    }

    async fn shutdown(&self) -> Result<(), FlameError> {
        trace_fn!("ReadyState::shutdown");

        Err(FlameError::InvalidState(
            "Cannot shutdown Ready node directly, must drain first".to_string(),
        ))
    }

    async fn update_node(&self, updated: &Node) -> Result<(), FlameError> {
        trace_fn!("ReadyState::update_node");

        let mut node = lock_ptr!(self.node)?;

        // Update node fields (heartbeat, resources, etc.)
        node.capacity = updated.capacity.clone();
        node.allocatable = updated.allocatable.clone();
        node.info = updated.info.clone();
        // State remains Ready

        tracing::debug!("Node <{}> updated in Ready state", node.name);

        Ok(())
    }

    async fn release_node(&self) -> Result<(), FlameError> {
        trace_fn!("ReadyState::release_node");

        // Allow release from any state
        Ok(())
    }
}
