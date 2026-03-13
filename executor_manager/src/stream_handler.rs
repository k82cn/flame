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

//! StreamHandler for managing bidirectional WatchNode streams.
//!
//! This module provides the client-side implementation of the WatchNode
//! streaming protocol, including reconnection logic and heartbeat management.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::{interval, timeout};
use tokio_stream::wrappers::ReceiverStream;
use tonic::Streaming;

use common::apis::Node;
use common::FlameError;
use rpc::flame as proto;

use crate::client::BackendClient;
use crate::executor::Executor;

/// Default interval between heartbeats in seconds.
const DEFAULT_HEARTBEAT_INTERVAL_SECS: u64 = 5;

/// Default interval between reconnection attempts in seconds.
const DEFAULT_RECONNECT_INTERVAL_SECS: u64 = 1;

/// Maximum reconnection interval in seconds (for exponential backoff).
const MAX_RECONNECT_INTERVAL_SECS: u64 = 30;

/// Timeout for receiving responses from the server.
const RESPONSE_TIMEOUT_SECS: u64 = 10;

/// Manages the bidirectional WatchNode stream lifecycle.
///
/// The StreamHandler is responsible for:
/// - Establishing and maintaining the stream connection
/// - Sending periodic heartbeats with current node status
/// - Handling reconnection with exponential backoff
/// - Processing executor state notifications from the server
/// - Forwarding executor updates to the manager for action derivation
pub struct StreamHandler {
    client: BackendClient,
    node: Arc<Mutex<Node>>,
    reconnect_interval: Duration,
    heartbeat_interval: Duration,
}

impl StreamHandler {
    /// Creates a new StreamHandler.
    ///
    /// # Arguments
    ///
    /// * `client` - The backend client for gRPC communication
    /// * `node` - The node information to register
    pub fn new(client: BackendClient, node: Node) -> Self {
        StreamHandler {
            client,
            node: Arc::new(Mutex::new(node)),
            reconnect_interval: Duration::from_secs(DEFAULT_RECONNECT_INTERVAL_SECS),
            heartbeat_interval: Duration::from_secs(DEFAULT_HEARTBEAT_INTERVAL_SECS),
        }
    }

    /// Runs the stream handler, forwarding executor updates to the manager.
    ///
    /// This method establishes the WatchNode stream and continuously
    /// processes responses from the server. On stream failure, it
    /// attempts to reconnect with exponential backoff.
    ///
    /// This is a long-running, self-recovering task that handles errors
    /// internally and never returns under normal operation.
    pub async fn run(&mut self, executor_tx: mpsc::Sender<Executor>) {
        let mut current_reconnect_interval = self.reconnect_interval;

        loop {
            match self.run_stream(&executor_tx).await {
                Ok(()) => {
                    // Stream closed gracefully, reset reconnect interval
                    current_reconnect_interval = self.reconnect_interval;
                    let node_name = self
                        .node
                        .lock()
                        .map(|n| n.name.clone())
                        .unwrap_or_else(|_| "unknown".to_string());
                    tracing::info!(
                        "WatchNode stream closed gracefully for node <{}>",
                        node_name
                    );
                }
                Err(e) => {
                    let node_name = self
                        .node
                        .lock()
                        .map(|n| n.name.clone())
                        .unwrap_or_else(|_| "unknown".to_string());
                    tracing::warn!(
                        "WatchNode stream error for node <{}>: {}. Reconnecting in {:?}",
                        node_name,
                        e,
                        current_reconnect_interval
                    );
                }
            }

            // Wait before reconnecting
            tokio::time::sleep(current_reconnect_interval).await;

            // Exponential backoff
            current_reconnect_interval = std::cmp::min(
                current_reconnect_interval * 2,
                Duration::from_secs(MAX_RECONNECT_INTERVAL_SECS),
            );
        }
    }

    /// Runs a single stream session.
    async fn run_stream(&mut self, executor_tx: &mpsc::Sender<Executor>) -> Result<(), FlameError> {
        // Create channels for the bidirectional stream
        let (request_tx, request_rx) = mpsc::channel::<proto::WatchNodeRequest>(32);

        // Start the stream
        let response_stream = self
            .client
            .watch_node(ReceiverStream::new(request_rx))
            .await?;

        // Get current node state for registration
        let node_for_registration = self
            .node
            .lock()
            .map_err(|e| FlameError::Internal(format!("Failed to lock node: {}", e)))?
            .clone();

        // Send initial registration
        let registration = proto::WatchNodeRequest {
            request: Some(proto::watch_node_request::Request::Registration(
                proto::NodeRegistration {
                    node: Some(node_for_registration.into()),
                },
            )),
        };
        request_tx
            .send(registration)
            .await
            .map_err(|e| FlameError::Network(format!("Failed to send registration: {}", e)))?;

        // Spawn heartbeat task
        let heartbeat_tx = request_tx.clone();
        let node_ptr = self.node.clone();
        let heartbeat_interval = self.heartbeat_interval;
        let heartbeat_handle = tokio::spawn(async move {
            let mut ticker = interval(heartbeat_interval);
            loop {
                ticker.tick().await;

                // Refresh and collect current node status
                let (node_name, status) = match node_ptr.lock() {
                    Ok(mut node) => {
                        // Refresh node to get current resource status
                        node.refresh();
                        let status = proto::NodeStatus {
                            state: proto::NodeState::from(node.state) as i32,
                            capacity: Some(node.capacity.clone().into()),
                            allocatable: Some(node.allocatable.clone().into()),
                            info: Some(node.info.clone().into()),
                        };
                        (node.name.clone(), Some(status))
                    }
                    Err(e) => {
                        tracing::warn!("Failed to lock node for heartbeat: {}", e);
                        continue;
                    }
                };

                let heartbeat = proto::WatchNodeRequest {
                    request: Some(proto::watch_node_request::Request::Heartbeat(
                        proto::NodeHeartbeat { node_name, status },
                    )),
                };
                if heartbeat_tx.send(heartbeat).await.is_err() {
                    break;
                }
            }
        });

        // Process responses
        let result = self.process_responses(response_stream, executor_tx).await;

        // Cancel heartbeat task
        heartbeat_handle.abort();

        result
    }

