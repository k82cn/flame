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

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use futures::future::try_join_all;

use serde_json::json;

use flame_rs as flame;

use flame::{
    apis::{FlameError, SessionState, Shim, TaskState},
    client::{ApplicationAttributes, ApplicationSchema, SessionAttributes, Task, TaskInformer},
    lock_ptr, new_ptr,
};

const FLAME_DEFAULT_ADDR: &str = "http://127.0.0.1:8080";

const FLAME_DEFAULT_APP: &str = "flmping";

pub struct DefaultTaskInformer {
    pub succeed: i32,
    pub failed: i32,
    pub error: i32,
}

impl TaskInformer for DefaultTaskInformer {
    fn on_update(&mut self, task: Task) {
        tracing::info!("task: {:?}", task.state);
        match task.state {
            TaskState::Succeed => self.succeed += 1,
            TaskState::Failed => self.failed += 1,
            _ => {}
        }
    }

    fn on_error(&mut self, _: FlameError) {
        self.error += 1;
        tracing::info!("error: {}", self.error);
    }
}

#[tokio::test]
async fn test_create_session() -> Result<(), FlameError> {
    let conn = flame::client::connect(FLAME_DEFAULT_ADDR).await?;

    let ssn_attr = SessionAttributes {
        application: FLAME_DEFAULT_APP.to_string(),
        slots: 1,
        common_data: None,
    };
    let ssn = conn.create_session(&ssn_attr).await?;

    assert_eq!(ssn.state, SessionState::Open);

    ssn.close().await?;

    Ok(())
}

#[tokio::test]
async fn test_create_multiple_sessions() -> Result<(), FlameError> {
    let conn = flame::client::connect(FLAME_DEFAULT_ADDR).await?;

    let ssn_num = 10;

    for _ in 0..ssn_num {
        let ssn_attr = SessionAttributes {
            application: FLAME_DEFAULT_APP.to_string(),
            slots: 1,
            common_data: None,
        };
        let ssn = conn.create_session(&ssn_attr).await?;

        assert_eq!(ssn.state, SessionState::Open);

        ssn.close().await?;
    }

    Ok(())
}

#[tokio::test]
async fn test_create_session_with_tasks() -> Result<(), FlameError> {
    let conn = flame::client::connect(FLAME_DEFAULT_ADDR).await?;

    let ssn_attr = SessionAttributes {
        application: FLAME_DEFAULT_APP.to_string(),
        slots: 1,
        common_data: None,
    };
    let ssn = conn.create_session(&ssn_attr).await?;

    assert_eq!(ssn.state, SessionState::Open);

    let informer = new_ptr!(DefaultTaskInformer {
        succeed: 0,
        failed: 0,
        error: 0,
    });

    let task_num = 100;
    let mut tasks = vec![];
    for _ in 0..task_num {
        let task = ssn.run_task(None, informer.clone());
        tasks.push(task);
    }

    try_join_all(tasks).await?;

    {
        let informer = lock_ptr!(informer)?;
        assert_eq!(informer.succeed, task_num);
    }

    // Also check the events of the task.
    let task = ssn.get_task(&String::from("1")).await?;
    assert_eq!(task.state, TaskState::Succeed);
    assert_ne!(task.events.len(), 0);
    for event in task.events {
        assert!(
            event.code == TaskState::Succeed as i32
                || event.code == TaskState::Pending as i32
                || event.code == TaskState::Running as i32,
            "event code <{}> is not valid",
            event.code
        );
    }

    ssn.close().await?;

    Ok(())
}

#[tokio::test]
async fn test_create_multiple_sessions_with_tasks() -> Result<(), FlameError> {
    let conn = flame::client::connect(FLAME_DEFAULT_ADDR).await?;

    let ssn_attr = SessionAttributes {
        application: FLAME_DEFAULT_APP.to_string(),
        slots: 1,
        common_data: None,
    };
    let ssn_1 = conn.create_session(&ssn_attr).await?;
    assert_eq!(ssn_1.state, SessionState::Open);

    let ssn_2 = conn.create_session(&ssn_attr).await?;
    assert_eq!(ssn_2.state, SessionState::Open);

    let informer = new_ptr!(DefaultTaskInformer {
        succeed: 0,
        failed: 0,
        error: 0,
    });

    let task_num = 100;
    let mut tasks = vec![];

    for _ in 0..task_num {
        let task = ssn_1.run_task(None, informer.clone());
        tasks.push(task);
    }

    for _ in 0..task_num {
        let task = ssn_2.run_task(None, informer.clone());
        tasks.push(task);
    }

    try_join_all(tasks).await?;

    {
        let informer = lock_ptr!(informer)?;
        assert_eq!(informer.succeed, task_num * 2);
    }

    ssn_1.close().await?;
    ssn_2.close().await?;

    Ok(())
}

#[tokio::test]
async fn test_application_lifecycle() -> Result<(), FlameError> {
    let conn = flame::client::connect(FLAME_DEFAULT_ADDR).await?;

    let string_schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "string",
        "description": "The string for testing."
    });

    let apps = vec![
        (
            "my-test-agent-1".to_string(),
            ApplicationAttributes {
                shim: Shim::Host,
                image: Some("may-agent".to_string()),
                description: Some("This is my agent for testing.".to_string()),
                labels: vec!["test".to_string(), "agent".to_string()],
                command: Some("my-agent".to_string()),
                arguments: vec!["--test".to_string(), "--agent".to_string()],
                environments: HashMap::from([("TEST".to_string(), "true".to_string())]),
                working_directory: Some("/tmp".to_string()),
                max_instances: Some(10),
                delay_release: None,
                schema: Some(ApplicationSchema {
                    input: Some(string_schema.to_string()),
                    output: Some(string_schema.to_string()),
                    common_data: None,
                }),
            },
        ),
        (
            "empty-app".to_string(),
            ApplicationAttributes {
                shim: Shim::Host,
                image: None,
                description: None,
                labels: vec![],
                command: None,
                arguments: vec![],
                environments: HashMap::new(),
                working_directory: None,
                max_instances: None,
                delay_release: None,
                schema: None,
            },
        ),
    ];

    for (name, app_attr) in apps {
        conn.register_application(name.clone(), app_attr)
            .await
            .map_err(|e| {
                FlameError::Internal(format!("failed to register application <{name}>: {e}"))
            })?;
    }

    Ok(())
}
