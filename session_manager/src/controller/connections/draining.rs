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

//! Draining state for NodeConnection.
//!
//! In this state, the node has disconnected and the drain timer is running.
//! Valid operations:
//! - `connect()` → reconnect before timeout, transitions to Connected
//! - `close()` → drain timer expired, transitions to Closed
//!
//! Invalid operations:
//! - `drain()` → already draining
//! - `notify_executor()` → no active connection

use common::FlameError;
use stdng::lock_ptr;

use crate::controller::connections::ConnectionStates;
use crate::model::{ConnectionState, Executor, NodeConnectionPtr};

/// State handler for connections in Draining state.
pub struct DrainingState {
    pub connection: NodeConnectionPtr,
}

#[async_trait::async_trait]
impl ConnectionStates for DrainingState {
    async fn connect(&self) -> Result<ConnectionState, FlameError> {
        let mut conn = lock_ptr!(self.connection)?;

        // Cancel the drain timer
        if let Some(cancel_token) = conn.drain_cancel.take() {
            cancel_token.cancel();
            tracing::info!(
                "Cancelled drain timer for reconnecting node <{}>",
                conn.node_name
            );
        }

        // Store previous state for callback
        let previous_state = conn.state.clone();

        // Transition to Connected state
        conn.state = ConnectionState::Connected;
        tracing::info!(
            "Node <{}> reconnected, transitioning from Draining to Connected",
            conn.node_name
        );

        Ok(previous_state)
    }

    async fn drain(&self) -> Result<(), FlameError> {
        let conn = lock_ptr!(self.connection)?;

        Err(FlameError::InvalidState(format!(
            "Node <{}> is already draining",
            conn.node_name
        )))
    }

    async fn close(&self) -> Result<(), FlameError> {
        let mut conn = lock_ptr!(self.connection)?;

        conn.state = ConnectionState::Closed;
        conn.drain_cancel = None;

        tracing::info!(
            "Node <{}> connection closed, transitioning from Draining to Closed",
            conn.node_name
        );

        Ok(())
    }

    async fn notify_executor(&self, _executor: &Executor) -> Result<(), FlameError> {
        let conn = lock_ptr!(self.connection)?;

        Err(FlameError::InvalidState(format!(
            "Cannot notify executor to node <{}> in Draining state",
            conn.node_name
        )))
    }

    fn state(&self) -> ConnectionState {
        ConnectionState::Draining
    }
}
