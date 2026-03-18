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

//! Filesystem-based storage engine implementation.
//!
//! This module implements a high-performance storage engine that uses the filesystem
//! directly instead of a database. It uses a single-file architecture per session
//! for tasks to minimize filesystem metadata operations.
//!
//! # Architecture
//!
//! ```text
//! <work_dir>/data/
//! ├── sessions/<session_id>/
//! │   ├── metadata          # Session metadata (JSON)
//! │   ├── tasks.bin         # TaskMetadata records (fixed-size, indexed by Task ID)
//! │   ├── inputs.bin        # Concatenated input data (append-only)
//! │   └── outputs.bin       # Concatenated output data (append-only)
//! └── applications/<app_name>/
//!     └── metadata          # Application metadata (JSON)
//! ```
//!
//! # Design Decisions
//!
//! - **Fixed-size task metadata**: Enables O(1) random access by task ID
//! - **Append-only data files**: Maximizes write throughput for inputs/outputs
//! - **No file locks**: Relies on in-memory locks in the Session Manager
//! - **CRC32 checksums**: Detects corruption on read

use std::collections::HashMap;
use std::io::SeekFrom;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use bincode::{Decode, Encode};
use bytes::Bytes;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{Mutex, RwLock};

use common::apis::{
    Application, ApplicationAttributes, ApplicationID, ApplicationSchema, ApplicationState,
    Session, SessionAttributes, SessionID, SessionState, SessionStatus, Task, TaskGID, TaskID,
    TaskInput, TaskOutput, TaskResult, TaskState,
};
use common::{FlameError, FLAME_HOME};

use crate::storage::engine::{Engine, EnginePtr};

/// Task metadata stored in tasks.bin with fixed-size records.
///
/// Uses `bincode` with `fixint` encoding to ensure constant serialized size.
/// The checksum is calculated using `crc32fast` for data integrity.
#[derive(Encode, Decode, Debug, Clone, Default)]
struct TaskMetadata {
    /// Task ID (index in file)
    pub id: u64,
    /// Optimistic locking version
    pub version: u32,
    /// CRC32 checksum of the record (excluding this field)
    pub checksum: u32,
    /// Task state (TaskState enum as u8)
    pub state: u8,
    /// Unix timestamp of creation
    pub creation_time: i64,
    /// Unix timestamp of completion (0 if not completed)
    pub completion_time: i64,
    /// Offset in inputs.bin where input data starts
    pub input_offset: u64,
    /// Length of input data in bytes
    pub input_len: u64,
    /// Offset in outputs.bin where output data starts
    pub output_offset: u64,
    /// Length of output data in bytes
    pub output_len: u64,
}

/// Session metadata stored as JSON.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct SessionMetadata {
    pub id: String,
    pub application: String,
    pub slots: u32,
    pub version: u32,
    pub state: i32,
    pub creation_time: i64,
    pub completion_time: Option<i64>,
    pub min_instances: u32,
    pub max_instances: Option<u32>,
    /// Offset in common_data file (if any)
    pub common_data_len: u64,
}

/// Application metadata stored as JSON.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ApplicationMetadata {
    pub name: String,
    pub version: u32,
    pub state: i32,
    pub creation_time: i64,
    pub image: Option<String>,
    pub description: Option<String>,
    pub labels: Vec<String>,
    pub command: Option<String>,
    pub arguments: Vec<String>,
    pub environments: std::collections::HashMap<String, String>,
    pub working_directory: Option<String>,
    pub max_instances: u32,
    pub delay_release_seconds: i64,
    pub schema: Option<ApplicationSchemaMetadata>,
    pub url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ApplicationSchemaMetadata {
    pub input: Option<String>,
    pub output: Option<String>,
    pub common_data: Option<String>,
}

/// Bincode configuration for fixed-size encoding.
fn bincode_config() -> impl bincode::config::Config {
    bincode::config::standard()
        .with_fixed_int_encoding()
        .with_little_endian()
}

/// Calculate the fixed record size for TaskMetadata.
fn task_record_size() -> usize {
    // Calculate the serialized size of a default TaskMetadata
    let meta = TaskMetadata::default();
    bincode::encode_to_vec(&meta, bincode_config())
        .expect("Failed to calculate record size")
        .len()
}

/// Calculate CRC32 checksum for task metadata (excluding the checksum field itself).
fn calculate_checksum(meta: &TaskMetadata) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(&meta.id.to_le_bytes());
    hasher.update(&meta.version.to_le_bytes());
    hasher.update(&[meta.state]);
    hasher.update(&meta.creation_time.to_le_bytes());
    hasher.update(&meta.completion_time.to_le_bytes());
    hasher.update(&meta.input_offset.to_le_bytes());
    hasher.update(&meta.input_len.to_le_bytes());
    hasher.update(&meta.output_offset.to_le_bytes());
    hasher.update(&meta.output_len.to_le_bytes());
    hasher.finalize()
}

macro_rules! lock_ssn {
    ($self:expr, $ssn_id:expr) => {{
        let locks = $self.locks.read().await;
        let ssn_lock = locks
            .get($ssn_id)
            .ok_or_else(|| FlameError::NotFound(format!("Session lock not found: {}", $ssn_id)))?
            .clone();
        drop(locks);
        Ok::<_, FlameError>(ssn_lock.lock_owned().await)
    }};
}

macro_rules! lock_app {
    ($self:expr) => {
        $self.locks.write().await
    };
}

