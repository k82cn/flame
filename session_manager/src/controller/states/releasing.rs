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

use crate::controller::states::States;
use crate::model::ExecutorPtr;
use crate::storage::StoragePtr;

use common::apis::{ExecutorState, SessionPtr, Task, TaskOutput, TaskPtr, TaskResult};
use common::{lock_ptr, trace::TraceFn, trace_fn, FlameError};

pub struct ReleasingState {
    pub storage: StoragePtr,
    pub executor: ExecutorPtr,
}

#[async_trait::async_trait]
impl States for ReleasingState {
    async fn register_executor(&self) -> Result<(), FlameError> {
        trace_fn!("ReleasingState::register_executor");

        Err(FlameError::InvalidState(
            "Executor is releasing".to_string(),
        ))
    }

    async fn release_executor(&self) -> Result<(), FlameError> {
        trace_fn!("ReleasingState::release_executor");

        Err(FlameError::InvalidState(
            "Executor is releasing".to_string(),
        ))
    }

    async fn unregister_executor(&self) -> Result<(), FlameError> {
        trace_fn!("ReleasingState::unregister_executor");

        let mut e = lock_ptr!(self.executor)?;
        e.state = ExecutorState::Released;

        Ok(())
    }

    async fn bind_session(&self, ssn_ptr: SessionPtr) -> Result<(), FlameError> {
        trace_fn!("ReleasingState::bind_session");

        Err(FlameError::InvalidState(
            "Executor is releasing".to_string(),
        ))
    }

    async fn bind_session_completed(&self) -> Result<(), FlameError> {
        trace_fn!("ReleasingState::bind_session_completed");

        Err(FlameError::InvalidState(
            "Executor is releasing".to_string(),
        ))
    }

    async fn unbind_executor(&self) -> Result<(), FlameError> {
        trace_fn!("ReleasingState::unbind_executor");

        Err(FlameError::InvalidState(
            "Executor is releasing".to_string(),
        ))
    }

    async fn unbind_executor_completed(&self) -> Result<(), FlameError> {
        trace_fn!("ReleasingState::unbind_executor_completed");

        Err(FlameError::InvalidState(
            "Executor is releasing".to_string(),
        ))
    }

    async fn launch_task(&self, _ssn: SessionPtr) -> Result<Option<Task>, FlameError> {
        trace_fn!("ReleasingState::launch_task");

        Err(FlameError::InvalidState(
            "Executor is releasing".to_string(),
        ))
    }

    async fn complete_task(
        &self,
        _ssn: SessionPtr,
        _task: TaskPtr,
        _: TaskResult,
    ) -> Result<(), FlameError> {
        trace_fn!("ReleasingState::complete_task");

        Err(FlameError::InvalidState(
            "Executor is releasing".to_string(),
        ))
    }
}
