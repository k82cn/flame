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

/// State handler for nodes in NotReady state.
///
/// NotReady state means the node was shutdown (cleanup completed after drain timeout).
/// It can transition to:
/// - Ready: if the node reconnects (register_node)
pub struct NotReadyState {
    pub storage: StoragePtr,
    pub node: NodePtr,
}

#[async_trait::async_trait]
impl NodeStates for NotReadyState {
    async fn register_node(&self) -> Result<(), FlameError> {
        trace_fn!("NotReadyState::register_node");

        let node_name = {
            let mut node = lock_ptr!(self.node)?;
            node.state = NodeState::Ready;
            tracing::info!(
                "Node <{}> reconnected after shutdown, transitioning from NotReady to Ready",
                node.name
            );
            node.name.clone()
        };

        // Persist the state change
        self.storage
            .update_node_state(&node_name, NodeState::Ready)
            .await
    }

    async fn drain(&self) -> Result<(), FlameError> {
        trace_fn!("NotReadyState::drain");

        Err(FlameError::InvalidState(
            "Node is already in NotReady state (shutdown)".to_string(),
        ))
    }

    async fn shutdown(&self) -> Result<(), FlameError> {
        trace_fn!("NotReadyState::shutdown");

        // Already shutdown, this is a no-op
        tracing::debug!("Node is already in NotReady state, ignoring shutdown");
        Ok(())
    }

    async fn update_node(&self, _node: &Node) -> Result<(), FlameError> {
        trace_fn!("NotReadyState::update_node");

        Err(FlameError::InvalidState(
            "Cannot update node in NotReady state, must reconnect first".to_string(),
        ))
    }

    async fn release_node(&self) -> Result<(), FlameError> {
        trace_fn!("NotReadyState::release_node");

        // Allow release from any state
        Ok(())
    }
}