/// Filesystem-based storage engine.
pub struct FilesystemEngine {
    base_path: PathBuf,
    record_size: usize,
    locks: RwLock<HashMap<String, Arc<Mutex<()>>>>,
}

impl FilesystemEngine {
    /// Create a new filesystem engine from a URL.
    ///
    /// URL format: `filesystem://<path>` or `file://<path>`
    pub async fn new_ptr(url: &str) -> Result<EnginePtr, FlameError> {
        let path = Self::parse_url(url)?;

        // Create base directories
        let sessions_path = path.join("sessions");
        let applications_path = path.join("applications");

        fs::create_dir_all(&sessions_path).await.map_err(|e| {
            FlameError::Storage(format!("Failed to create sessions directory: {e}"))
        })?;
        fs::create_dir_all(&applications_path).await.map_err(|e| {
            FlameError::Storage(format!("Failed to create applications directory: {e}"))
        })?;

        let record_size = task_record_size();
        tracing::info!(
            "Filesystem storage engine initialized at {:?} with record size {}",
            path,
            record_size
        );

        Ok(Arc::new(FilesystemEngine {
            base_path: path,
            record_size,
            locks: RwLock::new(HashMap::new()),
        }))
    }

    /// Parse the storage URL to extract the base path.
    fn parse_url(url: &str) -> Result<PathBuf, FlameError> {
        let path = if let Some(p) = url.strip_prefix("filesystem://") {
            p
        } else if let Some(p) = url.strip_prefix("file://") {
            p
        } else if let Some(p) = url.strip_prefix("fs://") {
            p
        } else {
            return Err(FlameError::InvalidConfig(format!(
                "Invalid filesystem URL: {url}. Expected filesystem://, file://, or fs:// prefix"
            )));
        };

        if path.starts_with('/') {
            Ok(PathBuf::from(path))
        } else {
            let flame_home =
                std::env::var(FLAME_HOME).unwrap_or_else(|_| "/usr/local/flame".to_string());
            Ok(PathBuf::from(flame_home).join(path))
        }
    }

    fn session_path(&self, session_id: &str) -> PathBuf {
        self.base_path.join("sessions").join(session_id)
    }

    fn application_path(&self, app_name: &str) -> PathBuf {
        self.base_path.join("applications").join(app_name)
    }

