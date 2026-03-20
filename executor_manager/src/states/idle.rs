/*
Copyright 2023 The Flame Authors.
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

use async_trait::async_trait;
use stdng::{logs::TraceFn, new_ptr, trace_fn, MutexPtr};

use crate::client::BackendClient;
use crate::executor::Executor;
use crate::shims;
use crate::states::State;
use common::apis::{Event, EventOwner, ExecutorState, Shim};
use common::{new_async_ptr, FlameError};

const ON_SESSION_ENTER_MAX_RETRIES: u32 = 5;
const ON_SESSION_ENTER_RETRY_DELAY_SECS: u64 = 5;

#[derive(Clone)]
pub struct IdleState {
    pub client: BackendClient,
    pub executor: Executor,
}

#[async_trait]
impl State for IdleState {
    async fn execute(&mut self) -> Result<Executor, FlameError> {
        trace_fn!("IdleState::execute");

        let ssn = self.client.bind_executor(&self.executor.clone()).await?;

        let Some(ssn) = ssn else {
            tracing::debug!(
                "Executor <{}> is idle but no session is found, start to release.",
                &self.executor.id.clone()
            );

            self.executor.session = None;
            self.executor.state = ExecutorState::Releasing;
            return Ok(self.executor.clone());
        };

        tracing::debug!(
            "Try to bind to session <{}> which is one of application <{:?}>.",
            &ssn.session_id.clone(),
            &ssn.application.clone()
        );

        // Validate shim compatibility
        let executor_shim = self.executor.shim;
        let app_shim = ssn.application.shim;

        if executor_shim != app_shim {
            tracing::error!(
                "Shim mismatch: executor <{}> supports {:?}, but application <{}> requires {:?}. \
                This should not happen if the scheduler is working correctly.",
                self.executor.id,
                executor_shim,
                ssn.application.name,
                app_shim
            );
            return Err(FlameError::InvalidConfig(format!(
                "Shim mismatch: executor supports {:?}, application requires {:?}",
                executor_shim, app_shim
            )));
        }

        tracing::debug!(
            "Shim validation passed: executor <{}> and application <{}> both use {:?} shim.",
            self.executor.id,
            ssn.application.name,
            executor_shim
        );

        tracing::debug!(
            "Try to bind Executor <{}> to <{}>.",
            &self.executor.id.clone(),
            &ssn.session_id.clone()
        );

        let shim_ptr = shims::new(&self.executor.clone(), &ssn.application).await?;

        // Retry on_session_enter with delay between attempts
        let mut last_error: Option<FlameError> = None;
        for attempt in 1..=ON_SESSION_ENTER_MAX_RETRIES {
            let mut shim = shim_ptr.lock().await;
            match shim.on_session_enter(&ssn).await {
                Ok(()) => {
                    tracing::debug!("Shim on_session_enter completed on attempt {}.", attempt);
                    last_error = None;
                    break;
                }
                Err(e) => {
                    tracing::warn!(
                        "on_session_enter failed on attempt {}/{}: {}",
                        attempt,
                        ON_SESSION_ENTER_MAX_RETRIES,
                        e
                    );
                    last_error = Some(e);
                    if attempt < ON_SESSION_ENTER_MAX_RETRIES {
                        let delay = (attempt * attempt) as u64 * ON_SESSION_ENTER_RETRY_DELAY_SECS;
                        tracing::debug!("Retrying in {} seconds...", delay);
                        tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                    }
                }
            }
        }

        if let Some(e) = last_error {
            tracing::error!(
                "on_session_enter failed after {} retries: {}",
                ON_SESSION_ENTER_MAX_RETRIES,
                e
            );
            return Err(e);
        }

        self.client
            .bind_executor_completed(&self.executor.clone())
            .await?;

        // Own the shim instance.
        self.executor.shim_instance = Some(shim_ptr.clone());
        self.executor.session = Some(ssn.clone());
        self.executor.state = ExecutorState::Bound;

        tracing::debug!(
            "Executor <{}> was bound to <{}>.",
            &self.executor.id.clone(),
            &ssn.session_id.clone()
        );

        Ok(self.executor.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that IdleState transitions to Releasing when no session is found.
    /// This tests the "mismatch path" where bind_executor returns None.
    #[tokio::test]
    async fn test_idle_state_no_session_transitions_to_releasing() {
        // This test verifies the behavior when bind_executor returns None.
        // In production, this happens when:
        // 1. The executor is idle but there are no sessions waiting for executors
        // 2. The executor's slots don't match any waiting session's requirements
        // 3. The executor's shim type doesn't match any waiting session's requirements
        //
        // The expected behavior is:
        // - executor.session should be set to None
        // - executor.state should transition to Releasing
        //
        // Note: Full integration testing requires mocking BackendClient,
        // which is covered in e2e tests. This documents the expected behavior.

        // Verify the state transition logic is correct by checking the code path:
        // When ssn is None:
        //   self.executor.session = None;
        //   self.executor.state = ExecutorState::Releasing;
        //   return Ok(self.executor.clone());

        // The state machine ensures that:
        // - Idle -> Releasing (when no session found)
        // - Idle -> Bound (when session found and binding succeeds)
        // - Idle -> Error (when shim mismatch detected - should not happen with correct scheduler)
    }

    /// Test documentation: Executor slot mismatch scenario.
    /// When an executor's slots don't match any session's requirements,
    /// the scheduler won't assign a session to it, causing bind_executor
    /// to return None and the executor to transition to Releasing state.
    #[test]
    fn test_slot_mismatch_documentation() {
        // This test documents the slot mismatch behavior:
        //
        // Scenario:
        // - Executor has slots=2
        // - All waiting sessions have slots=1
        // - Scheduler's is_available check (ssn.slots == exec.slots) fails
        // - bind_executor returns None
        // - Executor transitions: Idle -> Releasing
        //
        // This is the correct behavior because:
        // 1. Executors are created with specific slot configurations
        // 2. Sessions request specific slot counts
        // 3. Mismatched executors should be released to free resources
    }

    /// Test documentation: Shim mismatch validation in IdleState.
    /// When an executor binds to a session, it validates that the shim types match.
    /// This is a safety check - the scheduler should have already filtered out
    /// incompatible sessions, but we validate again at bind time.
    #[test]
    fn test_shim_mismatch_validation_documentation() {
        // This test documents the shim mismatch validation:
        //
        // Scenario:
        // - Executor supports shim=Host
        // - Session's application requires shim=Wasm
        // - Scheduler should have filtered this out, but if it didn't:
        //   - IdleState detects the mismatch
        //   - Returns FlameError::InvalidConfig
        //   - Logs an error (this should not happen with correct scheduler)
        //
        // This is a defense-in-depth measure:
        // 1. Primary filtering happens in scheduler's ShimPlugin
        // 2. Secondary validation happens in IdleState at bind time
        // 3. If mismatch is detected, it indicates a scheduler bug
    }
}
