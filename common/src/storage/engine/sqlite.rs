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

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time;

use async_trait::async_trait;
use bytes::Bytes;
#[cfg(test)]
use chrono::Duration;
use chrono::Utc;
#[cfg(test)]
use serde_json::json;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    types::Json,
    QueryBuilder, Sqlite, SqliteConnection, SqlitePool,
};
use stdng::trace_fn;

use crate::{
    apis::{
        Application, ApplicationAttributes, ApplicationPath, ApplicationSchema, ApplicationState,
        CommonData, Event, ExecutorID, ExecutorState, Node, Session, SessionAttributes,
        SessionPath, SessionState, SessionStatus, Shim, Task, TaskGID, TaskInput, TaskName,
        TaskOptions, TaskOutput, TaskResult, TaskState, Workspace, DEFAULT_DELAY_RELEASE,
        DEFAULT_MAX_INSTANCES, DEFAULT_WORKSPACE,
    },
    FlameError,
};

use crate::apis::{ApplicationFilter, Executor, SessionFilter, TaskFilter};
use crate::storage::engine::types::{
    AppSchemaDao, ApplicationDao, ExecutorDao, NodeDao, SessionDao, TaskDao,
};

use crate::storage::engine::{Engine, EnginePtr};

const SQLITE_SQL: &str = "migrations/sqlite";

pub struct SqliteEngine {
    pool: SqlitePool,
    catalog: Option<Arc<WorkspaceCatalog>>,
    workspace: Option<String>,
}

struct WorkspaceCatalog {
    root: PathBuf,
    pools: tokio::sync::Mutex<HashMap<String, SqlitePool>>,
}

impl SqliteEngine {
    pub async fn new_ptr(url: &str) -> Result<EnginePtr, FlameError> {
        let root = Self::storage_root(url)?;
        std::fs::create_dir_all(&root)?;
        let control_pool = Self::connect_pool(&root.join("flame.db")).await?;
        let engine = Self {
            pool: control_pool,
            catalog: Some(Arc::new(WorkspaceCatalog {
                root,
                pools: tokio::sync::Mutex::new(HashMap::new()),
            })),
            workspace: None,
        };
        match engine.create_workspace(DEFAULT_WORKSPACE.to_string()).await {
            Ok(_) | Err(FlameError::AlreadyExist(_)) => {}
            Err(error) => return Err(error),
        }
        for workspace in engine.list_workspaces().await? {
            engine.workspace_engine(&workspace.name).await?;
        }
        Ok(Arc::new(engine))
    }

    pub(crate) fn storage_root(url: &str) -> Result<PathBuf, FlameError> {
        let path = url
            .strip_prefix("sqlite://")
            .ok_or_else(|| FlameError::InvalidConfig(format!("invalid SQLite URL <{url}>")))?;
        if path.is_empty() {
            return Err(FlameError::InvalidConfig("empty SQLite path".into()));
        }
        let path = PathBuf::from(path);
        if path.extension().is_some_and(|extension| extension == "db") {
            Ok(path.with_extension(""))
        } else {
            Ok(path)
        }
    }

    async fn connect_pool(path: &Path) -> Result<SqlitePool, FlameError> {
        let url = format!("sqlite://{}", path.display());
        tracing::debug!("Try to create and connect to {}", url);

        let options = SqliteConnectOptions::from_str(&url)
            .map_err(|e| FlameError::Storage(e.to_string()))?
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            .busy_timeout(time::Duration::from_secs(15))
            .synchronous(SqliteSynchronous::Normal)
            .create_if_missing(true);

        let db = SqlitePoolOptions::new()
            .max_connections(50)
            .min_connections(3)
            .acquire_timeout(time::Duration::from_secs(30))
            .idle_timeout(time::Duration::from_secs(5 * 60))
            .max_lifetime(time::Duration::from_secs(30 * 60))
            .test_before_acquire(true)
            .connect_with(options)
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let installed_migrations = std::path::Path::new(SQLITE_SQL);
        let source_migrations = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(SQLITE_SQL);
        let migrations = if installed_migrations.exists() {
            installed_migrations
        } else {
            source_migrations.as_path()
        };
        let migrator = sqlx::migrate::Migrator::new(migrations)
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;
        migrator
            .run(&db)
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        Ok(db)
    }

    async fn workspace_engine(&self, workspace: &str) -> Result<Self, FlameError> {
        let catalog = self
            .catalog
            .as_ref()
            .ok_or_else(|| FlameError::Internal("missing workspace catalog".into()))?;
        crate::apis::validate_path_segment(workspace)?;
        let dir = catalog.root.join(workspace);
        if !dir.join("metadata").is_file() {
            return Err(FlameError::NotFound(format!("workspace <{workspace}>")));
        }
        let mut pools = catalog.pools.lock().await;
        let pool = if let Some(pool) = pools.get(workspace) {
            pool.clone()
        } else {
            let pool = Self::connect_pool(&dir.join("flame.db")).await?;
            pools.insert(workspace.to_string(), pool.clone());
            pool
        };
        Ok(Self {
            pool,
            catalog: None,
            workspace: Some(workspace.to_string()),
        })
    }

    fn local_app_id(&self, gid: &str) -> Result<String, FlameError> {
        let (workspace, app) = crate::apis::parse_application_path(gid)?;
        if self.workspace.as_deref() != Some(workspace) {
            return Err(FlameError::InvalidConfig(format!(
                "application <{gid}> does not belong to this workspace"
            )));
        }
        Ok(app.to_string())
    }

    fn local_session_name(&self, gid: &str) -> Result<String, FlameError> {
        let (workspace, session) = crate::apis::parse_session_path(gid)?;
        if self.workspace.as_deref() != Some(workspace) {
            return Err(FlameError::InvalidConfig(format!(
                "session <{gid}> does not belong to this workspace"
            )));
        }
        Ok(session.to_string())
    }

    fn local_session_attr(&self, attr: SessionAttributes) -> Result<SessionAttributes, FlameError> {
        if self.workspace.as_deref() != Some(&attr.workspace) {
            return Err(FlameError::InvalidConfig(format!(
                "session workspace <{}> does not belong to this database",
                attr.workspace
            )));
        }
        crate::apis::session_path(&attr.workspace, &attr.name)?;
        crate::apis::application_path(&attr.workspace, &attr.application)?;
        Ok(attr)
    }

    fn local_task_gid(&self, gid: TaskGID) -> Result<TaskGID, FlameError> {
        if self.workspace.as_deref() != Some(&gid.workspace) {
            return Err(FlameError::InvalidConfig(format!(
                "task workspace <{}> does not belong to this database",
                gid.workspace
            )));
        }
        Ok(gid)
    }

    fn global_app(&self, mut app: Application) -> Application {
        if let Some(workspace) = &self.workspace {
            app.gid = format!("{workspace}/{}", app.gid);
        }
        app
    }

    fn global_session(&self, mut session: Session) -> Session {
        if let Some(workspace) = &self.workspace {
            session.gid = format!("{workspace}/{}", session.gid);
            session.application = format!("{workspace}/{}", session.application);
        }
        session
    }

    fn global_task(&self, mut task: Task) -> Task {
        if let Some(workspace) = &self.workspace {
            task.session = format!("{workspace}/{}", task.session);
        }
        task
    }

    async fn application_engine(&self, path: &str) -> Result<Self, FlameError> {
        let (workspace, _) = crate::apis::parse_application_path(path)?;
        self.workspace_engine(workspace).await
    }

    async fn session_engine(&self, path: &str) -> Result<Self, FlameError> {
        let (workspace, _) = crate::apis::parse_session_path(path)?;
        self.workspace_engine(workspace).await
    }

    async fn _count_task(
        &self,
        tx: &mut SqliteConnection,
        filter: &TaskFilter,
    ) -> Result<i64, FlameError> {
        if filter.states.as_ref().is_some_and(Vec::is_empty) {
            return Ok(0);
        }

        let mut query = QueryBuilder::<Sqlite>::new("SELECT count(*) FROM tasks WHERE session=");
        query.push_bind(&filter.session);
        if let Some(states) = &filter.states {
            query.push(" AND state IN (");
            let mut values = query.separated(", ");
            for state in states {
                values.push_bind(*state as i32);
            }
            values.push_unseparated(")");
        }

        let count: i64 = query
            .build_query_scalar()
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(format!("failed to count tasks: {e}")))?;
        Ok(count)
    }

    async fn _delete_session(
        &self,
        tx: &mut SqliteConnection,
        id: SessionPath,
    ) -> Result<Session, FlameError> {
        let sql = "DELETE FROM tasks WHERE session=?";
        sqlx::query(sql)
            .bind(id.clone())
            .execute(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(format!("failed to delete session: {e}")))?;

        let sql = "DELETE FROM sessions WHERE name=? AND state=? RETURNING *";
        let ssn: SessionDao = sqlx::query_as(sql)
            .bind(id.clone())
            .bind(SessionState::Closed as i32)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(format!("failed to close session: {e}")))?;

        let ssn: Session = ssn.try_into()?;

        Ok(ssn)
    }

    async fn _count_sessions(
        &self,
        tx: &mut SqliteConnection,
        filter: &SessionFilter,
    ) -> Result<i64, FlameError> {
        if filter.predicate.is_some() {
            return Err(FlameError::Storage(
                "session predicates cannot be evaluated by SQLite".to_string(),
            ));
        }
        if filter.ids.as_ref().is_some_and(Vec::is_empty) {
            return Ok(0);
        }

        let mut query = QueryBuilder::<Sqlite>::new("SELECT count(*) FROM sessions");
        let mut has_condition = false;
        if let Some(application) = &filter.application {
            query.push(" WHERE application=").push_bind(application);
            has_condition = true;
        }
        if let Some(state) = filter.state {
            query.push(if has_condition {
                " AND state="
            } else {
                " WHERE state="
            });
            query.push_bind(state as i32);
            has_condition = true;
        }
        if let Some(ids) = &filter.ids {
            query.push(if has_condition {
                " AND name IN ("
            } else {
                " WHERE name IN ("
            });
            let mut values = query.separated(", ");
            for id in ids {
                values.push_bind(id);
            }
            values.push_unseparated(")");
        }

        let count: i64 = query
            .build_query_scalar()
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(format!("failed to count sessions: {e}")))?;
        Ok(count)
    }

    async fn _delete_application(
        &self,
        tx: &mut SqliteConnection,
        name: String,
    ) -> Result<(), FlameError> {
        let sql = "DELETE FROM applications WHERE name=?";
        sqlx::query(sql)
            .bind(name)
            .execute(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(format!("failed to delete application: {e}")))?;
        Ok(())
    }

    /// Internal helper to get session within an existing transaction.
    /// Returns None if session not found.
    async fn _get_session(
        tx: &mut SqliteConnection,
        id: SessionPath,
    ) -> Result<Option<Session>, FlameError> {
        let sql = "SELECT * FROM sessions WHERE name=?";
        let ssn: Option<SessionDao> = sqlx::query_as(sql)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        match ssn {
            Some(dao) => Ok(Some(dao.try_into()?)),
            None => Ok(None),
        }
    }

    /// Internal helper to create session within an existing transaction.
    async fn _create_session(
        tx: &mut SqliteConnection,
        attr: SessionAttributes,
    ) -> Result<Session, FlameError> {
        let common_data: Option<Vec<u8>> = attr.common_data.map(Bytes::into);
        let (resreq_cpu, resreq_memory, resreq_gpu) = match &attr.resreq {
            Some(r) => (
                Some(r.cpu as i64),
                Some(r.memory as i64),
                Some(r.gpu as i64),
            ),
            None => (None, None, None),
        };
        let sql = r#"INSERT INTO sessions (id, name, application, common_data, creation_time, state, min_instances, max_instances, batch_size, priority, resreq_cpu, resreq_memory, resreq_gpu)
            VALUES (
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?,
                ?
            )
            RETURNING *"#;
        let ssn: SessionDao = sqlx::query_as(sql)
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(attr.name)
            .bind(attr.application)
            .bind(common_data)
            .bind(Utc::now().timestamp())
            .bind(SessionState::Open as i32)
            .bind(attr.min_instances as i64)
            .bind(attr.max_instances.map(|v| v as i64))
            .bind(1_i64)
            .bind(attr.priority as i64)
            .bind(resreq_cpu)
            .bind(resreq_memory)
            .bind(resreq_gpu)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        ssn.try_into()
    }
}

