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

use std::collections::HashMap;

use common::apis::SessionID;
use common::FlameError;

use crate::model::{NodeInfoPtr, SessionInfoPtr, SnapShot};
use crate::scheduler::plugins::{Plugin, PluginPtr};

struct GangState {
    batch_size: u32,
    pipelined: u32,
}

pub struct GangPlugin {
    ssn_state: HashMap<SessionID, GangState>,
}

impl GangPlugin {
    pub fn new_ptr() -> PluginPtr {
        Box::new(GangPlugin {
            ssn_state: HashMap::new(),
        })
    }
}

impl Plugin for GangPlugin {
    fn setup(&mut self, ss: &SnapShot) -> Result<(), FlameError> {
        self.ssn_state.clear();

        let sessions = ss
            .sessions
            .lock()
            .map_err(|e| FlameError::Internal(format!("failed to lock sessions: {}", e)))?;

        for ssn in sessions.values() {
            self.ssn_state.insert(
                ssn.id.clone(),
                GangState {
                    batch_size: ssn.batch_size.max(1),
                    pipelined: 0,
                },
            );
        }

        Ok(())
    }

    fn is_ready(&self, ssn: &SessionInfoPtr) -> Option<bool> {
        let state = self.ssn_state.get(&ssn.id)?;
        if state.batch_size <= 1 {
            return Some(state.pipelined > 0);
        }
        Some(state.pipelined > 0 && state.pipelined % state.batch_size == 0)
    }

    fn on_pipeline_executor(&mut self, _node: NodeInfoPtr, ssn: SessionInfoPtr) {
        if let Some(state) = self.ssn_state.get_mut(&ssn.id) {
            state.pipelined += 1;
        }
    }

    fn on_bind_executor(&mut self, _node: NodeInfoPtr, ssn: SessionInfoPtr) {
        if let Some(state) = self.ssn_state.get_mut(&ssn.id) {
            state.pipelined += 1;
        }
    }

    fn on_discard_executor(&mut self, _node: NodeInfoPtr, ssn: SessionInfoPtr) {
        if let Some(state) = self.ssn_state.get_mut(&ssn.id) {
            state.pipelined = state.pipelined.saturating_sub(1);
        }
    }
}
