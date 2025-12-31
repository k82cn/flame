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

use std::collections::{hash_map::Entry, HashMap};
use std::fs;
use std::sync::{Arc, Mutex};

use bincode::{Decode, Encode};
use chrono::{DateTime, Utc};
use stdng::{lock_ptr, new_ptr, MutexPtr};

use common::apis::{Event, EventOwner, SessionID, TaskID};
use common::storage::{DataStorage, Index, Object, ObjectId, ObjectStorage};
use common::FlameError;

const EVENT_STORAGE: &str = "events";

struct EventStorage {
    object_storage: ObjectStorage,
    data_storage: DataStorage,
}

#[derive(Clone, Debug, Encode, Decode)]
struct EventDao {
    id: Option<u64>,
    owner: TaskID,
    code: i32,
    message: Index,
    creation_time: i64,
}

impl Object for EventDao {
    fn id(&self) -> ObjectId {
        self.id.unwrap_or(0)
    }

    fn owner(&self) -> ObjectId {
        self.owner as ObjectId
    }
    fn set_id(&mut self, id: ObjectId) {
        self.id = Some(id);
    }
}

pub type EventManagerPtr = Arc<EventManager>;

pub struct EventManager {
    storage_path: String,
    event_storage: MutexPtr<HashMap<SessionID, EventStorage>>,
    events: MutexPtr<HashMap<SessionID, HashMap<TaskID, Vec<EventDao>>>>,
}

impl EventManager {
    pub fn new(path: Option<&str>) -> Result<Self, FlameError> {
        let storage_path = path.unwrap_or(EVENT_STORAGE);
        fs::create_dir_all(storage_path)?;

        let mut evtmgr = Self {
            storage_path: storage_path.to_string(),
            event_storage: new_ptr(HashMap::new()),
            events: new_ptr(HashMap::new()),
        };

        // Setup event storage for all sessions.
        let sessions = evtmgr.list_sessions()?;
        for session_id in &sessions {
            evtmgr.setup_event_storage(session_id.clone())?;
        }

        // Setup event message for all sessions.
        evtmgr.load_events()?;

        Ok(evtmgr)
    }

    fn load_events(&self) -> Result<(), FlameError> {
        let mut event_storage = lock_ptr!(self.event_storage)?;
        let mut events = lock_ptr!(self.events)?;
        let sessions = event_storage.keys().cloned().collect::<Vec<SessionID>>();
        for session_id in &sessions {
            let event_daos: Vec<EventDao> = event_storage
                .get_mut(session_id)
                .ok_or(FlameError::Internal(format!(
                    "Event storage not found: {}",
                    session_id
                )))?
                .object_storage
                .list(None)?;
            for event_dao in event_daos {
                events
                    .entry(session_id.clone())
                    .or_insert(HashMap::new())
                    .entry(event_dao.owner as TaskID)
                    .or_insert(vec![])
                    .push(event_dao);
            }
        }
        Ok(())
    }

    fn list_sessions(&self) -> Result<Vec<SessionID>, FlameError> {
        let mut sessions = vec![];
        let entries = fs::read_dir(&self.storage_path)?;
        for entry in entries {
            let file_name = entry?.file_name();
            let session_id = file_name.to_string_lossy().to_string();
            sessions.push(session_id);
        }

        Ok(sessions)
    }

    fn setup_event_storage(&self, session_id: SessionID) -> Result<(), FlameError> {
        let base_path = format!("{}/{}", self.storage_path, session_id);

        let mut event_storage = lock_ptr!(self.event_storage)?;
        if let Entry::Vacant(e) = event_storage.entry(session_id) {
            fs::create_dir_all(&base_path)?;

            let storage = EventStorage {
                object_storage: ObjectStorage::new(&base_path, "events")?,
                data_storage: DataStorage::new(&base_path, "event_messages")?,
            };
            e.insert(storage);
        }

        Ok(())
    }