#[async_trait]
impl Engine for SqliteEngine {
    async fn create_workspace(&self, name: String) -> Result<Workspace, FlameError> {
        crate::apis::validate_path_segment(&name)?;
        let catalog = self
            .catalog
            .as_ref()
            .ok_or_else(|| FlameError::Internal("missing workspace catalog".into()))?;
        let dir = catalog.root.join(&name);
        if dir.exists() {
            return Err(FlameError::AlreadyExist(format!("workspace <{name}>")));
        }
        // Publish the workspace directory only after its database and catalog
        // marker are ready. An interrupted creation leaves an ignored staging
        // directory instead of an invisible, uncreatable workspace.
        let staging = catalog
            .root
            .join(format!(".creating-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&staging)?;
        let now = Utc::now();
        let pool = match Self::connect_pool(&staging.join("flame.db")).await {
            Ok(pool) => pool,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&staging);
                return Err(error);
            }
        };
        if let Err(error) = std::fs::write(staging.join("metadata"), now.timestamp().to_string()) {
            pool.close().await;
            let _ = std::fs::remove_dir_all(&staging);
            return Err(FlameError::Storage(error.to_string()));
        }
        pool.close().await;
        if let Err(error) = std::fs::rename(&staging, &dir) {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(if dir.exists() {
                FlameError::AlreadyExist(format!("workspace <{name}>"))
            } else {
                FlameError::Storage(error.to_string())
            });
        }
        let pool = Self::connect_pool(&dir.join("flame.db")).await?;
        catalog.pools.lock().await.insert(name.clone(), pool);
        Ok(Workspace {
            name,
            creation_time: now,
        })
    }

    async fn list_workspaces(&self) -> Result<Vec<Workspace>, FlameError> {
        let catalog = self
            .catalog
            .as_ref()
            .ok_or_else(|| FlameError::Internal("missing workspace catalog".into()))?;
        let mut workspaces = Vec::new();
        for entry in std::fs::read_dir(&catalog.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            if entry
                .file_name()
                .to_string_lossy()
                .starts_with(".creating-")
            {
                continue;
            }
            if !entry.path().join("metadata").is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            crate::apis::validate_path_segment(&name)?;
            let timestamp = std::fs::read_to_string(entry.path().join("metadata"))?
                .parse::<i64>()
                .map_err(|error| FlameError::Storage(error.to_string()))?;
            let creation_time = chrono::DateTime::from_timestamp(timestamp, 0)
                .ok_or_else(|| FlameError::Storage("invalid workspace timestamp".into()))?;
            workspaces.push(Workspace {
                name,
                creation_time,
            });
        }
        workspaces.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(workspaces)
    }

    async fn register_application(
        &self,
        name: String,
        attr: ApplicationAttributes,
    ) -> Result<Application, FlameError> {
        if self.catalog.is_some() {
            let id = crate::apis::resolve_application_path(&name, &attr.id)?;
            let engine = self.application_engine(&id).await?;
            let mut attr = attr;
            attr.id.clear();
            return engine
                .register_application(name, attr)
                .await
                .map(|app| engine.global_app(app));
        }

        trace_fn!("Sqlite::register_application");

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(format!("failed to begin TX: {e}")))?;

        let schema: Option<Json<AppSchemaDao>> =
            attr.schema.clone().map(AppSchemaDao::from).map(Json);
        crate::apis::validate_path_segment(&name)?;

