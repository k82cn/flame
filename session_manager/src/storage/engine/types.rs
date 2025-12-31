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

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{types::Json, FromRow};
use std::collections::HashMap;

use crate::FlameError;
use bytes::Bytes;
use common::apis::{
    Application, ApplicationSchema, ApplicationState, Session, SessionStatus, Shim, Task,
};
use common::apis::{ApplicationID, Event, SessionID, TaskID};

#[derive(Clone, FromRow, Debug)]
pub struct EventDao {
    pub owner: String,
    pub parent: Option<String>,
    pub code: i32,
    pub message: Option<String>,
    pub creation_time: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppSchemaDao {
    pub input: Option<String>,
    pub output: Option<String>,
    pub common_data: Option<String>,
}

#[derive(Clone, FromRow, Debug)]
pub struct ApplicationDao {
    pub name: ApplicationID,
    pub version: u32,
    pub image: Option<String>,
    pub description: Option<String>,
    pub labels: Option<Json<Vec<String>>>,
    pub command: Option<String>,
    pub arguments: Option<Json<Vec<String>>>,
    pub environments: Option<Json<HashMap<String, String>>>,
    pub working_directory: Option<String>,
    pub max_instances: i64,
    pub delay_release: i64,
    pub schema: Option<Json<AppSchemaDao>>,

    pub shim: i32,
    pub creation_time: i64,
    pub state: i32,
}

#[derive(Clone, FromRow, Debug)]
pub struct SessionDao {
    pub id: SessionID,
    pub application: String,
    pub slots: i64,
    pub version: u32,

    pub common_data: Option<Vec<u8>>,
    pub creation_time: i64,
    pub completion_time: Option<i64>,

    pub state: i32,
}

#[derive(Clone, FromRow, Debug)]
pub struct TaskDao {
    pub id: TaskID,
    pub ssn_id: SessionID,
    pub version: u32,
    pub input: Option<Vec<u8>>,
    pub output: Option<Vec<u8>>,

    pub creation_time: i64,
    pub completion_time: Option<i64>,

    pub state: i32,
}

impl TryFrom<&SessionDao> for Session {
    type Error = FlameError;

    fn try_from(ssn: &SessionDao) -> Result<Self, Self::Error> {
        Ok(Self {
            id: ssn.id.clone(),
            application: ssn.application.clone(),
            slots: ssn.slots as u32,
            version: ssn.version,
            common_data: ssn.common_data.clone().map(Bytes::from),
            creation_time: DateTime::<Utc>::from_timestamp(ssn.creation_time, 0)
                .ok_or(FlameError::Storage("invalid creation time".to_string()))?,
            completion_time: ssn
                .completion_time
                .map(|t| {
                    DateTime::<Utc>::from_timestamp(t, 0)
                        .ok_or(FlameError::Storage("invalid completion time".to_string()))
                })
                .transpose()?,
            tasks: HashMap::new(),
            tasks_index: HashMap::new(),
            status: SessionStatus {
                state: ssn.state.try_into()?,
            },
            events: vec![],
        })
    }
}

impl TryFrom<SessionDao> for Session {
    type Error = FlameError;

    fn try_from(ssn: SessionDao) -> Result<Self, Self::Error> {
        Session::try_from(&ssn)
    }
}

impl TryFrom<&TaskDao> for Task {
    type Error = FlameError;

    fn try_from(task: &TaskDao) -> Result<Self, Self::Error> {
        Ok(Self {
            id: task.id,
            ssn_id: task.ssn_id.clone(),
            version: task.version,
            input: task.input.clone().map(Bytes::from),
            output: task.output.clone().map(Bytes::from),

            creation_time: DateTime::<Utc>::from_timestamp(task.creation_time, 0)
                .ok_or(FlameError::Storage("invalid creation time".to_string()))?,
            completion_time: task
                .completion_time
                .map(|t| {
                    DateTime::<Utc>::from_timestamp(t, 0)
                        .ok_or(FlameError::Storage("invalid completion time".to_string()))
                })
                .transpose()?,

            state: task.state.try_into()?,
            events: vec![],
        })
    }
}

impl TryFrom<TaskDao> for Task {
    type Error = FlameError;

    fn try_from(ssn: TaskDao) -> Result<Self, Self::Error> {
        Task::try_from(&ssn)
    }
}

impl TryFrom<&ApplicationDao> for Application {
    type Error = FlameError;

    fn try_from(app: &ApplicationDao) -> Result<Self, Self::Error> {
        tracing::debug!("Application Shim is {}", app.shim);

        Ok(Self {
            name: app.name.clone(),
            version: app.version,
            state: ApplicationState::try_from(app.state)?,
            shim: Shim::try_from(app.shim)
                .map_err(|_| FlameError::Internal("unknown shim".to_string()))?,
            creation_time: DateTime::<Utc>::from_timestamp(app.creation_time, 0)
                .ok_or(FlameError::Storage("invalid creation time".to_string()))?,
            image: app.image.clone(),
            description: app.description.clone(),
            labels: app
                .labels
                .clone()
                .map(|labels| labels.0)
                .unwrap_or_default(),
            command: app.command.clone(),
            arguments: app.arguments.clone().map(|args| args.0).unwrap_or_default(),
            environments: app
                .environments
                .clone()
                .map(|envs| envs.0)
                .unwrap_or_default(),
            working_directory: app.working_directory.clone().unwrap_or("/tmp".to_string()),
            max_instances: app.max_instances as u32,
            delay_release: Duration::seconds(app.delay_release),
            schema: app.schema.clone().map(|arg| arg.0.into()),
        })
    }
}

impl TryFrom<ApplicationDao> for Application {
    type Error = FlameError;

    fn try_from(ssn: ApplicationDao) -> Result<Self, Self::Error> {
        Application::try_from(&ssn)
    }
}

impl From<ApplicationSchema> for AppSchemaDao {
    fn from(schema: ApplicationSchema) -> Self {
        Self {
            input: schema.input,
            output: schema.output,
            common_data: schema.common_data,
        }
    }
}

impl From<AppSchemaDao> for ApplicationSchema {
    fn from(schema: AppSchemaDao) -> Self {
        Self {
            input: schema.input,
            output: schema.output,
            common_data: schema.common_data,
        }
    }
}

impl TryFrom<EventDao> for Event {
    type Error = FlameError;

    fn try_from(event: EventDao) -> Result<Self, Self::Error> {
        Ok(Self {
            code: event.code,
            message: event.message.clone(),
            creation_time: DateTime::<Utc>::from_timestamp(event.creation_time, 0)
                .ok_or(FlameError::Storage("invalid creation time".to_string()))?,
        })
    }
}
