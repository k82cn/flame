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

//! Closed state for NodeConnection.
//!
//! In this state, the connection has been permanently closed after the drain timeout expired.
//! The connection cannot be reconnected or recovered.
//!
//! Invalid operations:
//! - `connect()` → connection is permanently closed
//! - `drain()` → already closed
//! - `close()` → already closed
//! - `notify_executor()` → no active connection

use common::FlameError;
use stdng::lock_ptr;

use crate::controller::connections::ConnectionStates;
use crate::model::{ConnectionState, Executor, NodeConnectionPtr};

/// State handler for connections in Closed state.
pub struct ClosedState {
    pub connection: NodeConnectionPtr,
}

#[async_trait::async_trait]
impl ConnectionStates for ClosedState {
    async fn connect(&self) -> Result<ConnectionState, FlameError> {
        let conn = lock_ptr!(self.connection)?;

        Err(FlameError::InvalidState(format!(
            "Node <{}> connection is permanently closed, cannot reconnect",
            conn.node_name
        )))
    }

    async fn drain(&self) -> Result<(), FlameError> {
        let conn = lock_ptr!(self.connection)?;

        Err(FlameError::InvalidState(format!(
            "Node <{}> connection is already closed",
            conn.node_name
        )))
    }

    async fn close(&self) -> Result<(), FlameError> {
        let conn = lock_ptr!(self.connection)?;

        Err(FlameError::InvalidState(format!(
            "Node <{}> connection is already closed",
            conn.node_name
        )))
    }

    async fn notify_executor(&self, _executor: &Executor) -> Result<(), FlameError> {
        let conn = lock_ptr!(self.connection)?;

        Err(FlameError::InvalidState(format!(
            "Cannot notify executor to node <{}>, connection is closed",
            conn.node_name
        )))
    }

    fn state(&self) -> ConnectionState {
        ConnectionState::Closed
    }
}