        let sql = r#"INSERT INTO applications
            (
                id,
                name,
                shim,
                image,
                description, 
                labels, 
                command, 
                arguments, 
                environments, 
                working_directory, 
                max_instances, 
                delay_release, 
                schema, 
                url,
                installer,
                creation_time, 
                state)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING *"#;
        let app: ApplicationDao = sqlx::query_as(sql)
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(name)
            .bind(attr.shim as i32)
            .bind(attr.image)
            .bind(attr.description)
            .bind(Json(attr.labels))
            .bind(attr.command)
            .bind(Json(attr.arguments))
            .bind(Json(attr.environments))
            .bind(attr.working_directory)
            .bind(attr.max_instances)
            .bind(attr.delay_release.num_seconds())
            .bind(schema)
            .bind(attr.url)
            .bind(attr.installer)
            .bind(Utc::now().timestamp())
            .bind(ApplicationState::Enabled as i32)
            .fetch_one(&mut *tx)
            .await
            .map_err(|error| {
                if error
                    .as_database_error()
                    .is_some_and(|error| error.is_unique_violation())
                {
                    FlameError::AlreadyExist("application already exists".to_string())
                } else {
                    FlameError::Storage(format!("failed to register application: {error}"))
                }
            })?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(format!("failed to commit TX: {e}")))?;

        Ok(app.try_into()?)
    }

    async fn update_application(
        &self,
        name: String,
        attr: ApplicationAttributes,
    ) -> Result<Application, FlameError> {
        if self.catalog.is_some() {
            let engine = self.application_engine(&name).await?;
            let local = engine.local_app_id(&name)?;
            return engine
                .update_application(local, attr)
                .await
                .map(|app| engine.global_app(app));
        }

        trace_fn!("Sqlite::update_application");

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(format!("failed to begin TX: {e}")))?;

        let state: i32 = sqlx::query_scalar("SELECT state FROM applications WHERE name=?")
            .bind(&name)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => {
                    FlameError::NotFound(format!("application <{name}> not found"))
                }
                _ => FlameError::Storage(e.to_string()),
            })?;
        if state != ApplicationState::Enabled as i32 {
            return Err(FlameError::InvalidState(format!(
                "application <{name}> is not enabled"
            )));
        }

        let filter = SessionFilter::by_application_state(name.clone(), SessionState::Open);
        let count = self._count_sessions(&mut tx, &filter).await?;
        if count > 0 {
            return Err(FlameError::Storage(format!(
                "{count} open sessions in the application"
            )));
        }

        let schema: Option<Json<AppSchemaDao>> =
            attr.schema.clone().map(AppSchemaDao::from).map(Json);

        let sql = r#"UPDATE applications
                    SET shim=?,
                        image=?,
                        schema=?,
                        description=?,
                        labels=?,
                        command=?,
                        arguments=?,
                        environments=?,
                        working_directory=?,
                        max_instances=?,
                        delay_release=?,
                        url=?,
                        installer=?,
                        version=version+1
                    WHERE name=? AND state=?
                    RETURNING *"#;

        let app: ApplicationDao = sqlx::query_as(sql)
            .bind(attr.shim as i32)
            .bind(attr.image)
            .bind(schema)
            .bind(attr.description)
            .bind(Json(attr.labels))
            .bind(attr.command)
            .bind(Json(attr.arguments))
            .bind(Json(attr.environments))
            .bind(attr.working_directory)
            .bind(attr.max_instances)
            .bind(attr.delay_release.num_seconds())
            .bind(attr.url)
            .bind(attr.installer)
            .bind(&name)
            .bind(ApplicationState::Enabled as i32)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => {
                    FlameError::InvalidState(format!("application <{name}> is not enabled"))
                }
                _ => FlameError::Storage(format!("failed to update application: {e}")),
            })?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(format!("failed to commit TX: {e}")))?;

        Ok(app.try_into()?)
    }

    async fn update_application_state(
        &self,
        name: ApplicationPath,
        state: ApplicationState,
    ) -> Result<Application, FlameError> {
        if self.catalog.is_some() {
            let engine = self.application_engine(&name).await?;
            let local = engine.local_app_id(&name)?;
            return engine
                .update_application_state(local, state)
                .await
                .map(|app| engine.global_app(app));
        }

        trace_fn!("Sqlite::update_application_state");

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(format!("failed to begin TX: {e}")))?;

        let current: ApplicationDao = sqlx::query_as("SELECT * FROM applications WHERE name=?")
            .bind(&name)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => {
                    FlameError::NotFound(format!("application <{name}> not found"))
                }
                _ => FlameError::Storage(e.to_string()),
            })?;

        if current.state == state as i32 {
            tx.commit()
                .await
                .map_err(|e| FlameError::Storage(format!("failed to commit TX: {e}")))?;
            return current.try_into();
        }

        let updated: ApplicationDao = sqlx::query_as(
            "UPDATE applications SET state=?, version=version+1 WHERE name=? RETURNING *",
        )
        .bind(state as i32)
        .bind(&name)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| FlameError::Storage(format!("failed to update application state: {e}")))?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(format!("failed to commit TX: {e}")))?;

        updated.try_into()
    }

    async fn delete_application(&self, name: ApplicationPath) -> Result<(), FlameError> {
        if self.catalog.is_some() {
            let engine = self.application_engine(&name).await?;
            return engine.delete_application(engine.local_app_id(&name)?).await;
        }

        trace_fn!("Sqlite::delete_application");

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(format!("failed to begin TX: {e}")))?;

        let state: i32 = sqlx::query_scalar("SELECT state FROM applications WHERE name=?")
            .bind(&name)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => {
                    FlameError::NotFound(format!("application <{name}> not found"))
                }
                _ => FlameError::Storage(e.to_string()),
            })?;
        if state != ApplicationState::Disabled as i32 {
            return Err(FlameError::InvalidState(format!(
                "application <{name}> is not disabled"
            )));
        }

        let filter = SessionFilter::by_application(name.clone());
        let count = self._count_sessions(&mut tx, &filter).await?;
        if count > 0 {
            return Err(FlameError::InvalidState(format!(
                "application <{name}> still has {count} sessions"
            )));
        }

        self._delete_application(&mut tx, name).await?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(format!("failed to delete application: {e}")))?;

        Ok(())
    }

    async fn get_application(&self, id: ApplicationPath) -> Result<Application, FlameError> {
        if self.catalog.is_some() {
            let engine = self.application_engine(&id).await?;
            return engine
                .get_application(engine.local_app_id(&id)?)
                .await
                .map(|app| engine.global_app(app));
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let sql = "SELECT * FROM applications WHERE name=?";
        let app: ApplicationDao = sqlx::query_as(sql)
            .bind(&id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => {
                    FlameError::NotFound(format!("application <{id}> not found"))
                }
                _ => FlameError::Storage(e.to_string()),
            })?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(format!("failed to get application: {e}")))?;

        app.try_into()
    }

    async fn find_applications(
        &self,
        filter: Option<&ApplicationFilter>,
    ) -> Result<Vec<Application>, FlameError> {
        if self.catalog.is_some() {
            let mut applications = Vec::new();
            for workspace in self.list_workspaces().await? {
                let engine = self.workspace_engine(&workspace.name).await?;
                applications.extend(
                    engine
                        .find_applications(filter)
                        .await?
                        .into_iter()
                        .map(|app| engine.global_app(app)),
                );
            }
            return Ok(applications);
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let app: Vec<ApplicationDao> = match filter.and_then(|filter| filter.state) {
            Some(state) => {
                sqlx::query_as("SELECT * FROM applications WHERE state=?")
                    .bind(state as i32)
                    .fetch_all(&mut *tx)
                    .await
            }
            None => {
                sqlx::query_as("SELECT * FROM applications")
                    .fetch_all(&mut *tx)
                    .await
            }
        }
        .map_err(|e| FlameError::Storage(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        Ok(app
            .iter()
            .map(Application::try_from)
            .filter_map(Result::ok)
            .collect())
    }

    async fn create_session(&self, attr: SessionAttributes) -> Result<Session, FlameError> {
        if self.catalog.is_some() {
            let engine = self.session_engine(&attr.gid()?).await?;
            let local = engine.local_session_attr(attr)?;
            return engine
                .create_session(local)
                .await
                .map(|session| engine.global_session(session));
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let ssn = Self::_create_session(&mut tx, attr).await?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        Ok(ssn)
    }

    async fn get_session(&self, id: SessionPath) -> Result<Session, FlameError> {
        if self.catalog.is_some() {
            let engine = self.session_engine(&id).await?;
            return engine
                .get_session(engine.local_session_name(&id)?)
                .await
                .map(|session| engine.global_session(session));
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let ssn = Self::_get_session(&mut tx, id.clone())
            .await?
            .ok_or_else(|| FlameError::NotFound(format!("session <{id}> not found")))?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        Ok(ssn)
    }

    async fn open_session(
        &self,
        id: SessionPath,
        spec: Option<SessionAttributes>,
    ) -> Result<Session, FlameError> {
        if self.catalog.is_some() {
            let engine = self.session_engine(&id).await?;
            let local_id = engine.local_session_name(&id)?;
            let local_spec = spec
                .map(|attr| engine.local_session_attr(attr))
                .transpose()?;
            return engine
                .open_session(local_id, local_spec)
                .await
                .map(|session| engine.global_session(session));
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let ssn = match Self::_get_session(&mut tx, id.clone()).await? {
            Some(session) => {
                // Session exists - validate state
                if session.status.state != SessionState::Open {
                    return Err(FlameError::InvalidState(format!(
                        "session <{id}> is not open"
                    )));
                }
                // If spec provided, validate it matches
                if let Some(ref attr) = spec {
                    session.validate_spec(attr)?;
                }
                session
            }
            None => {
                // Session doesn't exist
                match spec {
                    Some(attr) => Self::_create_session(&mut tx, attr).await?,
                    None => return Err(FlameError::NotFound(format!("session <{id}> not found"))),
                }
            }
        };

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        Ok(ssn)
    }

    async fn delete_session(&self, id: SessionPath) -> Result<Session, FlameError> {
        if self.catalog.is_some() {
            let engine = self.session_engine(&id).await?;
            return engine
                .delete_session(engine.local_session_name(&id)?)
                .await
                .map(|session| engine.global_session(session));
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let filter = TaskFilter::non_terminal(&id);
        let count = self._count_task(&mut tx, &filter).await?;
        if count > 0 {
            return Err(FlameError::Storage(format!(
                "{count} open tasks in the session"
            )));
        }

        let ssn = self._delete_session(&mut tx, id).await?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        Ok(ssn)
    }

    async fn close_session(&self, id: SessionPath) -> Result<Session, FlameError> {
        if self.catalog.is_some() {
            let engine = self.session_engine(&id).await?;
            return engine
                .close_session(engine.local_session_name(&id)?)
                .await
                .map(|session| engine.global_session(session));
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let check_running_sql = "SELECT COUNT(*) as cnt FROM tasks WHERE session=? AND state=?";
        let running_count: (i32,) = sqlx::query_as(check_running_sql)
            .bind(id.clone())
            .bind(TaskState::Running as i32)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        if running_count.0 > 0 {
            return Err(FlameError::Storage(
                "Cannot close session with running tasks".to_string(),
            ));
        }

        let cancel_pending_sql =
            "UPDATE tasks SET state=?, completion_time=? WHERE session=? AND state=?";
        sqlx::query(cancel_pending_sql)
            .bind(TaskState::Cancelled as i32)
            .bind(Utc::now().timestamp())
            .bind(id.clone())
            .bind(TaskState::Pending as i32)
            .execute(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let close_session_sql = r#"UPDATE sessions 
            SET state=?, completion_time=?, version=version+1
            WHERE name=?
            RETURNING *"#;
        let ssn: SessionDao = sqlx::query_as(close_session_sql)
            .bind(SessionState::Closed as i32)
            .bind(Utc::now().timestamp())
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        ssn.try_into()
    }

    async fn find_sessions(&self) -> Result<Vec<Session>, FlameError> {
        if self.catalog.is_some() {
            let mut sessions = Vec::new();
            for workspace in self.list_workspaces().await? {
                let engine = self.workspace_engine(&workspace.name).await?;
                sessions.extend(
                    engine
                        .find_sessions()
                        .await?
                        .into_iter()
                        .map(|session| engine.global_session(session)),
                );
            }
            return Ok(sessions);
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let ssn: Vec<SessionDao> = sqlx::query_as("SELECT * FROM sessions")
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        Ok(ssn
            .iter()
            .map(Session::try_from)
            .filter_map(Result::ok)
            .collect())
    }

    async fn create_task(
        &self,
        session: SessionPath,
        input: Option<TaskInput>,
        options: Option<TaskOptions>,
    ) -> Result<Task, FlameError> {
        if self.catalog.is_some() {
            let engine = self.session_engine(&session).await?;
            let local = engine.local_session_name(&session)?;
            return engine
                .create_task(local, input, options)
                .await
                .map(|task| engine.global_task(task));
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let input: Option<Vec<u8>> = input.map(Bytes::into);
        let affinity = serde_json::to_string(
            &options
                .unwrap_or_default()
                .affinity
                .iter()
                .map(|key| key.to_vec())
                .collect::<Vec<_>>(),
        )
        .map_err(|e| FlameError::Storage(e.to_string()))?;
        let sql = r#"INSERT INTO tasks (id, number, session, input, affinity, creation_time, state)
            VALUES (
                ?,
                COALESCE((SELECT MAX(number)+1 FROM tasks WHERE session=?), 1),
                (SELECT name FROM sessions WHERE name=? AND state=?),
                ?,
                ?,
                ?,
                ?)
            RETURNING *"#;
        let task: TaskDao = sqlx::query_as(sql)
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(session.clone())
            .bind(session)
            .bind(SessionState::Open as i32)
            .bind(input)
            .bind(affinity)
            .bind(Utc::now().timestamp())
            .bind(TaskState::Pending as i32)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        task.try_into()
    }

    async fn get_task(&self, gid: TaskGID) -> Result<Task, FlameError> {
        if self.catalog.is_some() {
            let engine = self.session_engine(&gid.session_path()?).await?;
            return engine
                .get_task(engine.local_task_gid(gid)?)
                .await
                .map(|task| engine.global_task(task));
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let sql = r#"SELECT * FROM tasks WHERE number=? AND session=?"#;
        let task: TaskDao = sqlx::query_as(sql)
            .bind(gid.task)
            .bind(gid.session)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        task.try_into()
    }

    async fn retry_task(&self, gid: TaskGID) -> Result<Task, FlameError> {
        if self.catalog.is_some() {
            let engine = self.session_engine(&gid.session_path()?).await?;
            return engine
                .retry_task(engine.local_task_gid(gid)?)
                .await
                .map(|task| engine.global_task(task));
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let sql = r#"UPDATE tasks SET state=?, version=version+1 WHERE number=? AND session=? RETURNING *"#;
        let task: TaskDao = sqlx::query_as(sql)
            .bind(TaskState::Pending as i32)
            .bind(gid.task)
            .bind(gid.session)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        task.try_into()
    }

    async fn update_task_state(
        &self,
        gid: TaskGID,
        task_state: TaskState,
        message: Option<String>,
    ) -> Result<Task, FlameError> {
        if self.catalog.is_some() {
            let engine = self.session_engine(&gid.session_path()?).await?;
            return engine
                .update_task_state(engine.local_task_gid(gid)?, task_state, message)
                .await
                .map(|task| engine.global_task(task));
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let completion_time = match task_state {
            TaskState::Failed | TaskState::Succeed | TaskState::Cancelled => {
                Some(Utc::now().timestamp())
            }
            _ => None,
        };

        let sql = r#"UPDATE tasks SET state=?, completion_time=?, version=version+1 WHERE number=? AND session=? RETURNING *"#;
        let task: TaskDao = sqlx::query_as(sql)
            .bind::<i32>(task_state.into())
            .bind(completion_time)
            .bind(gid.task)
            .bind(gid.session)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        task.try_into()
    }

    async fn update_task_result(
        &self,
        gid: TaskGID,
        task_result: TaskResult,
    ) -> Result<Task, FlameError> {
        if self.catalog.is_some() {
            let engine = self.session_engine(&gid.session_path()?).await?;
            return engine
                .update_task_result(engine.local_task_gid(gid)?, task_result)
                .await
                .map(|task| engine.global_task(task));
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let completion_time = match task_result.state {
            TaskState::Failed | TaskState::Succeed => Some(Utc::now().timestamp()),
            _ => {
                tracing::warn!(
                    "Invalid task state <{:?}> for task <{}> when updating task result",
                    task_result.state,
                    gid
                );
                None
            }
        };

        let sql = r#"UPDATE tasks SET state=?, completion_time=?, output=?, version=version+1 WHERE number=? AND session=? RETURNING *"#;

        let task: TaskDao = sqlx::query_as(sql)
            .bind::<i32>(task_result.state.into())
            .bind(completion_time)
            .bind::<Option<Vec<u8>>>(task_result.output.map(Bytes::into))
            .bind(gid.task)
            .bind(gid.session)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        task.try_into()
    }

    async fn find_tasks(&self, session: SessionPath) -> Result<Vec<Task>, FlameError> {
        if self.catalog.is_some() {
            let engine = self.session_engine(&session).await?;
            return engine
                .find_tasks(engine.local_session_name(&session)?)
                .await
                .map(|tasks| {
                    tasks
                        .into_iter()
                        .map(|task| engine.global_task(task))
                        .collect()
                });
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let sql = "SELECT * FROM tasks WHERE session=?";
        let task_list: Vec<TaskDao> = sqlx::query_as(sql)
            .bind(session)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let tasks: Vec<Task> = task_list
            .iter()
            .map(Task::try_from)
            .filter_map(Result::ok)
            .collect();

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        Ok(tasks)
    }

    // Node operations

    async fn create_node(&self, node: &Node) -> Result<Node, FlameError> {
        trace_fn!("Sqlite::create_node");

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let now = Utc::now().timestamp();
        let sql = r#"INSERT INTO nodes 
            (name, state, capacity_cpu, capacity_memory, capacity_gpu,
             allocatable_cpu, allocatable_memory, allocatable_gpu,
             info_arch, info_os, creation_time, last_heartbeat)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING *"#;

        let dao: NodeDao = sqlx::query_as(sql)
            .bind(&node.name)
            .bind(i32::from(node.state))
            .bind(node.capacity.cpu as i64)
            .bind(node.capacity.memory as i64)
            .bind(node.capacity.gpu as i64)
            .bind(node.allocatable.cpu as i64)
            .bind(node.allocatable.memory as i64)
            .bind(node.allocatable.gpu as i64)
            .bind(&node.info.arch)
            .bind(&node.info.os)
            .bind(now)
            .bind(now)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(format!("failed to create node: {e}")))?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        dao.try_into()
    }

    async fn get_node(&self, name: &str) -> Result<Option<Node>, FlameError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let sql = "SELECT * FROM nodes WHERE name=?";
        let dao: Option<NodeDao> = sqlx::query_as(sql)
            .bind(name)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        match dao {
            Some(d) => Ok(Some(d.try_into()?)),
            None => Ok(None),
        }
    }

    async fn update_node(&self, node: &Node) -> Result<Node, FlameError> {
        trace_fn!("Sqlite::update_node");

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let sql = r#"UPDATE nodes 
            SET state=?, capacity_cpu=?, capacity_memory=?, capacity_gpu=?,
                allocatable_cpu=?, allocatable_memory=?, allocatable_gpu=?,
                info_arch=?, info_os=?, last_heartbeat=?
            WHERE name=?
            RETURNING *"#;

        let dao: NodeDao = sqlx::query_as(sql)
            .bind(i32::from(node.state))
            .bind(node.capacity.cpu as i64)
            .bind(node.capacity.memory as i64)
            .bind(node.capacity.gpu as i64)
            .bind(node.allocatable.cpu as i64)
            .bind(node.allocatable.memory as i64)
            .bind(node.allocatable.gpu as i64)
            .bind(&node.info.arch)
            .bind(&node.info.os)
            .bind(Utc::now().timestamp())
            .bind(&node.name)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(format!("failed to update node: {e}")))?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        dao.try_into()
    }

    async fn delete_node(&self, name: &str) -> Result<(), FlameError> {
        trace_fn!("Sqlite::delete_node");

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        // Note: executors are automatically deleted via ON DELETE CASCADE foreign key constraint
        let sql = "DELETE FROM nodes WHERE name=?";
        sqlx::query(sql)
            .bind(name)
            .execute(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(format!("failed to delete node: {e}")))?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        Ok(())
    }

    async fn find_nodes(&self) -> Result<Vec<Node>, FlameError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let sql = "SELECT * FROM nodes";
        let daos: Vec<NodeDao> = sqlx::query_as(sql)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        Ok(daos
            .iter()
            .map(Node::try_from)
            .filter_map(Result::ok)
            .collect())
    }

    // Executor operations

    async fn create_executor(&self, executor: &Executor) -> Result<Executor, FlameError> {
        trace_fn!("Sqlite::create_executor");

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let sql = r#"INSERT INTO executors
            (id, node, application, resreq_cpu, resreq_memory, resreq_gpu, shim, task, session, creation_time, state)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING *"#;

        let dao: ExecutorDao = sqlx::query_as(sql)
            .bind(&executor.id)
            .bind(&executor.node)
            .bind(&executor.application)
            .bind(executor.resreq.cpu as i64)
            .bind(executor.resreq.memory as i64)
            .bind(executor.resreq.gpu as i64)
            .bind(i32::from(executor.shim))
            .bind(executor.task)
            .bind(&executor.session)
            .bind(executor.creation_time.timestamp())
            .bind(i32::from(executor.state))
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(format!("failed to create executor: {e}")))?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        dao.try_into()
    }

    async fn get_executor(&self, id: &ExecutorID) -> Result<Option<Executor>, FlameError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let sql = "SELECT * FROM executors WHERE id=?";
        let dao: Option<ExecutorDao> = sqlx::query_as(sql)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        match dao {
            Some(d) => Ok(Some(d.try_into()?)),
            None => Ok(None),
        }
    }

    async fn update_executor(&self, executor: &Executor) -> Result<Executor, FlameError> {
        trace_fn!("Sqlite::update_executor");

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let sql = r#"UPDATE executors
            SET node=?, application=?, resreq_cpu=?, resreq_memory=?, resreq_gpu=?, shim=?,
                task=?, session=?, state=?
            WHERE id=?
            RETURNING *"#;

        let dao: ExecutorDao = sqlx::query_as(sql)
            .bind(&executor.node)
            .bind(&executor.application)
            .bind(executor.resreq.cpu as i64)
            .bind(executor.resreq.memory as i64)
            .bind(executor.resreq.gpu as i64)
            .bind(i32::from(executor.shim))
            .bind(executor.task)
            .bind(&executor.session)
            .bind(i32::from(executor.state))
            .bind(&executor.id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(format!("failed to update executor: {e}")))?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        dao.try_into()
    }

    async fn update_executor_state(
        &self,
        id: &ExecutorID,
        state: ExecutorState,
    ) -> Result<Executor, FlameError> {
        trace_fn!("Sqlite::update_executor_state");

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let sql = r#"UPDATE executors SET state=? WHERE id=? RETURNING *"#;

        let dao: ExecutorDao = sqlx::query_as(sql)
            .bind(i32::from(state))
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(format!("failed to update executor state: {e}")))?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        dao.try_into()
    }

    async fn delete_executor(&self, id: &ExecutorID) -> Result<(), FlameError> {
        trace_fn!("Sqlite::delete_executor");

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let sql = "DELETE FROM executors WHERE id=?";
        sqlx::query(sql)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| FlameError::Storage(format!("failed to delete executor: {e}")))?;

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        Ok(())
    }

    async fn find_executors(&self, node: Option<&str>) -> Result<Vec<Executor>, FlameError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        let daos: Vec<ExecutorDao> = match node {
            Some(node_name) => {
                let sql = "SELECT * FROM executors WHERE node=?";
                sqlx::query_as(sql)
                    .bind(node_name)
                    .fetch_all(&mut *tx)
                    .await
                    .map_err(|e| FlameError::Storage(e.to_string()))?
            }
            None => {
                let sql = "SELECT * FROM executors";
                sqlx::query_as(sql)
                    .fetch_all(&mut *tx)
                    .await
                    .map_err(|e| FlameError::Storage(e.to_string()))?
            }
        };

        tx.commit()
            .await
            .map_err(|e| FlameError::Storage(e.to_string()))?;

        Ok(daos
            .iter()
            .map(Executor::try_from)
            .filter_map(Result::ok)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::apis::ApplicationState;

    use super::*;

    fn app_id(name: &str) -> String {
        format!("default/{name}")
    }

    fn test_applications() -> Vec<(String, ApplicationAttributes)> {
        ["flmexec", "flmping", "flmrun"]
            .into_iter()
            .map(|name| {
                (
                    name.to_string(),
                    ApplicationAttributes {
                        id: app_id(name),
                        ..Default::default()
                    },
                )
            })
            .collect()
    }

    #[test]
    fn test_workspace_schema_uses_sqlx_history_on_restart() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_test_uuid_schema_restart");
        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;
        drop(storage);

        let root = SqliteEngine::storage_root(&url)?;
        let workspace_url = format!("sqlite://{}", root.join("default/flame.db").display());
        let pool = tokio_test::block_on(SqlitePool::connect(&workspace_url))
            .map_err(|error| FlameError::Storage(error.to_string()))?;
        let marker_count: i64 = tokio_test::block_on(
            sqlx::query_scalar(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name LIKE 'rfe392%'",
            )
            .fetch_one(&pool),
        )
        .map_err(|error| FlameError::Storage(error.to_string()))?;
        assert_eq!(marker_count, 0);
        for table in ["applications", "sessions", "tasks"] {
            let columns: Vec<(String, i64)> = tokio_test::block_on(
                sqlx::query_as(&format!(
                    "SELECT name, pk FROM pragma_table_info('{table}')"
                ))
                .fetch_all(&pool),
            )
            .map_err(|error| FlameError::Storage(error.to_string()))?;
            assert!(
                columns.iter().any(|(name, pk)| name == "id" && *pk == 1),
                "{table}"
            );
            assert!(!columns.iter().any(|(name, _)| name == "uuid"), "{table}");
        }
        let name_index_count: i64 = tokio_test::block_on(sqlx::query_scalar(
            "SELECT count(*) FROM pragma_index_list('applications') WHERE name='idx_applications_name'",
        ).fetch_one(&pool)).map_err(|error| FlameError::Storage(error.to_string()))?;
        assert_eq!(name_index_count, 1);
        tokio_test::block_on(pool.close());

        tokio_test::block_on(SqliteEngine::new_ptr(&url))?;
        Ok(())
    }

    #[test]
    fn test_applications_with_same_name_in_distinct_workspaces() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_test_duplicate_application_names");
        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;
        tokio_test::block_on(storage.create_workspace("other".to_string()))?;
        let first_id = "default/shared-name".to_string();
        let second_id = "other/shared-name".to_string();

        for id in [&first_id, &second_id] {
            tokio_test::block_on(storage.register_application(
                "shared-name".to_string(),
                ApplicationAttributes {
                    id: id.clone(),
                    ..Default::default()
                },
            ))?;
        }

        assert_eq!(
            tokio_test::block_on(storage.get_application(first_id))?.name,
            "shared-name"
        );
        assert_eq!(
            tokio_test::block_on(storage.get_application(second_id))?.name,
            "shared-name"
        );
        assert_eq!(
            tokio_test::block_on(storage.find_applications(None))?.len(),
            2
        );
        Ok(())
    }

    #[test]
    fn workspace_creation_opens_isolated_databases_and_reloads() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_test_workspace_databases");
        let root = SqliteEngine::storage_root(&url)?;
        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;
        tokio_test::block_on(storage.create_workspace("team".to_string()))?;
        assert!(root.join("flame.db").is_file());
        assert!(root.join("default/flame.db").is_file());
        assert!(root.join("team/flame.db").is_file());
        assert!(root.join("team/metadata").is_file());
        let team_url = format!("sqlite://{}", root.join("team/flame.db").display());
        let team_pool = tokio_test::block_on(SqlitePool::connect(&team_url))
            .map_err(|error| FlameError::Storage(error.to_string()))?;
        let table_count: i64 = tokio_test::block_on(
            sqlx::query_scalar(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='applications'",
            )
            .fetch_one(&team_pool),
        )
        .map_err(|error| FlameError::Storage(error.to_string()))?;
        assert_eq!(table_count, 1);
        tokio_test::block_on(team_pool.close());
        tokio_test::block_on(storage.register_application(
            "app".to_string(),
            ApplicationAttributes {
                id: "team/app".to_string(),
                ..Default::default()
            },
        ))?;
        drop(storage);

        let reopened = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;
        let workspaces = tokio_test::block_on(reopened.list_workspaces())?;
        assert_eq!(
            workspaces
                .iter()
                .map(|ws| ws.name.as_str())
                .collect::<Vec<_>>(),
            vec!["default", "team"]
        );
        assert_eq!(
            tokio_test::block_on(reopened.get_application("team/app".to_string()))?.name,
            "app"
        );
        assert!(matches!(
            tokio_test::block_on(reopened.get_application("default/app".to_string())),
            Err(FlameError::NotFound(_))
        ));
        Ok(())
    }

    #[test]
    fn interrupted_workspace_creation_does_not_block_restart_or_retry() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_test_interrupted_workspace_creation");
        let root = SqliteEngine::storage_root(&url)?;
        std::fs::create_dir_all(&root)?;
        let staging = root.join(".creating-interrupted");
        std::fs::create_dir(&staging)?;
        let pool = tokio_test::block_on(SqliteEngine::connect_pool(&staging.join("flame.db")))?;
        std::fs::write(staging.join("metadata"), "1")?;
        tokio_test::block_on(pool.close());

        let engine = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;
        assert_eq!(
            tokio_test::block_on(engine.list_workspaces())?
                .iter()
                .map(|workspace| workspace.name.as_str())
                .collect::<Vec<_>>(),
            vec!["default"]
        );
        tokio_test::block_on(engine.create_workspace("team".to_string()))?;
        assert!(root.join("team/flame.db").is_file());
        assert!(root.join("team/metadata").is_file());
        Ok(())
    }

    #[test]
    fn test_create_session_normalizes_batch_size() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_test_create_session_normalizes_batch_size");
        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;

        for (name, attr) in test_applications() {
            tokio_test::block_on(storage.register_application(name.clone(), attr))?;
        }

        let session = format!("default/ssn-batch-{}", Utc::now().timestamp());
        let ssn = tokio_test::block_on(storage.create_session(SessionAttributes {
            name: (session.clone()).rsplit('/').next().unwrap().to_string(),
            workspace: "default".to_string(),
            application: "flmexec".to_string(),
            common_data: None,
            min_instances: 2,
            max_instances: Some(4),
            batch_size: 2,
            priority: 0,
            resreq: None,
        }))?;

        assert_eq!(ssn.batch_size, 1);

        let ssn = tokio_test::block_on(storage.get_session(session))?;
        assert_eq!(ssn.batch_size, 1);

        Ok(())
    }

    #[test]
    fn test_get_task_with_events() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_test_get_task_with_events");
        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;

        for (name, attr) in test_applications() {
            tokio_test::block_on(storage.register_application(name.clone(), attr))?;
        }

        let ssn_1_id = format!("default/ssn-1-{}", Utc::now().timestamp());
        let ssn_1 = tokio_test::block_on(storage.create_session(SessionAttributes {
            name: (ssn_1_id.clone()).rsplit('/').next().unwrap().to_string(),
            workspace: "default".to_string(),
            application: "flmexec".to_string(),
            common_data: None,
            min_instances: 0,
            max_instances: None,
            batch_size: 1,
            priority: 0,
            resreq: None,
        }))?;
        assert_eq!(ssn_1.gid, ssn_1_id);
        assert_eq!(ssn_1.application, app_id("flmexec"));
        assert_eq!(ssn_1.status.state, SessionState::Open);

        let task_1_1 = tokio_test::block_on(storage.create_task(ssn_1.gid.clone(), None, None))?;
        assert_eq!(task_1_1.number, 1);
        let tasks = tokio_test::block_on(storage.find_tasks(ssn_1.gid.clone()))?;
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].number, 1);
        assert_eq!(tasks[0].session, ssn_1.gid.clone());
        assert_eq!(tasks[0].state, TaskState::Pending);
        assert_eq!(tasks[0].input, None);
        assert_eq!(tasks[0].output, None);

        let task_1_1 = tokio_test::block_on(storage.update_task_state(
            task_1_1.gid().unwrap(),
            TaskState::Succeed,
            Some("Task succeeded".to_string()),
        ))?;
        assert_eq!(task_1_1.state, TaskState::Succeed);
        let tasks = tokio_test::block_on(storage.find_tasks(ssn_1.gid.clone()))?;

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].number, 1);
        assert_eq!(tasks[0].session, ssn_1.gid.clone());
        assert_eq!(tasks[0].state, TaskState::Succeed);

        Ok(())
    }

    #[test]
    fn test_update_application() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_test_update_application");
        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;

        for (name, attr) in test_applications() {
            tokio_test::block_on(storage.register_application(name.clone(), attr))?;
        }

        let app_1 = tokio_test::block_on(storage.get_application(app_id("flmexec")))?;
        assert_eq!(app_1.name, "flmexec");
        assert_eq!(app_1.state, ApplicationState::Enabled);

        let app_2 = tokio_test::block_on(storage.update_application(
            app_id("flmexec"),
            ApplicationAttributes {
                id: String::new(),
                shim: Shim::Cri,
                description: Some("This is my agent for testing.".to_string()),
                labels: vec!["test".to_string(), "agent".to_string()],
                image: Some("may-agent".to_string()),
                command: Some("run-agent".to_string()),
                arguments: vec!["--test".to_string(), "--agent".to_string()],
                environments: HashMap::from([("TEST".to_string(), "true".to_string())]),
                working_directory: Some("/tmp".to_string()),
                max_instances: 10,
                delay_release: Duration::seconds(0),
                schema: None,
                url: None,
                installer: None,
            },
        ))?;
        assert_eq!(app_2.name, "flmexec");
        assert_eq!(app_2.shim, Shim::Cri);
        assert_eq!(app_2.image.as_deref(), Some("may-agent"));
        assert_eq!(
            app_2.description,
            Some("This is my agent for testing.".to_string())
        );
        assert_eq!(app_2.labels, vec!["test".to_string(), "agent".to_string()]);
        assert_eq!(app_2.command, Some("run-agent".to_string()));
        assert_eq!(
            app_2.arguments,
            vec!["--test".to_string(), "--agent".to_string()]
        );
        assert_eq!(
            app_2.environments,
            HashMap::from([("TEST".to_string(), "true".to_string())])
        );
        assert_eq!(app_2.working_directory, Some("/tmp".to_string()));
        assert_eq!(app_2.max_instances, 10);
        assert_eq!(app_2.delay_release, Duration::seconds(0));
        assert!(app_2.schema.is_none());

        Ok(())
    }

    #[test]
    fn test_application_state_update_and_filter() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_test_application_state_update_and_filter");
        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;
        let enabled = tokio_test::block_on(
            storage
                .register_application("enabled-app".to_string(), ApplicationAttributes::default()),
        )?;
        let disabled_app = tokio_test::block_on(
            storage
                .register_application("disabled-app".to_string(), ApplicationAttributes::default()),
        )?;

        let disabled = tokio_test::block_on(
            storage.update_application_state(disabled_app.gid.clone(), ApplicationState::Disabled),
        )?;
        assert_eq!(disabled.version, 2);
        let unchanged = tokio_test::block_on(
            storage.update_application_state(disabled_app.gid.clone(), ApplicationState::Disabled),
        )?;
        assert_eq!(unchanged.version, disabled.version);

        let result = tokio_test::block_on(storage.update_application(
            disabled_app.gid.clone(),
            ApplicationAttributes {
                id: String::new(),
                image: Some("must-not-be-written".to_string()),
                ..Default::default()
            },
        ));
        assert!(matches!(result, Err(FlameError::InvalidState(_))));
        let unchanged = tokio_test::block_on(storage.get_application(disabled_app.gid))?;
        assert_eq!(unchanged.version, disabled.version);
        assert_eq!(unchanged.image, disabled.image);

        let filter = ApplicationFilter::by_state(ApplicationState::Disabled);
        let apps = tokio_test::block_on(storage.find_applications(Some(&filter)))?;
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "disabled-app");

        let result = tokio_test::block_on(storage.delete_application(enabled.gid));
        assert!(matches!(result, Err(FlameError::InvalidState(_))));
        Ok(())
    }

    #[test]
    fn test_unregister_application() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_test_unregister_application");
        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;

        for (name, attr) in test_applications() {
            tokio_test::block_on(storage.register_application(name.clone(), attr))?;
        }

        let ssn_1_id = format!("default/ssn-1-{}", Utc::now().timestamp());

        let ssn_1 = tokio_test::block_on(storage.create_session(SessionAttributes {
            name: (ssn_1_id.clone()).rsplit('/').next().unwrap().to_string(),
            workspace: "default".to_string(),
            application: "flmexec".to_string(),
            common_data: None,
            min_instances: 0,
            max_instances: None,
            batch_size: 1,
            priority: 0,
            resreq: None,
        }))?;
        assert_eq!(ssn_1.gid, ssn_1_id);
        assert_eq!(ssn_1.application, app_id("flmexec"));
        assert_eq!(ssn_1.status.state, SessionState::Open);

        let task_1_1 = tokio_test::block_on(storage.create_task(ssn_1.gid, None, None))?;
        assert_eq!(task_1_1.number, 1);
        let res = tokio_test::block_on(storage.delete_application(app_id("flmexec")));
        assert!(res.is_err());

        let task_1_1 = tokio_test::block_on(storage.get_task(task_1_1.gid().unwrap()))?;
        assert_eq!(task_1_1.state, TaskState::Pending);

        let task_1_1 = tokio_test::block_on(storage.update_task_state(
            task_1_1.gid().unwrap(),
            TaskState::Succeed,
            None,
        ))?;
        assert_eq!(task_1_1.state, TaskState::Succeed);

        let res = tokio_test::block_on(storage.delete_application(app_id("flmexec")));
        assert!(res.is_err());

        let ssn_1 = tokio_test::block_on(storage.close_session(ssn_1_id.clone()))?;
        assert_eq!(ssn_1.status.state, SessionState::Closed);

        tokio_test::block_on(
            storage.update_application_state(app_id("flmexec"), ApplicationState::Disabled),
        )?;
        let res = tokio_test::block_on(storage.delete_application(app_id("flmexec")));
        assert!(matches!(res, Err(FlameError::InvalidState(_))));

        let list_ssn = tokio_test::block_on(storage.find_sessions())?;
        assert_eq!(list_ssn.len(), 1);
        assert_eq!(list_ssn[0].gid, ssn_1_id);

        tokio_test::block_on(storage.delete_session(ssn_1_id))?;
        tokio_test::block_on(storage.delete_application(app_id("flmexec")))?;

        let app_1 = tokio_test::block_on(storage.get_application(app_id("flmexec")));
        assert!(app_1.is_err());

        let list_ssn = tokio_test::block_on(storage.find_sessions())?;
        assert_eq!(list_ssn.len(), 0);

        Ok(())
    }

    #[test]
    fn test_register_application() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_test_register_appl");
        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;

        let string_schema = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "string",
            "description": "The string for testing."
        });

        let apps = vec![
            (
                "my-test-agent-1".to_string(),
                ApplicationAttributes {
                    id: String::new(),
                    shim: Shim::Host,
                    image: Some("may-agent".to_string()),
                    description: Some("This is my agent for testing.".to_string()),
                    labels: vec!["test".to_string(), "agent".to_string()],
                    command: Some("my-agent".to_string()),
                    arguments: vec!["--test".to_string(), "--agent".to_string()],
                    environments: HashMap::from([("TEST".to_string(), "true".to_string())]),
                    working_directory: Some("/tmp".to_string()),
                    max_instances: 10,
                    delay_release: Duration::seconds(0),
                    schema: Some(ApplicationSchema {
                        input: Some(string_schema.to_string()),
                        output: Some(string_schema.to_string()),
                        common_data: None,
                    }),
                    url: None,
                    installer: None,
                },
            ),
            (
                "empty-app".to_string(),
                ApplicationAttributes {
                    id: String::new(),
                    shim: Shim::Host,
                    image: None,
                    description: None,
                    labels: vec![],
                    command: None,
                    arguments: vec![],
                    environments: HashMap::new(),
                    working_directory: Some("/tmp".to_string()),
                    max_instances: 10,
                    delay_release: Duration::seconds(0),
                    schema: None,
                    url: None,
                    installer: None,
                },
            ),
        ];
        for (name, attr) in apps {
            let expected_shim = attr.shim;
            let expected_image = attr.image.clone();
            let app_id = format!("default/{name}");
            tokio_test::block_on(storage.register_application(name.clone(), attr)).map_err(
                |e| FlameError::Storage(format!("failed to register application <{name}>: {e}")),
            )?;
            let app_1 = tokio_test::block_on(storage.get_application(app_id)).map_err(|e| {
                FlameError::Storage(format!("failed to get application <{name}>: {e}"))
            })?;

            assert_eq!(app_1.name, name);
            assert_eq!(app_1.state, ApplicationState::Enabled);
            assert_eq!(app_1.shim, expected_shim);
            assert_eq!(app_1.image, expected_image);
        }

        Ok(())
    }

    #[test]
    fn test_register_duplicate_uid_returns_already_exists() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_test_register_duplicate_app");
        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;

        let attr = ApplicationAttributes {
            id: "default/duplicate".to_string(),
            ..Default::default()
        };
        tokio_test::block_on(storage.register_application("duplicate".to_string(), attr.clone()))?;
        let error =
            tokio_test::block_on(storage.register_application("duplicate".to_string(), attr))
                .unwrap_err();

        assert!(matches!(error, FlameError::AlreadyExist(_)));
        Ok(())
    }

    #[test]
    fn test_get_application() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_test_app");
        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;

        for (name, attr) in test_applications() {
            tokio_test::block_on(storage.register_application(name.clone(), attr))?;
        }

        let app_1 = tokio_test::block_on(storage.get_application(app_id("flmexec")))?;

        assert_eq!(app_1.name, "flmexec");
        assert_eq!(app_1.state, ApplicationState::Enabled);

        Ok(())
    }

    #[test]
    fn test_register_application_with_url() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_test_register_app_with_url");
        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;

        let test_url = "file:///opt/test-package.whl".to_string();

        // Register application with URL
        let app = tokio_test::block_on(storage.register_application(
            "flmtestapp-url".to_string(),
            ApplicationAttributes {
                id: String::new(),
                shim: Shim::Host,
                image: None,
                description: Some("Test application with URL".to_string()),
                labels: vec!["test".to_string()],
                command: Some("/usr/bin/uv".to_string()),
                arguments: vec![
                    "run".to_string(),
                    "-n".to_string(),
                    "flamepy.app.runpy".to_string(),
                ],
                environments: HashMap::new(),
                working_directory: Some("/tmp".to_string()),
                max_instances: 5,
                delay_release: Duration::seconds(10),
                schema: None,
                url: Some(test_url.clone()),
                installer: None,
            },
        ))?;

        // Verify application was registered with URL
        assert_eq!(app.name, "flmtestapp-url");
        assert_eq!(app.url.as_ref(), Some(&test_url));
        assert_eq!(
            app.description,
            Some("Test application with URL".to_string())
        );
        assert_eq!(app.state, ApplicationState::Enabled);

        // Retrieve and verify URL persisted
        let retrieved_app = tokio_test::block_on(storage.get_application(app.gid.clone()))?;
        assert_eq!(retrieved_app.name, "flmtestapp-url");
        assert_eq!(retrieved_app.url.as_ref(), Some(&test_url));
        assert_eq!(
            retrieved_app.description,
            Some("Test application with URL".to_string())
        );
        assert_eq!(retrieved_app.state, ApplicationState::Enabled);

        let updated = tokio_test::block_on(storage.update_application(
            app.gid,
            ApplicationAttributes {
                url: Some(test_url.clone()),
                ..ApplicationAttributes::default()
            },
        ))?;
        assert_eq!(updated.url, Some(test_url));

        Ok(())
    }

    #[test]
    fn test_register_application_without_package() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_test_register_app_without_url");
        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;

        // Applications without a package remain valid.
        let app = tokio_test::block_on(storage.register_application(
            "flmtestapp-no-url".to_string(),
            ApplicationAttributes {
                id: String::new(),
                shim: Shim::Host,
                image: None,
                description: Some("Test application without URL".to_string()),
                labels: vec!["test".to_string()],
                command: Some("/usr/bin/test".to_string()),
                arguments: vec![],
                environments: HashMap::new(),
                working_directory: Some("/tmp".to_string()),
                max_instances: 5,
                delay_release: Duration::seconds(10),
                schema: None,
                url: None,
                installer: None,
            },
        ))?;

        // Verify application was registered without a package.
        assert_eq!(app.name, "flmtestapp-no-url");
        assert!(app.url.is_none());
        assert_eq!(
            app.description,
            Some("Test application without URL".to_string())
        );
        assert_eq!(app.state, ApplicationState::Enabled);

        // Retrieve and verify the package is absent.
        let retrieved_app = tokio_test::block_on(storage.get_application(app.gid.clone()))?;
        assert_eq!(retrieved_app.name, "flmtestapp-no-url");
        assert!(retrieved_app.url.is_none());
        assert_eq!(retrieved_app.state, ApplicationState::Enabled);

        Ok(())
    }

    #[test]
    fn test_update_application_with_url() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_test_update_application_with_url");
        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;

        // Register initial application without URL
        let registered = tokio_test::block_on(storage.register_application(
            "flmtestapp-update".to_string(),
            ApplicationAttributes {
                id: String::new(),
                shim: Shim::Host,
                image: None,
                description: Some("Initial description".to_string()),
                labels: vec![],
                command: Some("/usr/bin/test".to_string()),
                arguments: vec![],
                environments: HashMap::new(),
                working_directory: Some("/tmp".to_string()),
                max_instances: 5,
                delay_release: Duration::seconds(10),
                schema: None,
                url: None,
                installer: None,
            },
        ))?;

        let app_before = tokio_test::block_on(storage.get_application(registered.gid.clone()))?;
        assert!(app_before.url.is_none());

        // Update application with URL
        let test_url = "file:///opt/updated-package.whl".to_string();
        let updated_app = tokio_test::block_on(storage.update_application(
            registered.gid.clone(),
            ApplicationAttributes {
                id: String::new(),
                shim: Shim::Host,
                image: Some("updated-image".to_string()),
                description: Some("Updated description".to_string()),
                labels: vec!["updated".to_string()],
                command: Some("/usr/bin/uv".to_string()),
                arguments: vec!["run".to_string()],
                environments: HashMap::from([("ENV".to_string(), "test".to_string())]),
                working_directory: Some("/opt".to_string()),
                max_instances: 10,
                delay_release: Duration::seconds(20),
                schema: None,
                url: Some(test_url.clone()),
                installer: None,
            },
        ))?;

        // Verify update including URL
        assert_eq!(updated_app.name, "flmtestapp-update");
        assert_eq!(updated_app.url.as_ref(), Some(&test_url));
        assert_eq!(
            updated_app.description,
            Some("Updated description".to_string())
        );
        // Note: image field is not updated by update_application method
        assert_eq!(updated_app.working_directory, Some("/opt".to_string()));
        assert_eq!(updated_app.max_instances, 10);

        // Retrieve and verify URL persisted after update
        let retrieved_app = tokio_test::block_on(storage.get_application(registered.gid))?;
        assert_eq!(retrieved_app.url.as_ref(), Some(&test_url));
        assert_eq!(
            retrieved_app.description,
            Some("Updated description".to_string())
        );

        Ok(())
    }

    #[test]
    fn test_single_session() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_test_single_session");
        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;
        for (name, attr) in test_applications() {
            tokio_test::block_on(storage.register_application(name.clone(), attr))?;
        }

        let ssn_1_id = format!("default/ssn-1-{}", Utc::now().timestamp());
        let ssn_1 = tokio_test::block_on(storage.create_session(SessionAttributes {
            name: (ssn_1_id.clone()).rsplit('/').next().unwrap().to_string(),
            workspace: "default".to_string(),
            application: "flmexec".to_string(),
            common_data: None,
            min_instances: 0,
            max_instances: None,
            batch_size: 1,
            priority: 0,
            resreq: None,
        }))?;

        assert_eq!(ssn_1.gid, ssn_1_id);
        assert_eq!(ssn_1.application, app_id("flmexec"));
        assert_eq!(ssn_1.status.state, SessionState::Open);

        let task_1_1 = tokio_test::block_on(storage.create_task(ssn_1.gid.clone(), None, None))?;
        assert_eq!(task_1_1.number, 1);

        let task_1_2 = tokio_test::block_on(storage.create_task(ssn_1.gid.clone(), None, None))?;
        assert_eq!(task_1_2.number, 2);

        let task_list = tokio_test::block_on(storage.find_tasks(ssn_1.gid))?;
        assert_eq!(task_list.len(), 2);

        let task_1_1 = tokio_test::block_on(storage.update_task_state(
            task_1_1.gid().unwrap(),
            TaskState::Succeed,
            None,
        ))?;
        assert_eq!(task_1_1.state, TaskState::Succeed);

        let task_1_2 = tokio_test::block_on(storage.update_task_state(
            task_1_2.gid().unwrap(),
            TaskState::Succeed,
            None,
        ))?;
        assert_eq!(task_1_2.state, TaskState::Succeed);

        let ssn_1 = tokio_test::block_on(storage.close_session(ssn_1_id.clone()))?;
        assert_eq!(ssn_1.status.state, SessionState::Closed);

        Ok(())
    }

    #[test]
    fn test_multiple_session() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_test_multiple_session");
        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;
        for (name, attr) in test_applications() {
            tokio_test::block_on(storage.register_application(name.clone(), attr))?;
        }

        let ssn_1_id = format!("default/ssn-1-{}", Utc::now().timestamp());
        let ssn_1 = tokio_test::block_on(storage.create_session(SessionAttributes {
            name: (ssn_1_id.clone()).rsplit('/').next().unwrap().to_string(),
            workspace: "default".to_string(),
            application: "flmexec".to_string(),
            common_data: None,
            min_instances: 0,
            max_instances: None,
            batch_size: 1,
            priority: 0,
            resreq: None,
        }))?;

        assert_eq!(ssn_1.gid, ssn_1_id);
        assert_eq!(ssn_1.application, app_id("flmexec"));
        assert_eq!(ssn_1.status.state, SessionState::Open);

        let task_1_1 = tokio_test::block_on(storage.create_task(ssn_1.gid.clone(), None, None))?;
        assert_eq!(task_1_1.number, 1);

        let task_1_2 = tokio_test::block_on(storage.create_task(ssn_1.gid.clone(), None, None))?;
        assert_eq!(task_1_2.number, 2);

        let task_1_1 = tokio_test::block_on(storage.update_task_state(
            task_1_1.gid().unwrap(),
            TaskState::Succeed,
            None,
        ))?;
        assert_eq!(task_1_1.state, TaskState::Succeed);

        let task_1_2 = tokio_test::block_on(storage.update_task_state(
            task_1_2.gid().unwrap(),
            TaskState::Succeed,
            None,
        ))?;
        assert_eq!(task_1_2.state, TaskState::Succeed);

        let ssn_2_id = format!("default/ssn-2-{}", Utc::now().timestamp());
        let ssn_2 = tokio_test::block_on(storage.create_session(SessionAttributes {
            name: (ssn_2_id.clone()).rsplit('/').next().unwrap().to_string(),
            workspace: "default".to_string(),
            application: "flmping".to_string(),
            common_data: None,
            min_instances: 0,
            max_instances: None,
            batch_size: 1,
            priority: 0,
            resreq: None,
        }))?;

        assert_eq!(ssn_2.gid, ssn_2_id);
        assert_eq!(ssn_2.application, app_id("flmping"));
        assert_eq!(ssn_2.status.state, SessionState::Open);

        let task_2_1 = tokio_test::block_on(storage.create_task(ssn_2.gid.clone(), None, None))?;
        assert_eq!(task_2_1.number, 1);

        let task_2_2 = tokio_test::block_on(storage.create_task(ssn_2.gid.clone(), None, None))?;
        assert_eq!(task_2_2.number, 2);

        let task_2_1 = tokio_test::block_on(storage.update_task_state(
            task_2_1.gid().unwrap(),
            TaskState::Succeed,
            None,
        ))?;
        assert_eq!(task_2_1.state, TaskState::Succeed);

        let task_2_2 = tokio_test::block_on(storage.update_task_state(
            task_2_2.gid().unwrap(),
            TaskState::Succeed,
            None,
        ))?;
        assert_eq!(task_2_2.state, TaskState::Succeed);

        let ssn_list = tokio_test::block_on(storage.find_sessions())?;
        assert_eq!(ssn_list.len(), 2);

        let ssn_1 = tokio_test::block_on(storage.close_session(ssn_1_id.clone()))?;
        assert_eq!(ssn_1.status.state, SessionState::Closed);
        let ssn_2 = tokio_test::block_on(storage.close_session(ssn_2_id.clone()))?;
        assert_eq!(ssn_2.status.state, SessionState::Closed);

        Ok(())
    }

    #[test]
    fn test_close_session_with_open_tasks() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_test_close_session_with_open_tasks");
        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;
        for (name, attr) in test_applications() {
            tokio_test::block_on(storage.register_application(name.clone(), attr))?;
        }
        let ssn_1_id = format!("default/ssn-1-{}", Utc::now().timestamp());
        let ssn_1 = tokio_test::block_on(storage.create_session(SessionAttributes {
            name: (ssn_1_id.clone()).rsplit('/').next().unwrap().to_string(),
            workspace: "default".to_string(),
            application: "flmexec".to_string(),
            common_data: None,
            min_instances: 0,
            max_instances: None,
            batch_size: 1,
            priority: 0,
            resreq: None,
        }))?;

        assert_eq!(ssn_1.gid, ssn_1_id);
        assert_eq!(ssn_1.application, app_id("flmexec"));
        assert_eq!(ssn_1.status.state, SessionState::Open);

        let task_1_1 = tokio_test::block_on(storage.create_task(ssn_1.gid.clone(), None, None))?;
        assert_eq!(task_1_1.number, 1);

        let task_1_2 = tokio_test::block_on(storage.create_task(ssn_1.gid, None, None))?;
        assert_eq!(task_1_2.number, 2);

        let ssn_1 = tokio_test::block_on(storage.close_session(ssn_1_id.clone()))?;
        assert_eq!(ssn_1.status.state, SessionState::Closed);

        let task_1_1 = tokio_test::block_on(storage.get_task(task_1_1.gid().unwrap()))?;
        assert_eq!(task_1_1.state, TaskState::Cancelled);

        let task_1_2 = tokio_test::block_on(storage.get_task(task_1_2.gid().unwrap()))?;
        assert_eq!(task_1_2.state, TaskState::Cancelled);

        Ok(())
    }

    #[test]
    fn test_close_session_with_running_tasks() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_test_close_session_with_running_tasks");
        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;
        for (name, attr) in test_applications() {
            tokio_test::block_on(storage.register_application(name.clone(), attr))?;
        }
        let ssn_1_id = format!("default/ssn-1-{}", Utc::now().timestamp());
        let ssn_1 = tokio_test::block_on(storage.create_session(SessionAttributes {
            name: (ssn_1_id.clone()).rsplit('/').next().unwrap().to_string(),
            workspace: "default".to_string(),
            application: "flmexec".to_string(),
            common_data: None,
            min_instances: 0,
            max_instances: None,
            batch_size: 1,
            priority: 0,
            resreq: None,
        }))?;

        assert_eq!(ssn_1.status.state, SessionState::Open);

        let task_1_1 = tokio_test::block_on(storage.create_task(ssn_1.gid.clone(), None, None))?;
        assert_eq!(task_1_1.state, TaskState::Pending);

        tokio_test::block_on(storage.update_task_state(
            task_1_1.gid().unwrap(),
            TaskState::Running,
            None,
        ))?;

        let res = tokio_test::block_on(storage.close_session(ssn_1_id.clone()));
        assert!(res.is_err());

        Ok(())
    }

    #[test]
    fn test_create_task_for_close_session() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_test_create_task_for_close_session");

        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;
        for (name, attr) in test_applications() {
            tokio_test::block_on(storage.register_application(name.clone(), attr))?;
        }
        let ssn_1_id = format!("default/ssn-1-{}", Utc::now().timestamp());
        let ssn_1 = tokio_test::block_on(storage.create_session(SessionAttributes {
            name: (ssn_1_id.clone()).rsplit('/').next().unwrap().to_string(),
            workspace: "default".to_string(),
            application: "flmexec".to_string(),
            common_data: None,
            min_instances: 0,
            max_instances: None,
            batch_size: 1,
            priority: 0,
            resreq: None,
        }))?;

        assert_eq!(ssn_1.gid, ssn_1_id);
        assert_eq!(ssn_1.application, app_id("flmexec"));
        assert_eq!(ssn_1.status.state, SessionState::Open);

        let task_1_1 = tokio_test::block_on(storage.create_task(ssn_1.gid, None, None))?;
        assert_eq!(task_1_1.number, 1);

        let task_1_1 = tokio_test::block_on(storage.update_task_state(
            task_1_1.gid().unwrap(),
            TaskState::Succeed,
            None,
        ))?;
        assert_eq!(task_1_1.state, TaskState::Succeed);

        let ssn_1 = tokio_test::block_on(storage.close_session(ssn_1_id.clone()))?;
        assert_eq!(ssn_1.status.state, SessionState::Closed);

        let res = tokio_test::block_on(storage.create_task(ssn_1.gid, None, None));
        assert!(res.is_err());

        Ok(())
    }

    #[test]
    fn test_delete_session_with_open_tasks() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_test_delete_session_with_open_tasks");
        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;
        for (name, attr) in test_applications() {
            tokio_test::block_on(storage.register_application(name.clone(), attr))?;
        }
        let ssn_1_id = format!("default/ssn-1-{}", Utc::now().timestamp());
        let ssn_1 = tokio_test::block_on(storage.create_session(SessionAttributes {
            name: (ssn_1_id.clone()).rsplit('/').next().unwrap().to_string(),
            workspace: "default".to_string(),
            application: "flmexec".to_string(),
            common_data: None,
            min_instances: 0,
            max_instances: None,
            batch_size: 1,
            priority: 0,
            resreq: None,
        }))?;

        assert_eq!(ssn_1.gid, ssn_1_id);
        assert_eq!(ssn_1.application, app_id("flmexec"));
        assert_eq!(ssn_1.status.state, SessionState::Open);

        let task_1_1 = tokio_test::block_on(storage.create_task(ssn_1.gid.clone(), None, None))?;
        assert_eq!(task_1_1.number, 1);

        // It should be failed because the session is open and there are open tasks
        let res = tokio_test::block_on(storage.delete_session(ssn_1_id.clone()));
        assert!(res.is_err());

        let task_1_1 = tokio_test::block_on(storage.get_task(task_1_1.gid().unwrap()))?;
        assert_eq!(task_1_1.state, TaskState::Pending);

        let task_1_1 = tokio_test::block_on(storage.update_task_state(
            task_1_1.gid().unwrap(),
            TaskState::Succeed,
            None,
        ))?;
        assert_eq!(task_1_1.state, TaskState::Succeed);

        // It should be failed because the session is open
        let res = tokio_test::block_on(storage.delete_session(ssn_1_id.clone()));
        assert!(res.is_err());

        let ssn_1 = tokio_test::block_on(storage.close_session(ssn_1_id.clone()))?;
        assert_eq!(ssn_1.status.state, SessionState::Closed);

        let ssn_1 = tokio_test::block_on(storage.delete_session(ssn_1_id.clone()))?;
        assert_eq!(ssn_1.status.state, SessionState::Closed);

        Ok(())
    }

    #[test]
    fn test_delete_session_with_cancelled_tasks() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_test_delete_session_with_cancelled_tasks");
        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;
        tokio_test::block_on(
            storage.register_application("flmexec".to_string(), ApplicationAttributes::default()),
        )?;
        let session = format!(
            "default/cancelled-tasks-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        );
        tokio_test::block_on(storage.create_session(SessionAttributes {
            name: (session.clone()).rsplit('/').next().unwrap().to_string(),
            workspace: "default".to_string(),
            application: "flmexec".to_string(),
            ..Default::default()
        }))?;
        let task = tokio_test::block_on(storage.create_task(session.clone(), None, None))?;

        tokio_test::block_on(storage.close_session(session.clone()))?;
        assert_eq!(
            tokio_test::block_on(storage.get_task(task.gid().unwrap()))?.state,
            TaskState::Cancelled
        );
        let deleted = tokio_test::block_on(storage.delete_session(session))?;
        assert_eq!(deleted.status.state, SessionState::Closed);

        Ok(())
    }

    #[test]
    fn resource_uuids_survive_workspace_database_restart() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_uuid_persistence");
        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;
        let app = tokio_test::block_on(
            storage.register_application("app".to_string(), ApplicationAttributes::default()),
        )?;
        let session = tokio_test::block_on(storage.create_session(SessionAttributes {
            workspace: "default".to_string(),
            name: "run".to_string(),
            application: app.name.clone(),
            ..Default::default()
        }))?;
        let task = tokio_test::block_on(storage.create_task(session.gid.clone(), None, None))?;
        assert_eq!(app.gid, "default/app");
        assert_eq!(session.gid, "default/run");
        assert_eq!(session.application, "default/app");
        assert_eq!(task.session, "default/run");
        let root = SqliteEngine::storage_root(&url)?;
        let workspace_url = format!("sqlite://{}", root.join("default/flame.db").display());
        let pool = tokio_test::block_on(SqlitePool::connect(&workspace_url))
            .map_err(|error| FlameError::Storage(error.to_string()))?;
        let (app_row, app_name): (String, String) = tokio_test::block_on(
            sqlx::query_as("SELECT id, name FROM applications").fetch_one(&pool),
        )
        .map_err(|error| FlameError::Storage(error.to_string()))?;
        let (session_row, stored_session, application_row): (String, String, String) =
            tokio_test::block_on(
                sqlx::query_as("SELECT id, name, application FROM sessions").fetch_one(&pool),
            )
            .map_err(|error| FlameError::Storage(error.to_string()))?;
        let (task_row, task_number, task_session_row): (String, i64, String) =
            tokio_test::block_on(
                sqlx::query_as("SELECT id, number, session FROM tasks").fetch_one(&pool),
            )
            .map_err(|error| FlameError::Storage(error.to_string()))?;
        assert_eq!(
            (app_row, session_row, task_row),
            (app.id.clone(), session.id.clone(), task.id.clone())
        );
        assert_eq!(
            (
                app_name,
                stored_session,
                application_row,
                task_number,
                task_session_row
            ),
            (
                "app".into(),
                "run".into(),
                "app".into(),
                task.number,
                "run".into()
            )
        );
        tokio_test::block_on(pool.close());
        for uuid in [&app.id, &session.id, &task.id] {
            uuid::Uuid::parse_str(uuid).map_err(|error| FlameError::Storage(error.to_string()))?;
        }
        let task_gid = task.gid().unwrap();
        drop(storage);

        let reopened = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;
        assert_eq!(
            tokio_test::block_on(reopened.get_application(app.gid))?.id,
            app.id
        );
        assert_eq!(
            tokio_test::block_on(reopened.get_session(session.gid))?.id,
            session.id
        );
        assert_eq!(
            tokio_test::block_on(reopened.get_task(task_gid))?.id,
            task.id
        );
        Ok(())
    }

    #[test]
    fn session_name_is_unique_across_apps_in_workspace() -> Result<(), FlameError> {
        let url = crate::temp_sqlite_url("flame_session_workspace_uniqueness");
        let storage = tokio_test::block_on(SqliteEngine::new_ptr(&url))?;
        let first = tokio_test::block_on(
            storage.register_application("first".to_string(), ApplicationAttributes::default()),
        )?;
        let second = tokio_test::block_on(
            storage.register_application("second".to_string(), ApplicationAttributes::default()),
        )?;
        let first_session = SessionAttributes {
            workspace: "default".to_string(),
            name: "run".to_string(),
            application: first.name,
            ..Default::default()
        };
        tokio_test::block_on(storage.create_session(first_session))?;
        let second_session = SessionAttributes {
            workspace: "default".to_string(),
            name: "run".to_string(),
            application: second.name,
            ..Default::default()
        };
        assert!(tokio_test::block_on(storage.create_session(second_session)).is_err());
        tokio_test::block_on(storage.create_workspace("other".to_string()))?;
        let other = tokio_test::block_on(storage.register_application(
            "first".to_string(),
            ApplicationAttributes {
                id: "other/first".to_string(),
                ..Default::default()
            },
        ))?;
        let other_session = tokio_test::block_on(storage.create_session(SessionAttributes {
            workspace: "other".to_string(),
            name: "run".to_string(),
            application: other.name,
            ..Default::default()
        }))?;
        assert_eq!(other_session.gid, "other/run");
        assert_eq!(tokio_test::block_on(storage.find_sessions())?.len(), 2);
        Ok(())
    }
}
