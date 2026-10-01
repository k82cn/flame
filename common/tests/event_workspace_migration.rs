/*
Copyright 2026 The Flame Authors.
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
use bincode::{Decode, Encode};
use common::apis::EventOwner;
use common::events::{EventManager, FsEventManager};
use common::storage::{DataStorage, Index, Object, ObjectId, ObjectStorage};
use std::{fs, process::Command};

// Match the previous release's EventDao and use its actual storage writer.
#[derive(Clone, Encode, Decode)]
struct LegacyEvent {
    id: Option<u64>,
    owner: i64,
    code: i32,
    message: Index,
    creation_time: i64,
}
impl Object for LegacyEvent {
    fn id(&self) -> ObjectId {
        self.id.unwrap_or(0)
    }
    fn owner(&self) -> ObjectId {
        self.owner as u64
    }
    fn set_id(&mut self, id: ObjectId) {
        self.id = Some(id);
    }
}

#[test]
fn migrates_legacy_events_without_modifying_source() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("old");
    let session = source.join("session");
    fs::create_dir_all(&session).unwrap();
    let mut objects = ObjectStorage::new(session.to_str().unwrap(), "events").unwrap();
    let mut messages = DataStorage::new(session.to_str().unwrap(), "event_messages").unwrap();
    for owner in [0, 10] {
        objects
            .save(&LegacyEvent {
                id: None,
                owner,
                code: -42,
                message: messages.save("message 雪".as_bytes()).unwrap(),
                creation_time: 1_700_000_000_123,
            })
            .unwrap();
    }
    drop(objects);
    drop(messages);
    let original = fs::read(session.join("events.dat")).unwrap();
    assert!(FsEventManager::new(source.to_str().unwrap()).is_err());
    let target = temp.path().join("new");
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/scripts/migrate_event_workspaces.py"
    );
    let dry = Command::new("python3")
        .args([script, source.to_str().unwrap(), target.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        dry.status.success(),
        "{}",
        String::from_utf8_lossy(&dry.stderr)
    );
    assert!(!target.exists());
    let applied = Command::new("python3")
        .args([
            script,
            source.to_str().unwrap(),
            target.to_str().unwrap(),
            "--apply",
        ])
        .output()
        .unwrap();
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    let events = FsEventManager::new(target.to_str().unwrap()).unwrap();
    for task in [None, Some("10".to_string())] {
        let owner = EventOwner {
            workspace: "default".into(),
            session: "session".into(),
            task,
        };
        let result = events.find_events(owner.clone()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].code, -42);
        assert_eq!(result[0].message.as_deref(), Some("message 雪"));
        assert_eq!(
            result[0].creation_time.timestamp_millis(),
            1_700_000_000_123
        );
        assert!(events
            .find_events(EventOwner {
                workspace: "other".into(),
                ..owner
            })
            .unwrap()
            .is_empty());
    }
    assert_eq!(fs::read(session.join("events.dat")).unwrap(), original);
    let duplicate = Command::new("python3")
        .args([
            script,
            source.to_str().unwrap(),
            target.to_str().unwrap(),
            "--apply",
        ])
        .output()
        .unwrap();
    assert!(!duplicate.status.success());
    fs::write(session.join("event_messages.dat"), b"truncated").unwrap();
    let corrupt_target = temp.path().join("corrupt");
    let corrupt = Command::new("python3")
        .args([
            script,
            source.to_str().unwrap(),
            corrupt_target.to_str().unwrap(),
            "--apply",
        ])
        .output()
        .unwrap();
    assert!(!corrupt.status.success());
    assert!(!corrupt_target.exists());
}
