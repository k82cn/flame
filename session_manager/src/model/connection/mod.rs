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

//! Connection management between session_manager (fsm) and executor_manager (fem).
//!
//! # Architecture
//!
//! - `NodeConnection`: Represents a single connection to one node (one-per-node)
//! - `NodeConnectionSender`: Cloneable handle for sending executor updates
//! - `NodeConnectionReceiver`: Cloneable handle for receiving executor updates
//! - `ConnectionManager`: Manages all NodeConnections in the controller
//! - `ConnectionStates`: Trait in controller/connections for state machine operations
//!
//! # State Machine
//!
//! ```text
//!                    ┌─────────────┐
//!                    │   (new)     │
//!                    └──────┬──────┘
//!                           │ connect()
//!                           ▼
//!              ┌────────────────────────┐
//!              │      Connected         │◄───────┐
//!              │  - can send/recv       │        │
//!              │  - can drain           │        │
//!              └───────────┬────────────┘        │
//!                          │ drain()             │
//!                          ▼                     │
//!              ┌────────────────────────┐        │
//!              │      Draining          │        │
//!              │  - drain timer running │────────┘ connect()
//!              │  - waiting for close   │
//!              └───────────┬────────────┘
//!                          │ close()
//!                          ▼
//!              ┌────────────────────────┐
//!              │       Closed           │
//!              │  - permanently closed  │
//!              │  - cannot recover      │
//!              └────────────────────────┘
//! ```
//!
//! # Connection Lifecycle
//!
//! 1. `register_node` - Creates connection in Connected state, sends initial executors via sender
//! 2. `watch_node` - Gets (sender, receiver) handles, receives executors via receiver
//! 3. `on_draining` - Called when the watch stream closes (starts drain timer)
//! 4. `on_closed` - Called when drain timer expires (connection closed)
//!
//! # Channel Pattern
//!
//! The connection uses sender/receiver handles that are cloneable and safe to use across await points:
//!
//! ```ignore
//! // Get handles from ConnectionManager
//! let (sender, receiver) = connection_manager.connect(node_name).await?;
//!
//! // Send executor updates (can be called from multiple tasks)
//! sender.send(executor).await?;
//!
//! // Receive executor updates (typically in a dedicated task)
//! while let Some(executor) = receiver.recv().await {
//!     // process executor
//! }
//! ```

use tokio_util::sync::CancellationToken;

use stdng::collections::AsyncQueue;
use stdng::MutexPtr;

use common::FlameError;

use super::Executor;

/// Default timeout before shutting down a disconnected node (30 seconds)
pub const DEFAULT_DRAIN_TIMEOUT_SECS: u64 = 30;

/// Type alias for NodeConnection pointer (thread-safe mutable reference)
pub type NodeConnectionPtr = MutexPtr<NodeConnection>;

/// Connection state for a node.
#[derive(Clone, Debug, PartialEq)]
pub enum ConnectionState {
    /// Node is connected and streaming
    Connected,
    /// Node disconnected, drain timer is running
    Draining,
    /// Connection is permanently closed
    Closed,
}

/// Represents a single connection to one node.
///
/// Each node has exactly one NodeConnection instance managed by ConnectionManager.
/// The connection lifecycle is managed through the state machine pattern.
///
/// Use `send()` to push executor updates to the node, and `recv()` to receive them.
/// Note: The queue is cloneable and can be used across await points.
pub struct NodeConnection {
    /// The node name this connection belongs to
    pub node_name: String,
    /// Internal queue of executor updates (cloneable for async operations)
    queue: AsyncQueue<Executor>,
    /// Current connection state
    pub state: ConnectionState,
    /// Cancellation token for the drain timer (if running)
    pub drain_cancel: Option<CancellationToken>,
}

impl NodeConnection {
    /// Creates a new NodeConnection in Connected state.
    pub fn new(node_name: String) -> Self {
        Self {
            node_name,
            queue: AsyncQueue::new(),
            state: ConnectionState::Connected,
            drain_cancel: None,
        }
    }

    /// Returns the node name.
    pub fn node_name(&self) -> &str {
        &self.node_name
    }

    /// Returns the current connection state.
    pub fn state(&self) -> &ConnectionState {
        &self.state
    }

    /// Checks if the connection is currently connected.
    pub fn is_connected(&self) -> bool {
        self.state == ConnectionState::Connected
    }

    /// Returns a sender handle for this connection.
    ///
    /// The sender can be used to send executor updates to the node.
    /// It is cloneable and can be used across await points.
    pub fn sender(&self) -> NodeConnectionSender {
        NodeConnectionSender {
            queue: self.queue.clone(),
            node_name: self.node_name.clone(),
        }
    }

    /// Returns a receiver handle for this connection.
    ///
    /// The receiver can be used to receive executor updates from the connection.
    /// It is cloneable and can be used across await points.
    pub fn receiver(&self) -> NodeConnectionReceiver {
        NodeConnectionReceiver {
            queue: self.queue.clone(),
        }
    }
}

/// Sender handle for a NodeConnection.
///
/// Can be used to send executor updates to the node.
/// Cloneable and safe to use across await points.
#[derive(Clone)]
pub struct NodeConnectionSender {
    queue: AsyncQueue<Executor>,
    node_name: String,
}

impl NodeConnectionSender {
    /// Sends an executor update to the node.
    pub async fn send(&self, executor: Executor) -> Result<(), FlameError> {
        self.queue.push(executor).await.map_err(|_| {
            FlameError::Network(format!(
                "Failed to send executor to node <{}>",
                self.node_name
            ))
        })
    }
}

/// Receiver handle for a NodeConnection.
///
/// Can be used to receive executor updates from the connection.
/// Cloneable and safe to use across await points.
#[derive(Clone)]
pub struct NodeConnectionReceiver {
    queue: AsyncQueue<Executor>,
}

impl NodeConnectionReceiver {
    /// Receives an executor update from the connection.
    ///
    /// Returns None if the queue is closed.
    pub async fn recv(&self) -> Option<Executor> {
        self.queue.pop().await
    }
}

/// Callbacks for node connection lifecycle events.
///
/// These callbacks are invoked by the state machine during state transitions,
/// allowing the controller to react to connection events.
#[async_trait::async_trait]
pub trait ConnectionCallbacks: Send + Sync + 'static {
    /// Called when a node successfully connects (new or reconnect).
    ///
    /// This is invoked for both new connections and reconnections from Draining state.
    /// The controller should transition the node to Ready state.
    /// Note: Node registration in storage is done separately in controller.register_node().
    async fn on_connected(&self, node_name: &str) -> Result<(), FlameError>;

    /// Called when a node's stream disconnects and enters draining state.
    ///
    /// This is invoked after transitioning to Draining state.
    /// The controller should transition the node to Unknown state.
    async fn on_draining(&self, node_name: &str) -> Result<(), FlameError>;

    /// Called when the drain timeout expires and connection is closed.
    ///
    /// This is invoked after transitioning to Closed state.
    /// The controller should shutdown the node and clean up its executors.
    async fn on_closed(&self, node_name: &str) -> Result<(), FlameError>;
}