    /// Read session metadata from disk.
    async fn read_session_metadata(&self, session_id: &str) -> Result<SessionMetadata, FlameError> {
        let path = self.session_path(session_id).join("metadata");
        let content = fs::read_to_string(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FlameError::NotFound(format!("Session {session_id} not found: {e}"))
            } else {
                FlameError::Storage(format!(
                    "Failed to read session metadata for {session_id}: {e}"
                ))
            }
        })?;
        serde_json::from_str(&content)
            .map_err(|e| FlameError::Storage(format!("Failed to parse session metadata: {e}")))
    }

    /// Write session metadata to disk atomically.
    async fn write_session_metadata(
        &self,
        session_id: &str,
        meta: &SessionMetadata,
    ) -> Result<(), FlameError> {
        let session_dir = self.session_path(session_id);
        let path = session_dir.join("metadata");
        let tmp_path = session_dir.join("metadata.tmp");

        let content = serde_json::to_string_pretty(meta).map_err(|e| {
            FlameError::Storage(format!("Failed to serialize session metadata: {e}"))
        })?;

        // Write to temp file first
        fs::write(&tmp_path, &content)
            .await
            .map_err(|e| FlameError::Storage(format!("Failed to write session metadata: {e}")))?;

        // Atomic rename
        fs::rename(&tmp_path, &path)
            .await
            .map_err(|e| FlameError::Storage(format!("Failed to rename session metadata: {e}")))?;

        Ok(())
    }

    /// Read application metadata from disk.
    async fn read_application_metadata(
        &self,
        app_name: &str,
    ) -> Result<ApplicationMetadata, FlameError> {
        let path = self.application_path(app_name).join("metadata");
        let content = fs::read_to_string(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                FlameError::NotFound(format!("Application {app_name} not found: {e}"))
            } else {
                FlameError::Storage(format!(
                    "Failed to read application metadata for {app_name}: {e}"
                ))
            }
        })?;
        serde_json::from_str(&content)
            .map_err(|e| FlameError::Storage(format!("Failed to parse application metadata: {e}")))
    }

    /// Write application metadata to disk atomically.
    async fn write_application_metadata(
        &self,
        app_name: &str,
        meta: &ApplicationMetadata,
    ) -> Result<(), FlameError> {
        let app_dir = self.application_path(app_name);
        fs::create_dir_all(&app_dir).await.map_err(|e| {
            FlameError::Storage(format!("Failed to create application directory: {e}"))
        })?;

        let path = app_dir.join("metadata");
        let tmp_path = app_dir.join("metadata.tmp");

        let content = serde_json::to_string_pretty(meta).map_err(|e| {
            FlameError::Storage(format!("Failed to serialize application metadata: {e}"))
        })?;

        // Write to temp file first
        fs::write(&tmp_path, &content).await.map_err(|e| {
            FlameError::Storage(format!("Failed to write application metadata: {e}"))
        })?;

        // Atomic rename
        fs::rename(&tmp_path, &path).await.map_err(|e| {
            FlameError::Storage(format!("Failed to rename application metadata: {e}"))
        })?;

        Ok(())
    }

    /// Read task metadata from tasks.bin.
    async fn read_task_metadata(
        &self,
        session_id: &str,
        task_id: TaskID,
    ) -> Result<TaskMetadata, FlameError> {
        if task_id < 1 {
            return Err(FlameError::NotFound(format!(
                "Invalid task ID: {task_id} (must be >= 1)"
            )));
        }

        let path = self.session_path(session_id).join("tasks.bin");

        let mut file = tokio::fs::OpenOptions::new()
            .read(true)
            .open(&path)
            .await
            .map_err(|e| FlameError::NotFound(format!("Tasks file not found: {e}")))?;

        let offset = (task_id as u64 - 1) * self.record_size as u64;
        file.seek(SeekFrom::Start(offset))
            .await
            .map_err(|e| FlameError::Storage(format!("Failed to seek to task {task_id}: {e}")))?;

        let mut buffer = vec![0u8; self.record_size];
        file.read_exact(&mut buffer)
            .await
            .map_err(|e| FlameError::NotFound(format!("Task {task_id} not found: {e}")))?;

        let (meta, _): (TaskMetadata, _) = bincode::decode_from_slice(&buffer, bincode_config())
            .map_err(|e| {
                FlameError::Storage(format!("Failed to deserialize task metadata: {e}"))
            })?;

        // Verify checksum
        let expected_checksum = calculate_checksum(&meta);
        if meta.checksum != expected_checksum {
            return Err(FlameError::Storage(format!(
                "Task {task_id} checksum mismatch: expected {expected_checksum}, got {}",
                meta.checksum
            )));
        }

        Ok(meta)
    }

    /// Write task metadata to tasks.bin at the specified offset.
    async fn write_task_metadata(
        &self,
        session_id: &str,
        meta: &TaskMetadata,
    ) -> Result<(), FlameError> {
        let path = self.session_path(session_id).join("tasks.bin");

        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .await
            .map_err(|e| FlameError::Storage(format!("Failed to open tasks file: {e}")))?;

        let offset = (meta.id - 1) * self.record_size as u64;
        file.seek(SeekFrom::Start(offset))
            .await
            .map_err(|e| FlameError::Storage(format!("Failed to seek to task {}: {e}", meta.id)))?;

        let buffer = bincode::encode_to_vec(meta, bincode_config())
            .map_err(|e| FlameError::Storage(format!("Failed to serialize task metadata: {e}")))?;

        file.write_all(&buffer)
            .await
            .map_err(|e| FlameError::Storage(format!("Failed to write task metadata: {e}")))?;

        file.sync_data()
            .await
            .map_err(|e| FlameError::Storage(format!("Failed to sync task metadata: {e}")))?;

        Ok(())
    }

    /// Append data to a file and return the offset where it was written.
    async fn append_data(
        &self,
        session_id: &str,
        filename: &str,
        data: &[u8],
    ) -> Result<u64, FlameError> {
        let path = self.session_path(session_id).join(filename);

        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(&path)
            .await
            .map_err(|e| FlameError::Storage(format!("Failed to open {filename}: {e}")))?;

        // Get current file size (this is where we'll write)
        let offset = file.seek(SeekFrom::End(0)).await.map_err(|e| {
            FlameError::Storage(format!("Failed to seek to end of {filename}: {e}"))
        })?;

        file.write_all(data)
            .await
            .map_err(|e| FlameError::Storage(format!("Failed to write to {filename}: {e}")))?;

        file.sync_data()
            .await
            .map_err(|e| FlameError::Storage(format!("Failed to sync {filename}: {e}")))?;

        Ok(offset)
    }

    /// Read data from a file at the specified offset and length.
    async fn read_data(
        &self,
        session_id: &str,
        filename: &str,
        offset: u64,
        len: u64,
    ) -> Result<Vec<u8>, FlameError> {
        if len == 0 {
            return Ok(Vec::new());
        }

        let path = self.session_path(session_id).join(filename);

        let mut file = tokio::fs::OpenOptions::new()
            .read(true)
            .open(&path)
            .await
            .map_err(|e| FlameError::Storage(format!("Failed to open {filename}: {e}")))?;

        file.seek(SeekFrom::Start(offset))
            .await
            .map_err(|e| FlameError::Storage(format!("Failed to seek in {filename}: {e}")))?;

        let mut buffer = vec![0u8; len as usize];
        file.read_exact(&mut buffer)
            .await
            .map_err(|e| FlameError::Storage(format!("Failed to read from {filename}: {e}")))?;

        Ok(buffer)
    }

    /// Get the number of tasks in a session by checking the tasks.bin file size.
    async fn get_task_count(&self, session_id: &str) -> Result<u64, FlameError> {
        let path = self.session_path(session_id).join("tasks.bin");

        match fs::metadata(&path).await {
            Ok(metadata) => Ok(metadata.len() / self.record_size as u64),
            Err(_) => Ok(0),
        }
    }

    /// Read common data for a session.
    async fn read_common_data(
        &self,
        session_id: &str,
        len: u64,
    ) -> Result<Option<Bytes>, FlameError> {
        if len == 0 {
            return Ok(None);
        }

        let path = self.session_path(session_id).join("common_data.bin");
        match fs::read(&path).await {
            Ok(data) => Ok(Some(Bytes::from(data))),
            Err(_) => Ok(None),
        }
    }

    /// Write common data for a session.
    async fn write_common_data(&self, session_id: &str, data: &[u8]) -> Result<(), FlameError> {
        let path = self.session_path(session_id).join("common_data.bin");
        fs::write(&path, data)
            .await
            .map_err(|e| FlameError::Storage(format!("Failed to write common data: {e}")))?;
        Ok(())
    }

    /// Convert TaskMetadata to Task.
    async fn task_from_metadata(
        &self,
        session_id: &str,
        meta: &TaskMetadata,
    ) -> Result<Task, FlameError> {
        let input = if meta.input_len > 0 {
            let data = self
                .read_data(session_id, "inputs.bin", meta.input_offset, meta.input_len)
                .await?;
            Some(Bytes::from(data))
        } else {
            None
        };

        let output = if meta.output_len > 0 {
            let data = self
                .read_data(
                    session_id,
                    "outputs.bin",
                    meta.output_offset,
                    meta.output_len,
                )
                .await?;
            Some(Bytes::from(data))
        } else {
            None
        };

        let state = TaskState::try_from(meta.state as i32)?;
        let completion_time = if meta.completion_time > 0 {
            DateTime::from_timestamp(meta.completion_time, 0)
        } else {
            None
        };

        Ok(Task {
            id: meta.id as TaskID,
            ssn_id: session_id.to_string(),
            version: meta.version,
            input,
            output,
            creation_time: DateTime::from_timestamp(meta.creation_time, 0)
                .ok_or_else(|| FlameError::Storage("Invalid creation time".to_string()))?,
            completion_time,
            events: Vec::new(), // Events are handled by EventManager
            state,
        })
    }

    /// Convert SessionMetadata to Session.
    async fn session_from_metadata(&self, meta: &SessionMetadata) -> Result<Session, FlameError> {
        let state = SessionState::try_from(meta.state)?;
        let common_data = self
            .read_common_data(&meta.id, meta.common_data_len)
            .await?;
        let completion_time = meta
            .completion_time
            .and_then(|t| DateTime::from_timestamp(t, 0));

        Ok(Session {
            id: meta.id.clone(),
            application: meta.application.clone(),
            slots: meta.slots,
            version: meta.version,
            common_data,
            tasks: std::collections::HashMap::new(),
            tasks_index: std::collections::HashMap::new(),
            creation_time: DateTime::from_timestamp(meta.creation_time, 0)
                .ok_or_else(|| FlameError::Storage("Invalid creation time".to_string()))?,
            completion_time,
            events: Vec::new(),
            status: SessionStatus { state },
            min_instances: meta.min_instances,
            max_instances: meta.max_instances,
        })
    }

    /// Convert ApplicationMetadata to Application.
    fn application_from_metadata(meta: &ApplicationMetadata) -> Result<Application, FlameError> {
        let state = ApplicationState::try_from(meta.state)?;
        let schema = meta.schema.as_ref().map(|s| ApplicationSchema {
            input: s.input.clone(),
            output: s.output.clone(),
            common_data: s.common_data.clone(),
        });

        Ok(Application {
            name: meta.name.clone(),
            version: meta.version,
            state,
            creation_time: DateTime::from_timestamp(meta.creation_time, 0)
                .ok_or_else(|| FlameError::Storage("Invalid creation time".to_string()))?,
            image: meta.image.clone(),
            description: meta.description.clone(),
            labels: meta.labels.clone(),
            command: meta.command.clone(),
            arguments: meta.arguments.clone(),
            environments: meta.environments.clone(),
            working_directory: meta.working_directory.clone(),
            max_instances: meta.max_instances,
            delay_release: Duration::seconds(meta.delay_release_seconds),
            schema,
            url: meta.url.clone(),
        })
    }

    /// Check if an application exists and is enabled.
    async fn check_application_enabled(&self, app_name: &str) -> Result<(), FlameError> {
        let meta = self.read_application_metadata(app_name).await?;
        if meta.state != ApplicationState::Enabled as i32 {
            return Err(FlameError::InvalidState(format!(
                "Application {app_name} is not enabled"
            )));
        }
        Ok(())
    }
}