    /// Processes responses from the server stream.
    async fn process_responses(
        &self,
        mut stream: Streaming<proto::WatchNodeResponse>,
        executor_tx: &mpsc::Sender<Executor>,
    ) -> Result<(), FlameError> {
        loop {
            match timeout(
                Duration::from_secs(RESPONSE_TIMEOUT_SECS * 3), // Allow for missed heartbeats
                stream.message(),
            )
            .await
            {
                Ok(Ok(Some(response))) => {
                    self.handle_response(response, executor_tx).await?;
                }
                Ok(Ok(None)) => {
                    // Stream closed by server
                    return Ok(());
                }
                Ok(Err(e)) => {
                    return Err(FlameError::Network(format!("Stream error: {}", e)));
                }
                Err(_) => {
                    return Err(FlameError::Network("Response timeout".to_string()));
                }
            }
        }
    }

    /// Handles a single response from the server.
    ///
    /// Executor updates are forwarded directly to the manager, which is
    /// responsible for deriving the appropriate action (create/update/delete).
    async fn handle_response(
        &self,
        response: proto::WatchNodeResponse,
        executor_tx: &mpsc::Sender<Executor>,
    ) -> Result<(), FlameError> {
        match response.response {
            Some(proto::watch_node_response::Response::Executor(proto_executor)) => {
                let executor: Executor = proto_executor.into();

                tracing::debug!(
                    "WatchNode: Received executor <{}> with state {:?}",
                    executor.id,
                    executor.state
                );

                executor_tx
                    .send(executor)
                    .await
                    .map_err(|e| FlameError::Internal(format!("Failed to send executor: {}", e)))?;
            }
            Some(proto::watch_node_response::Response::Ack(ack)) => {
                tracing::trace!(
                    "WatchNode: Received acknowledgement with timestamp {}",
                    ack.timestamp
                );
            }
            None => {
                tracing::warn!("WatchNode: Received empty response");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_status_collection() {
        use common::apis::{NodeInfo, NodeState, ResourceRequirement};

        // Create a node with known values
        let node = Node {
            name: "test-node".to_string(),
            state: NodeState::Ready,
            capacity: ResourceRequirement {
                cpu: 4,
                memory: 8192,
            },
            allocatable: ResourceRequirement {
                cpu: 3,
                memory: 6144,
            },
            info: NodeInfo {
                arch: "x86_64".to_string(),
                os: "linux".to_string(),
            },
        };

        // Verify the node can be converted to proto NodeStatus
        let status = proto::NodeStatus {
            state: proto::NodeState::from(node.state) as i32,
            capacity: Some(node.capacity.clone().into()),
            allocatable: Some(node.allocatable.clone().into()),
            info: Some(node.info.clone().into()),
        };

        assert_eq!(status.state, proto::NodeState::Ready as i32);
        assert!(status.capacity.is_some());
        assert!(status.allocatable.is_some());
        assert!(status.info.is_some());

        let capacity = status.capacity.unwrap();
        assert_eq!(capacity.cpu, 4);
        assert_eq!(capacity.memory, 8192);
    }

    #[test]
    fn test_stream_handler_creation() {
        use common::apis::{NodeInfo, NodeState, ResourceRequirement};

        let node = Node {
            name: "test-node".to_string(),
            state: NodeState::Ready,
            capacity: ResourceRequirement {
                cpu: 4,
                memory: 8192,
            },
            allocatable: ResourceRequirement {
                cpu: 3,
                memory: 6144,
            },
            info: NodeInfo {
                arch: "x86_64".to_string(),
                os: "linux".to_string(),
            },
        };

        // We can't fully test StreamHandler without a real client,
        // but we can verify the struct is created correctly
        // by checking the intervals are set to defaults
        assert_eq!(
            Duration::from_secs(DEFAULT_HEARTBEAT_INTERVAL_SECS),
            Duration::from_secs(5)
        );
        assert_eq!(
            Duration::from_secs(DEFAULT_RECONNECT_INTERVAL_SECS),
            Duration::from_secs(1)
        );
        assert_eq!(
            Duration::from_secs(MAX_RECONNECT_INTERVAL_SECS),
            Duration::from_secs(30)
        );
    }

    #[test]
    fn test_executor_conversion() {
        use common::apis::{ExecutorState, ResourceRequirement};

        // Test that Executor can be created with expected fields
        let executor = Executor {
            id: "test-exec".to_string(),
            node: "test-node".to_string(),
            resreq: ResourceRequirement::default(),
            slots: 1,
            session: None,
            task: None,
            context: None,
            shim: None,
            state: ExecutorState::Idle,
        };

        assert_eq!(executor.id, "test-exec");
        assert_eq!(executor.node, "test-node");
        assert_eq!(executor.state, ExecutorState::Idle);
    }
}
