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

pub mod apis;
pub mod ctx;
pub mod ptr;
pub mod trace;

use serde_json::json;
use std::collections::HashMap;
use thiserror::Error;
use tonic::Status;
use tracing_subscriber::filter::{FromEnvError, ParseError};

use crate::apis::{ApplicationAttributes, ApplicationSchema, Shim};

#[derive(Error, Debug)]
pub enum FlameError {
    #[error("'{0}' not found")]
    NotFound(String),

    #[error("{0}")]
    Internal(String),

    #[error("{0}")]
    Network(String),

    #[error("{0}")]
    InvalidConfig(String),

    #[error("'{0}' is not initialized")]
    Uninitialized(String),

    #[error("{0}")]
    InvalidState(String),

    #[error("{0}")]
    Storage(String),
}

impl From<FlameError> for Status {
    fn from(value: FlameError) -> Self {
        match value {
            FlameError::NotFound(s) => Status::not_found(s),
            FlameError::Internal(s) => Status::internal(s),
            _ => Status::unknown(value.to_string()),
        }
    }
}

impl From<Status> for FlameError {
    fn from(value: Status) -> Self {
        FlameError::Network(value.message().to_string())
    }
}

impl From<ParseError> for FlameError {
    fn from(value: ParseError) -> Self {
        FlameError::InvalidConfig(value.to_string())
    }
}

impl From<FromEnvError> for FlameError {
    fn from(value: FromEnvError) -> Self {
        FlameError::InvalidConfig(value.to_string())
    }
}

#[macro_export]
macro_rules! lock_ptr {
    ( $mutex_arc:expr ) => {
        $mutex_arc
            .lock()
            .map_err(|_| FlameError::Internal("mutex ptr".to_string()))
    };
}

#[macro_export]
macro_rules! lock_async_ptr {
    ( $mutex_arc:expr ) => {
        $mutex_arc
            .lock()
            .await
            .map_err(|_| FlameError::Internal("mutex ptr".to_string()))
    };
}

pub const FLAME_EXECUTOR_ID: &str = "FLAME_EXECUTOR_ID";
pub const FLAME_WORKING_DIRECTORY: &str = "/tmp";

pub fn init_logger() -> Result<(), FlameError> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()?
        .add_directive("h2=error".parse()?)
        .add_directive("hyper_util=error".parse()?)
        .add_directive("tower=error".parse()?);
    // Initialize tracing with a custom format
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_timer(tracing_subscriber::fmt::time::LocalTime::rfc_3339())
        .with_target(true)
        .with_thread_ids(true)
        // .with_process_ids(true)
        .init();

    Ok(())
}

pub fn default_applications() -> HashMap<String, ApplicationAttributes> {
    let script_input_schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "properties": {
            "language": {
                "type": "string",
                "description": "The language of the script, e.g. python"
            },
            "code": {
                "type": "string",
                "description": "The code of the script to run, e.g. print('Hello, world!')"
            },
            "input": {
                "type": "array",
                "items": {
                    "type": "integer",
                    "description": "The input to the script in bytes, e.g. [0x1, 0x2]"
                }
            }
        },
        "required": [
            "language",
            "code"
        ]
    });

    let script_output_schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "string",
        "description": "The output of the script in UTF-8."
    });

    HashMap::from([
        (
            "flmexec".to_string(),
            ApplicationAttributes {
                shim: Shim::Host,
                description: Some(
                    "The Flame Executor application, which is used to run scripts.".to_string(),
                ),
                command: Some("/usr/local/flame/bin/flmexec-service".to_string()),
                schema: Some(ApplicationSchema {
                    input: Some(script_input_schema.to_string()),
                    output: Some(script_output_schema.to_string()),
                    ..ApplicationSchema::default()
                }),
                ..ApplicationAttributes::default()
            },
        ),
        (
            "flmping".to_string(),
            ApplicationAttributes {
                shim: Shim::Host,
                command: Some("/usr/local/flame/bin/flmping-service".to_string()),
                ..ApplicationAttributes::default()
            },
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::Code;

    #[test]
    fn test_from_flame_error_to_status() {
        let error = FlameError::NotFound("test".to_string());
        let status = Status::from(error);
        assert_eq!(status.code(), Code::NotFound);
        assert_eq!(status.message(), "test");

        let error = FlameError::Internal("test".to_string());
        let status = Status::from(error);
        assert_eq!(status.code(), Code::Internal);
        assert_eq!(status.message(), "test");

        let error = FlameError::Network("test".to_string());
        let status = Status::from(error);
        assert_eq!(status.code(), Code::Unknown);
        assert_eq!(status.message(), "test");
    }
}