#[async_trait]
impl Engine for FilesystemEngine {
    async fn register_application(
        &self,
        name: String,
        attr: ApplicationAttributes,
    ) -> Result<Application, FlameError> {
        let schema = attr.schema.map(|s| ApplicationSchemaMetadata {
            input: s.input,
            output: s.output,
            common_data: s.common_data,
        });

        let meta = ApplicationMetadata {
            name: name.clone(),
            version: 1,
            state: ApplicationState::Enabled as i32,
            creation_time: Utc::now().timestamp(),
            image: attr.image,
            description: attr.description,
            labels: attr.labels,
            command: attr.command,
            arguments: attr.arguments,
            environments: attr.environments,
            working_directory: attr.working_directory,
            max_instances: attr.max_instances,
            delay_release_seconds: attr.delay_release.num_seconds(),
            schema,
            url: attr.url,
        };

        self.write_application_metadata(&name, &meta).await?;
        Self::application_from_metadata(&meta)
    }

    async fn unregister_application(&self, name: String) -> Result<(), FlameError> {
        let _guard = lock_app!(self);

        let sessions_dir = self.base_path.join("sessions");
        if let Ok(mut entries) = fs::read_dir(&sessions_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let session_id = entry.file_name().to_string_lossy().to_string();
                if let Ok(meta) = self.read_session_metadata(&session_id).await {
                    if meta.application == name && meta.state == SessionState::Open as i32 {
                        return Err(FlameError::Storage(format!(
                            "Cannot unregister application '{}': has open sessions",
                            name
                        )));
                    }
                }
            }
        }

        let app_dir = self.application_path(&name);
        fs::remove_dir_all(&app_dir).await.map_err(|e| {
            FlameError::Storage(format!("Failed to delete application '{}': {e}", name))
        })?;