    pub fn record_event(&self, owner: EventOwner, event: Event) -> Result<(), FlameError> {
        self.setup_event_storage(owner.session_id.clone())?;

        let mut event_storage = lock_ptr!(self.event_storage)?;
        let event_storage: &mut EventStorage = event_storage
            .get_mut(&owner.session_id)
            .ok_or(FlameError::Internal("Event storage not found".to_string()))?;

        let message = event.message.unwrap_or_default();
        let msg_index = event_storage.data_storage.save(message.as_bytes())?;

        let event_dao = EventDao {
            id: None,
            owner: owner.task_id,
            code: event.code,
            message: msg_index,
            creation_time: event.creation_time.timestamp(),
        };

        event_storage.object_storage.save(&event_dao)?;

        let mut events = lock_ptr!(self.events)?;
        events
            .entry(owner.session_id)
            .or_insert_with(HashMap::new)
            .entry(owner.task_id)
            .or_insert_with(Vec::new)
            .push(event_dao);

        Ok(())
    }

    pub fn find_events(&self, owner: EventOwner) -> Result<Vec<Event>, FlameError> {
        let mut event_storage = lock_ptr!(self.event_storage)?;
        let event_storage: &mut EventStorage = event_storage
            .get_mut(&owner.session_id)
            .ok_or(FlameError::Internal("Event storage not found".to_string()))?;

        let event_daos: Vec<EventDao> = event_storage.object_storage.list(None)?;

        let events = lock_ptr!(self.events)?;
        let event_daos = events
            .get(&owner.session_id)
            .ok_or(FlameError::Internal("Session not found".to_string()))?
            .get(&owner.task_id)
            .ok_or(FlameError::Internal("Task not found".to_string()))?;

        let mut event_list = vec![];
        for event_dao in event_daos {
            let message = event_storage.data_storage.load(&event_dao.message)?;
            event_list.push(Event {
                code: event_dao.code,
                message: Some(String::from_utf8(message)?),
                creation_time: DateTime::<Utc>::from_timestamp(event_dao.creation_time, 0)
                    .ok_or(FlameError::Internal("Invalid creation time".to_string()))?,
            });
        }

        Ok(event_list)
    }

    pub fn remove_events(&self, session_id: SessionID) -> Result<(), FlameError> {
        {
            let mut event_storage = lock_ptr!(self.event_storage)?;
            let event_storage: &mut EventStorage = event_storage
                .get_mut(&session_id)
                .ok_or(FlameError::Internal("Event storage not found".to_string()))?;

            event_storage.object_storage.clear()?;
            event_storage.data_storage.clear()?;
        }

        {
            let mut events = lock_ptr!(self.events)?;
            events
                .remove(&session_id)
                .ok_or(FlameError::Internal(format!(
                    "Session not found: {}",
                    session_id
                )))?;
        }

        fs::remove_dir_all(format!("{}/{}", self.storage_path, session_id)).map_err(|e| {
            FlameError::Storage(format!("Failed to remove event storage directory: {}", e))
        })?;

        Ok(())
    }

    pub fn clear(&self) -> Result<(), FlameError> {
        let mut event_storage = lock_ptr!(self.event_storage)?;
        for event_storage in event_storage.values_mut() {
            event_storage.object_storage.clear()?;
            event_storage.data_storage.clear()?;
        }

        let mut events = lock_ptr!(self.events)?;
        events.clear();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_record_event() {
        let temp_dir = tempfile::tempdir().unwrap();
        let event_manager =
            EventManager::new(Some(temp_dir.path().to_string_lossy().as_ref())).unwrap();
        event_manager
            .record_event(
                EventOwner {
                    session_id: String::from("1"),
                    task_id: 1,
                },
                Event {
                    code: 1,
                    message: Some("test".to_string()),
                    creation_time: Utc::now(),
                },
            )
            .unwrap();

        event_manager.clear().unwrap();
    }

    #[test]
    fn test_find_events() {
        let temp_dir = tempfile::tempdir().unwrap();
        let event_manager =
            EventManager::new(Some(temp_dir.path().to_string_lossy().as_ref())).unwrap();
        event_manager
            .record_event(
                EventOwner {
                    session_id: String::from("1"),
                    task_id: 1,
                },
                Event {
                    code: 1,
                    message: Some("test".to_string()),
                    creation_time: Utc::now(),
                },
            )
            .unwrap();

        let events = event_manager
            .find_events(EventOwner {
                session_id: String::from("1"),
                task_id: 1,
            })
            .unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].code, 1);
        assert_eq!(events[0].message, Some("test".to_string()));

        event_manager.clear().unwrap();
    }
}
