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

use chrono::{DateTime, Utc};
use stdng::{lock_ptr, new_ptr, MutexPtr};

use crate::apis::{Event, EventOwner, SessionPath, TaskName};
use crate::FlameError;

use super::EventManager;

#[derive(Clone, Debug)]
struct InMemoryEvent {
    code: i32,
    message: String,
    creation_time: i64,
}

pub struct MemoryEventManager {
    events: MutexPtr<HashMap<SessionPath, HashMap<TaskName, Vec<InMemoryEvent>>>>,
}

impl MemoryEventManager {
    pub fn new() -> Self {
        Self {
            events: new_ptr(HashMap::new()),
        }
    }
}

impl Default for MemoryEventManager {
    fn default() -> Self {
        Self::new()
    }
}

impl EventManager for MemoryEventManager {
    fn record_event(&self, owner: EventOwner, event: Event) -> Result<(), FlameError> {
        let session = owner.session_path()?;
        let mut events = lock_ptr!(self.events)?;
        events
            .entry(session)
            .or_default()
            .entry(owner.task)
            .or_default()
            .push(InMemoryEvent {
                code: event.code,
                message: event.message.unwrap_or_default(),
                creation_time: event.creation_time.timestamp_millis(),
            });
        Ok(())
    }

    fn find_events(&self, owner: EventOwner) -> Result<Vec<Event>, FlameError> {
        let session = owner.session_path()?;
        let events = lock_ptr!(self.events)?;
        let Some(session_events) = events.get(&session) else {
            return Ok(vec![]);
        };
        let Some(task_events) = session_events.get(&owner.task) else {
            return Ok(vec![]);
        };

        let mut event_list = Vec::with_capacity(task_events.len());
        for e in task_events {
            let creation_time = DateTime::<Utc>::from_timestamp_millis(e.creation_time)
                .ok_or(FlameError::Internal("Invalid creation time".to_string()))?;
            event_list.push(Event {
                code: e.code,
                message: Some(e.message.clone()),
                creation_time,
            });
        }
        Ok(event_list)
    }

    fn remove_events(&self, session_path: SessionPath) -> Result<(), FlameError> {
        let mut events = lock_ptr!(self.events)?;
        events.remove(&session_path);
        Ok(())
    }

    fn clear(&self) -> Result<(), FlameError> {
        let mut events = lock_ptr!(self.events)?;
        events.clear();
        Ok(())
    }
}
