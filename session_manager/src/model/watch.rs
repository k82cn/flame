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

//! WatchRegistry for managing active WatchNode streams.
//!
//! This module provides the infrastructure for tracking and notifying
//! connected nodes about executor changes via bidirectional gRPC streams.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use common::FlameError;
use rpc::flame as proto;

use super::Executor;

/// Type alias for the stream sender
type StreamSender = mpsc::Sender<proto::WatchNodeResponse>;

/// Tracks active watch streams per node.
///
/// The WatchRegistry maintains a mapping of node names to their
/// corresponding stream senders, enabling server-push notifications
/// for executor lifecycle events.
#[derive(Clone)]
pub struct WatchRegistry {
    /// Map of node_name -> stream sender
    streams: Arc<RwLock<HashMap<String, StreamSender>>>,
}

impl Default for WatchRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl WatchRegistry {
    /// Creates a new empty WatchRegistry.
    pub fn new() -> Self {
        WatchRegistry {
            streams: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers a new watch stream for a node.
    ///
    /// If a stream already exists for the node, it will be replaced.
    /// The old stream sender will be dropped, causing the old stream to close.
    pub async fn register(&self, node_name: String, tx: StreamSender) {
        let mut streams = self.streams.write().await;
        if streams.contains_key(&node_name) {
            tracing::info!("Replacing existing watch stream for node <{}>", node_name);
        }
        streams.insert(node_name.clone(), tx);
        tracing::debug!("Registered watch stream for node <{}>", node_name);
    }

    /// Unregisters a watch stream for a node.
    pub async fn unregister(&self, node_name: &str) {
        let mut streams = self.streams.write().await;
        if streams.remove(node_name).is_some() {
            tracing::debug!("Unregistered watch stream for node <{}>", node_name);
        }
    }

    /// Notifies a specific node about an executor state change.
    pub async fn notify(
        &self,
        node_name: &str,
        response: proto::WatchNodeResponse,
    ) -> Result<(), FlameError> {
        let streams = self.streams.read().await;
        if let Some(tx) = streams.get(node_name) {
            tx.send(response).await.map_err(|e| {
                FlameError::Network(format!(
                    "Failed to send notification to node <{}>: {}",
                    node_name, e
                ))
            })?;
            Ok(())
        } else {
            Err(FlameError::NotFound(format!(
                "No watch stream registered for node <{}>",
                node_name
            )))
        }
    }

    /// Notifies all connected nodes about an executor state change.
    pub async fn notify_all(&self, response: proto::WatchNodeResponse) {
        let streams = self.streams.read().await;
        for (node_name, tx) in streams.iter() {
            if let Err(e) = tx.send(response.clone()).await {
                tracing::warn!(
                    "Failed to send broadcast notification to node <{}>: {}",
                    node_name,
                    e
                );
            }
        }
    }

    /// Returns the number of active watch streams.
    pub async fn active_streams(&self) -> usize {
        let streams = self.streams.read().await;
        streams.len()
    }

    /// Checks if a node has an active watch stream.
    pub async fn has_stream(&self, node_name: &str) -> bool {
        let streams = self.streams.read().await;
        streams.contains_key(node_name)
    }

    /// Notifies a node about an executor state change.
    /// The client derives the action (create/update/delete) from the executor's state.
    pub async fn notify_executor(
        &self,
        node_name: &str,
        executor: &Executor,
    ) -> Result<(), FlameError> {
        let response = proto::WatchNodeResponse {
            response: Some(proto::watch_node_response::Response::Executor(
                proto::Executor::from(executor),
            )),
        };
        self.notify(node_name, response).await
    }

    /// Notifies a node about an executor creation.
    /// Convenience method - sends the executor directly.
    pub async fn notify_executor_created(
        &self,
        node_name: &str,
        executor: &Executor,
    ) -> Result<(), FlameError> {
        self.notify_executor(node_name, executor).await
    }

    /// Notifies a node about an executor update.
    /// Convenience method - sends the executor directly.
    pub async fn notify_executor_updated(
        &self,
        node_name: &str,
        executor: &Executor,
    ) -> Result<(), FlameError> {
        self.notify_executor(node_name, executor).await
    }

    /// Notifies a node about an executor deletion.
    /// Convenience method - sends the executor directly.
    pub async fn notify_executor_deleted(
        &self,
        node_name: &str,
        executor: &Executor,
    ) -> Result<(), FlameError> {
        self.notify_executor(node_name, executor).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use common::apis::{ExecutorState, ResourceRequirement};

    fn create_test_executor(id: &str, node: &str) -> Executor {
        Executor {
            id: id.to_string(),
            node: node.to_string(),
            resreq: ResourceRequirement::default(),
            slots: 1,
            task_id: None,
            ssn_id: None,
            creation_time: Utc::now(),
            state: ExecutorState::Idle,
        }
    }

    #[tokio::test]
    async fn test_register_and_unregister() {
        let registry = WatchRegistry::new();
        let (tx, _rx) = mpsc::channel(10);

        // Register a node
        registry.register("node1".to_string(), tx).await;
        assert!(registry.has_stream("node1").await);
        assert_eq!(registry.active_streams().await, 1);

        // Unregister the node
        registry.unregister("node1").await;
        assert!(!registry.has_stream("node1").await);
        assert_eq!(registry.active_streams().await, 0);
    }

    #[tokio::test]
    async fn test_notify_single_node() {
        let registry = WatchRegistry::new();
        let (tx, mut rx) = mpsc::channel(10);

        registry.register("node1".to_string(), tx).await;

        let executor = create_test_executor("exec1", "node1");
        registry
            .notify_executor_created("node1", &executor)
            .await
            .unwrap();

        // Verify the message was received
        let response = rx.recv().await.unwrap();
        match response.response {
            Some(proto::watch_node_response::Response::Executor(exec)) => {
                // Executor received directly - client derives action from state
                assert!(exec.metadata.is_some());
                let metadata = exec.metadata.unwrap();
                assert_eq!(metadata.id, "exec1");
            }
            _ => panic!("Expected Executor response"),
        }
    }

    #[tokio::test]
    async fn test_notify_nonexistent_node() {
        let registry = WatchRegistry::new();
        let executor = create_test_executor("exec1", "node1");

        let result = registry
            .notify_executor_created("nonexistent", &executor)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_replace_existing_stream() {
        let registry = WatchRegistry::new();
        let (tx1, _rx1) = mpsc::channel(10);
        let (tx2, mut rx2) = mpsc::channel(10);

        // Register first stream
        registry.register("node1".to_string(), tx1).await;

        // Replace with second stream
        registry.register("node1".to_string(), tx2).await;

        // Should still have only one stream
        assert_eq!(registry.active_streams().await, 1);

        // Notification should go to the new stream
        let executor = create_test_executor("exec1", "node1");
        registry
            .notify_executor_created("node1", &executor)
            .await
            .unwrap();

        // Verify the message was received on the new stream
        let response = rx2.recv().await.unwrap();
        assert!(response.response.is_some());
    }
}