        Ok(())
    }

    async fn update_application(
        &self,
        name: String,
        attr: ApplicationAttributes,
    ) -> Result<Application, FlameError> {
        let _guard = lock_app!(self);

        let mut meta = self.read_application_metadata(&name).await?;

        let sessions_dir = self.base_path.join("sessions");
        if let Ok(mut entries) = fs::read_dir(&sessions_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let session_id = entry.file_name().to_string_lossy().to_string();
                if let Ok(ssn_meta) = self.read_session_metadata(&session_id).await {
                    if ssn_meta.application == name && ssn_meta.state == SessionState::Open as i32 {
                        return Err(FlameError::Storage(format!(
                            "Cannot update application '{}': has open sessions",
                            name
                        )));
                    }
                }
            }
        }

        let schema = attr.schema.map(|s| ApplicationSchemaMetadata {
            input: s.input,
            output: s.output,
            common_data: s.common_data,
        });

        meta.version += 1;
        meta.image = attr.image;
        meta.description = attr.description;
        meta.labels = attr.labels;
        meta.command = attr.command;
        meta.arguments = attr.arguments;
        meta.environments = attr.environments;
        meta.working_directory = attr.working_directory;
        meta.max_instances = attr.max_instances;
        meta.delay_release_seconds = attr.delay_release.num_seconds();
        meta.schema = schema;
        meta.url = attr.url;

        self.write_application_metadata(&name, &meta).await?;
        Self::application_from_metadata(&meta)
    }

    async fn get_application(&self, id: ApplicationID) -> Result<Application, FlameError> {
        let meta = self.read_application_metadata(&id).await?;
        Self::application_from_metadata(&meta)
    }

    async fn find_application(&self) -> Result<Vec<Application>, FlameError> {
        let mut apps = Vec::new();
        let apps_dir = self.base_path.join("applications");

        if let Ok(mut entries) = fs::read_dir(&apps_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let app_name = entry.file_name().to_string_lossy().to_string();
                if let Ok(meta) = self.read_application_metadata(&app_name).await {
                    if let Ok(app) = Self::application_from_metadata(&meta) {
                        apps.push(app);
                    }
                }
            }
        }

        Ok(apps)
    }

    async fn create_session(&self, attr: SessionAttributes) -> Result<Session, FlameError> {
        self.check_application_enabled(&attr.application).await?;

        {
            let mut locks = lock_app!(self);
            locks.insert(attr.id.clone(), Arc::new(Mutex::new(())));
        }

        let session_dir = self.session_path(&attr.id);
        fs::create_dir_all(&session_dir)
            .await
            .map_err(|e| FlameError::Storage(format!("Failed to create session directory: {e}")))?;

        let common_data_len = if let Some(ref data) = attr.common_data {
            self.write_common_data(&attr.id, data).await?;
            data.len() as u64
        } else {
            0
        };

        let meta = SessionMetadata {
            id: attr.id.clone(),
            application: attr.application.clone(),
            slots: attr.slots,
            version: 1,
            state: SessionState::Open as i32,
            creation_time: Utc::now().timestamp(),
            completion_time: None,
            min_instances: attr.min_instances,
            max_instances: attr.max_instances,
            common_data_len,
        };

        self.write_session_metadata(&attr.id, &meta).await?;

        let tasks_path = session_dir.join("tasks.bin");
        let inputs_path = session_dir.join("inputs.bin");
        let outputs_path = session_dir.join("outputs.bin");

        fs::write(&tasks_path, &[])
            .await
            .map_err(|e| FlameError::Storage(format!("Failed to create tasks.bin: {e}")))?;
        fs::write(&inputs_path, &[])
            .await
            .map_err(|e| FlameError::Storage(format!("Failed to create inputs.bin: {e}")))?;
        fs::write(&outputs_path, &[])
            .await
            .map_err(|e| FlameError::Storage(format!("Failed to create outputs.bin: {e}")))?;

        self.session_from_metadata(&meta).await
    }

    async fn get_session(&self, id: SessionID) -> Result<Session, FlameError> {
        let meta = self.read_session_metadata(&id).await?;
        self.session_from_metadata(&meta).await
    }

    async fn open_session(
        &self,
        id: SessionID,
        spec: Option<SessionAttributes>,
    ) -> Result<Session, FlameError> {
        // Try to get existing session
        match self.read_session_metadata(&id).await {
            Ok(meta) => {
                // Session exists - validate state
                if meta.state != SessionState::Open as i32 {
                    return Err(FlameError::InvalidState(format!(
                        "Session {id} is not open"
                    )));
                }

                // If spec provided, validate it matches
                if let Some(ref attr) = spec {
                    if meta.application != attr.application {
                        return Err(FlameError::InvalidConfig(format!(
                            "Session {id} spec mismatch: application differs"
                        )));
                    }
                    if meta.slots != attr.slots {
                        return Err(FlameError::InvalidConfig(format!(
                            "Session {id} spec mismatch: slots differs"
                        )));
                    }
                }

                self.session_from_metadata(&meta).await
            }
            Err(_) => {
                // Session doesn't exist
                match spec {
                    Some(attr) => self.create_session(attr).await,
                    None => Err(FlameError::NotFound(format!("Session {id} not found"))),
                }
            }
        }
    }

    async fn close_session(&self, id: SessionID) -> Result<Session, FlameError> {
        let mut meta = self.read_session_metadata(&id).await?;

        let task_count = self.get_task_count(&id).await?;
        for task_id in 1..=task_count {
            if let Ok(task_meta) = self.read_task_metadata(&id, task_id as TaskID).await {
                let state = match TaskState::try_from(task_meta.state as i32) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!(
                            "Task {}/{} has corrupted state ({}): {}, treating as incomplete",
                            id,
                            task_id,
                            task_meta.state,
                            e
                        );
                        return Err(FlameError::Storage(
                            "Cannot close session with corrupted task state".to_string(),
                        ));
                    }
                };
                if state != TaskState::Succeed && state != TaskState::Failed {
                    return Err(FlameError::Storage(
                        "Cannot close session with open tasks".to_string(),
                    ));
                }
            }
        }

        meta.state = SessionState::Closed as i32;
        meta.completion_time = Some(Utc::now().timestamp());
        meta.version += 1;

        self.write_session_metadata(&id, &meta).await?;
        self.session_from_metadata(&meta).await
    }

    async fn delete_session(&self, id: SessionID) -> Result<Session, FlameError> {
        let meta = self.read_session_metadata(&id).await?;

        if meta.state != SessionState::Closed as i32 {
            return Err(FlameError::Storage(
                "Cannot delete open session".to_string(),
            ));
        }

        let task_count = self.get_task_count(&id).await?;
        for task_id in 1..=task_count {
            if let Ok(task_meta) = self.read_task_metadata(&id, task_id as TaskID).await {
                let state = match TaskState::try_from(task_meta.state as i32) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!(
                            "Task {}/{} has corrupted state ({}): {}, treating as incomplete",
                            id,
                            task_id,
                            task_meta.state,
                            e
                        );
                        return Err(FlameError::Storage(
                            "Cannot delete session with corrupted task state".to_string(),
                        ));
                    }
                };
                if state != TaskState::Succeed && state != TaskState::Failed {
                    return Err(FlameError::Storage(
                        "Cannot delete session with open tasks".to_string(),
                    ));
                }
            }
        }

        let session = self.session_from_metadata(&meta).await?;

        let session_dir = self.session_path(&id);
        fs::remove_dir_all(&session_dir)
            .await
            .map_err(|e| FlameError::Storage(format!("Failed to delete session: {e}")))?;

        {
            let mut locks = lock_app!(self);
            locks.remove(&id);
        }

        Ok(session)
    }

    async fn find_session(&self) -> Result<Vec<Session>, FlameError> {
        let mut sessions = Vec::new();
        let sessions_dir = self.base_path.join("sessions");

        if let Ok(mut entries) = fs::read_dir(&sessions_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let session_id = entry.file_name().to_string_lossy().to_string();
                if let Ok(meta) = self.read_session_metadata(&session_id).await {
                    if let Ok(session) = self.session_from_metadata(&meta).await {
                        sessions.push(session);
                    }
                }
            }
        }

        Ok(sessions)
    }

    async fn create_task(
        &self,
        ssn_id: SessionID,
        input: Option<TaskInput>,
    ) -> Result<Task, FlameError> {
        let ssn_meta = self.read_session_metadata(&ssn_id).await?;
        if ssn_meta.state != SessionState::Open as i32 {
            return Err(FlameError::InvalidState(
                "Cannot create task in closed session".to_string(),
            ));
        }

        let _guard = lock_ssn!(self, &ssn_id)?;

        let task_count = self.get_task_count(&ssn_id).await?;
        let task_id = task_count + 1;

        let (input_offset, input_len) = if let Some(ref data) = input {
            let offset = self.append_data(&ssn_id, "inputs.bin", data).await?;
            (offset, data.len() as u64)
        } else {
            (0, 0)
        };

        let mut meta = TaskMetadata {
            id: task_id,
            version: 1,
            checksum: 0,
            state: TaskState::Pending as u8,
            creation_time: Utc::now().timestamp(),
            completion_time: 0,
            input_offset,
            input_len,
            output_offset: 0,
            output_len: 0,
        };

        meta.checksum = calculate_checksum(&meta);

        self.write_task_metadata(&ssn_id, &meta).await?;

        self.task_from_metadata(&ssn_id, &meta).await
    }

    async fn get_task(&self, gid: TaskGID) -> Result<Task, FlameError> {
        let _guard = lock_ssn!(self, &gid.ssn_id)?;
        let meta = self.read_task_metadata(&gid.ssn_id, gid.task_id).await?;
        self.task_from_metadata(&gid.ssn_id, &meta).await
    }

    async fn retry_task(&self, gid: TaskGID) -> Result<Task, FlameError> {
        let _guard = lock_ssn!(self, &gid.ssn_id)?;

        let mut meta = self.read_task_metadata(&gid.ssn_id, gid.task_id).await?;

        meta.state = TaskState::Pending as u8;
        meta.version += 1;
        meta.checksum = calculate_checksum(&meta);

        self.write_task_metadata(&gid.ssn_id, &meta).await?;
        self.task_from_metadata(&gid.ssn_id, &meta).await
    }

    async fn delete_task(&self, gid: TaskGID) -> Result<Task, FlameError> {
        // In append-only filesystem architecture, physical deletion is not supported.
        // The task data remains in the append-only files (inputs.bin, outputs.bin).
        // Callers should use close_session + delete_session to clean up entire sessions.
        Err(FlameError::Storage(format!(
            "Task deletion not supported in filesystem storage engine (task {}/{}). \
             Use session deletion to clean up completed sessions.",
            gid.ssn_id, gid.task_id
        )))
    }

    async fn update_task_state(
        &self,
        gid: TaskGID,
        task_state: TaskState,
        _message: Option<String>,
    ) -> Result<Task, FlameError> {
        let _guard = lock_ssn!(self, &gid.ssn_id)?;

        let mut meta = self.read_task_metadata(&gid.ssn_id, gid.task_id).await?;

        meta.state = task_state as u8;
        meta.version += 1;

        if task_state == TaskState::Succeed || task_state == TaskState::Failed {
            meta.completion_time = Utc::now().timestamp();
        }

        meta.checksum = calculate_checksum(&meta);

        self.write_task_metadata(&gid.ssn_id, &meta).await?;
        self.task_from_metadata(&gid.ssn_id, &meta).await
    }

    async fn update_task_result(
        &self,
        gid: TaskGID,
        task_result: TaskResult,
    ) -> Result<Task, FlameError> {
        let _guard = lock_ssn!(self, &gid.ssn_id)?;

        let mut meta = self.read_task_metadata(&gid.ssn_id, gid.task_id).await?;

        if let Some(ref output) = task_result.output {
            let offset = self.append_data(&gid.ssn_id, "outputs.bin", output).await?;
            meta.output_offset = offset;
            meta.output_len = output.len() as u64;
        }

        meta.state = task_result.state as u8;
        meta.version += 1;

        if task_result.state == TaskState::Succeed || task_result.state == TaskState::Failed {
            meta.completion_time = Utc::now().timestamp();
        }

        meta.checksum = calculate_checksum(&meta);

        self.write_task_metadata(&gid.ssn_id, &meta).await?;
        self.task_from_metadata(&gid.ssn_id, &meta).await
    }

    async fn find_tasks(&self, ssn_id: SessionID) -> Result<Vec<Task>, FlameError> {
        let _guard = lock_ssn!(self, &ssn_id)?;

        let mut tasks = Vec::new();
        let task_count = self.get_task_count(&ssn_id).await?;

        for task_id in 1..=task_count {
            if let Ok(meta) = self.read_task_metadata(&ssn_id, task_id as TaskID).await {
                if let Ok(task) = self.task_from_metadata(&ssn_id, &meta).await {
                    tasks.push(task);
                }
            }
        }

        Ok(tasks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_engine() -> (FilesystemEngine, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let url = format!("filesystem://{}", temp_dir.path().display());
        let _engine_ptr = FilesystemEngine::new_ptr(&url).await.unwrap();

        let engine = FilesystemEngine {
            base_path: temp_dir.path().to_path_buf(),
            record_size: task_record_size(),
            locks: RwLock::new(HashMap::new()),
        };

        (engine, temp_dir)
    }

    #[tokio::test]
    async fn test_record_size_is_constant() {
        let size1 = task_record_size();
        let size2 = task_record_size();
        assert_eq!(size1, size2);

        // Verify different metadata values produce same size
        let meta1 = TaskMetadata::default();
        let meta2 = TaskMetadata {
            id: u64::MAX,
            version: u32::MAX,
            checksum: u32::MAX,
            state: 255,
            creation_time: i64::MAX,
            completion_time: i64::MAX,
            input_offset: u64::MAX,
            input_len: u64::MAX,
            output_offset: u64::MAX,
            output_len: u64::MAX,
        };

        let buf1 = bincode::encode_to_vec(&meta1, bincode_config()).unwrap();
        let buf2 = bincode::encode_to_vec(&meta2, bincode_config()).unwrap();

        assert_eq!(buf1.len(), buf2.len());
    }

    #[tokio::test]
    async fn test_checksum_calculation() {
        let meta = TaskMetadata {
            id: 1,
            version: 1,
            checksum: 0,
            state: TaskState::Pending as u8,
            creation_time: 1234567890,
            completion_time: 0,
            input_offset: 0,
            input_len: 100,
            output_offset: 0,
            output_len: 0,
        };

        let checksum1 = calculate_checksum(&meta);
        let checksum2 = calculate_checksum(&meta);

        assert_eq!(checksum1, checksum2);

        // Different metadata should produce different checksum
        let meta2 = TaskMetadata { id: 2, ..meta };

        let checksum3 = calculate_checksum(&meta2);
        assert_ne!(checksum1, checksum3);
    }

    #[tokio::test]
    async fn test_url_parsing() {
        std::env::set_var("FLAME_HOME", "/opt/flame");

        let path1 = FilesystemEngine::parse_url("filesystem:///var/lib/flame").unwrap();
        assert_eq!(path1, PathBuf::from("/var/lib/flame"));

        let path2 = FilesystemEngine::parse_url("file:///tmp/flame").unwrap();
        assert_eq!(path2, PathBuf::from("/tmp/flame"));

        let path3 = FilesystemEngine::parse_url("fs:///data").unwrap();
        assert_eq!(path3, PathBuf::from("/data"));

        let path4 = FilesystemEngine::parse_url("fs://data").unwrap();
        assert_eq!(path4, PathBuf::from("/opt/flame/data"));

        let path5 = FilesystemEngine::parse_url("filesystem://data/sessions").unwrap();
        assert_eq!(path5, PathBuf::from("/opt/flame/data/sessions"));

        let path6 = FilesystemEngine::parse_url("file://storage").unwrap();
        assert_eq!(path6, PathBuf::from("/opt/flame/storage"));

        std::env::remove_var("FLAME_HOME");

        let err = FilesystemEngine::parse_url("sqlite:///tmp/flame.db");
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn test_application_lifecycle() {
        let (engine, _temp_dir) = create_test_engine().await;

        // Register application
        let attr = ApplicationAttributes {
            image: Some("test-image".to_string()),
            description: Some("Test application".to_string()),
            labels: vec!["test".to_string()],
            command: Some("/bin/test".to_string()),
            arguments: vec!["--arg1".to_string()],
            environments: std::collections::HashMap::new(),
            working_directory: Some("/tmp".to_string()),
            max_instances: 10,
            delay_release: Duration::seconds(60),
            schema: None,
            url: None,
        };

        let app = engine
            .register_application("test-app".to_string(), attr.clone())
            .await
            .unwrap();
        assert_eq!(app.name, "test-app");
        assert_eq!(app.state, ApplicationState::Enabled);

        // Get application
        let app2 = engine
            .get_application("test-app".to_string())
            .await
            .unwrap();
        assert_eq!(app2.name, "test-app");

        // Find applications
        let apps = engine.find_application().await.unwrap();
        assert_eq!(apps.len(), 1);

        // Update application
        let updated_attr = ApplicationAttributes {
            description: Some("Updated description".to_string()),
            ..attr
        };
        let app3 = engine
            .update_application("test-app".to_string(), updated_attr)
            .await
            .unwrap();
        assert_eq!(app3.description, Some("Updated description".to_string()));
        assert_eq!(app3.version, 2);

        // Unregister application
        engine
            .unregister_application("test-app".to_string())
            .await
            .unwrap();

        // Verify it's gone
        let result = engine.get_application("test-app".to_string()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_session_lifecycle() {
        let (engine, _temp_dir) = create_test_engine().await;

        // First register an application
        let app_attr = ApplicationAttributes {
            image: None,
            description: None,
            labels: vec![],
            command: Some("/bin/test".to_string()),
            arguments: vec![],
            environments: std::collections::HashMap::new(),
            working_directory: None,
            max_instances: 10,
            delay_release: Duration::seconds(0),
            schema: None,
            url: None,
        };
        engine
            .register_application("test-app".to_string(), app_attr)
            .await
            .unwrap();

        // Create session
        let ssn_attr = SessionAttributes {
            id: "test-session".to_string(),
            application: "test-app".to_string(),
            slots: 1,
            common_data: Some(Bytes::from("test data")),
            min_instances: 0,
            max_instances: None,
        };

        let session = engine.create_session(ssn_attr).await.unwrap();
        assert_eq!(session.id, "test-session");
        assert_eq!(session.status.state, SessionState::Open);

        // Get session
        let session2 = engine
            .get_session("test-session".to_string())
            .await
            .unwrap();
        assert_eq!(session2.id, "test-session");

        // Find sessions
        let sessions = engine.find_session().await.unwrap();
        assert_eq!(sessions.len(), 1);

        // Close session (should work since no tasks)
        let closed = engine
            .close_session("test-session".to_string())
            .await
            .unwrap();
        assert_eq!(closed.status.state, SessionState::Closed);

        // Delete session
        let deleted = engine
            .delete_session("test-session".to_string())
            .await
            .unwrap();
        assert_eq!(deleted.id, "test-session");
    }

    #[tokio::test]
    async fn test_task_lifecycle() {
        let (engine, _temp_dir) = create_test_engine().await;

        // Setup: register app and create session
        let app_attr = ApplicationAttributes {
            image: None,
            description: None,
            labels: vec![],
            command: Some("/bin/test".to_string()),
            arguments: vec![],
            environments: std::collections::HashMap::new(),
            working_directory: None,
            max_instances: 10,
            delay_release: Duration::seconds(0),
            schema: None,
            url: None,
        };
        engine
            .register_application("test-app".to_string(), app_attr)
            .await
            .unwrap();

        let ssn_attr = SessionAttributes {
            id: "test-session".to_string(),
            application: "test-app".to_string(),
            slots: 1,
            common_data: None,
            min_instances: 0,
            max_instances: None,
        };
        engine.create_session(ssn_attr).await.unwrap();

        // Create task with input
        let input = Bytes::from("test input data");
        let task = engine
            .create_task("test-session".to_string(), Some(input.clone()))
            .await
            .unwrap();
        assert_eq!(task.id, 1);
        assert_eq!(task.state, TaskState::Pending);
        assert_eq!(task.input, Some(input));

        // Get task
        let gid = TaskGID {
            ssn_id: "test-session".to_string(),
            task_id: 1,
        };
        let task2 = engine.get_task(gid.clone()).await.unwrap();
        assert_eq!(task2.id, 1);

        // Update task state
        let task3 = engine
            .update_task_state(gid.clone(), TaskState::Running, None)
            .await
            .unwrap();
        assert_eq!(task3.state, TaskState::Running);

        // Update task result
        let output = Bytes::from("test output data");
        let result = TaskResult {
            state: TaskState::Succeed,
            output: Some(output.clone()),
            message: None,
        };
        let task4 = engine
            .update_task_result(gid.clone(), result)
            .await
            .unwrap();
        assert_eq!(task4.state, TaskState::Succeed);
        assert_eq!(task4.output, Some(output));

        // Find tasks
        let tasks = engine.find_tasks("test-session".to_string()).await.unwrap();
        assert_eq!(tasks.len(), 1);

        // Create another task
        let task5 = engine
            .create_task("test-session".to_string(), None)
            .await
            .unwrap();
        assert_eq!(task5.id, 2);

        // Complete second task
        let gid2 = TaskGID {
            ssn_id: "test-session".to_string(),
            task_id: 2,
        };
        engine
            .update_task_state(gid2, TaskState::Succeed, None)
            .await
            .unwrap();

        // Now we can close the session
        let closed = engine
            .close_session("test-session".to_string())
            .await
            .unwrap();
        assert_eq!(closed.status.state, SessionState::Closed);
    }
}
