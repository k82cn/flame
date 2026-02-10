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
use common::apis::{Event, EventOwner, ExecutorState};
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

        // Own the shim.
        self.executor.shim = Some(shim_ptr.clone());
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
