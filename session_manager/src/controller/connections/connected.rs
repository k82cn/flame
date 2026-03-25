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

//! Connected state for NodeConnection.
//!
//! In this state, the node is connected and operational. Valid operations:
//! - `drain()` → transitions to Draining
//! - `notify_executor()` → sends executor to the node
//!
//! Invalid operations:
//! - `connect()` → already connected
//! - `close()` → must drain first

use common::FlameError;
use stdng::lock_ptr;

use crate::controller::connections::ConnectionStates;
use crate::model::{ConnectionState, Executor, NodeConnectionPtr};

/// State handler for connections in Connected state.
pub struct ConnectedState {
    pub connection: NodeConnectionPtr,
}

#[async_trait::async_trait]
impl ConnectionStates for ConnectedState {
    async fn connect(&self) -> Result<ConnectionState, FlameError> {
        // Already connected - idempotent, just return current state
        let conn = lock_ptr!(self.connection)?;
        tracing::debug!("Node <{}> is already connected", conn.node_name);
        Ok(ConnectionState::Connected)
    }

    async fn drain(&self) -> Result<(), FlameError> {
        let mut conn = lock_ptr!(self.connection)?;

        conn.state = ConnectionState::Draining;
        tracing::info!(
            "Node <{}> draining, transitioning from Connected to Draining",
            conn.node_name
        );

        Ok(())
    }

    async fn close(&self) -> Result<(), FlameError> {
        let conn = lock_ptr!(self.connection)?;

        Err(FlameError::InvalidState(format!(
            "Cannot close Connected node <{}>, must drain first",
            conn.node_name
        )))
    }

    async fn notify_executor(&self, executor: &Executor) -> Result<(), FlameError> {
        let sender = {
            let conn = lock_ptr!(self.connection)?;
            conn.sender()
        };
        sender.send(executor.clone()).await
    }

    fn state(&self) -> ConnectionState {
        ConnectionState::Connected
    }
}
