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

//! Connection state machine for managing node connection lifecycle.
//!
//! State transitions:
//! - (new) -> Connected (on register_node - connection created)
//! - Connected -> Draining (on drain - cleanup timer starts)
//! - Draining -> Connected (on connect before timeout)
//! - Draining -> Closed (on close after timeout)
//!
//! Once Closed, the connection cannot be recovered.

use std::sync::Arc;

use common::FlameError;
use stdng::lock_ptr;

use crate::model::{ConnectionState, Executor, NodeConnectionPtr};

mod closed;
mod connected;
mod draining;
mod manager;

use closed::ClosedState;
use connected::ConnectedState;
use draining::DrainingState;

pub use manager::ConnectionManager;

/// Creates a state handler based on the connection's current state.
pub fn from(conn_ptr: NodeConnectionPtr) -> Result<Arc<dyn ConnectionStates>, FlameError> {
    let conn = lock_ptr!(conn_ptr)?;

    tracing::debug!(
        "Build state <{:?}> for NodeConnection <{}>.",
        conn.state,
        conn.node_name
    );

    match conn.state {
        ConnectionState::Connected => Ok(Arc::new(ConnectedState {
            connection: conn_ptr.clone(),
        })),
        ConnectionState::Draining => Ok(Arc::new(DrainingState {
            connection: conn_ptr.clone(),
        })),
        ConnectionState::Closed => Ok(Arc::new(ClosedState {
            connection: conn_ptr.clone(),
        })),
    }
}

/// Trait defining the operations available for each connection state.
///
/// Each state implements this trait, returning errors for invalid operations
/// and performing state transitions for valid ones. This follows the same
/// pattern as `NodeStates` and executor `States` traits.
#[async_trait::async_trait]
pub trait ConnectionStates: Send + Sync + 'static {
    /// Transition to Connected state (reconnect from Draining).
    /// Valid from: Draining
    /// Invalid from: Connected (already connected), Closed (permanently closed)
    async fn connect(&self) -> Result<ConnectionState, FlameError>;

    /// Start draining the connection (transition to Draining state).
    /// Valid from: Connected
    /// Invalid from: Draining, Closed
    async fn drain(&self) -> Result<(), FlameError>;

    /// Close the connection permanently (transition to Closed state).
    /// Once closed, the connection cannot be reconnected/recovered.
    /// Valid from: Draining
    /// Invalid from: Connected, Closed
    async fn close(&self) -> Result<(), FlameError>;

    /// Notify the node about an executor update by pushing to the queue.
    /// Valid from: Connected
    /// Invalid from: Draining, Closed
    async fn notify_executor(&self, executor: &Executor) -> Result<(), FlameError>;

    /// Get the current state.
    fn state(&self) -> ConnectionState;
}
