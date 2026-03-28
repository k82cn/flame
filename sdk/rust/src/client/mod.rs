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
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, TimeZone, Utc};
use futures::TryFutureExt;
// use serde::{Deserialize, Serialize};
use serde_derive::{Deserialize, Serialize};
use stdng::{lock_ptr, trace_fn};
use tokio_stream::StreamExt;
use tonic::transport::Channel;
use tonic::transport::Endpoint;
use tonic::Request;
use url::Url;

use self::rpc::frontend_client::FrontendClient as FlameFrontendClient;
use self::rpc::{
    ApplicationSpec, CloseSessionRequest, CreateSessionRequest, CreateTaskRequest, Environment,
    GetApplicationRequest, GetNodeRequest, GetSessionRequest, GetTaskRequest,
    ListApplicationRequest, ListExecutorRequest, ListNodesRequest, ListSessionRequest,
    ListTaskRequest, OpenSessionRequest, RegisterApplicationRequest, SessionSpec, TaskSpec,
    UnregisterApplicationRequest, UpdateApplicationRequest, WatchTaskRequest,
};
use crate::apis::flame as rpc;
use crate::apis::FlameClientTls;
use crate::apis::{
    ApplicationID, ApplicationState, CommonData, ExecutorState, FlameError, SessionID,
    SessionState, Shim, TaskID, TaskInput, TaskOutput, TaskState,
};

type FlameClient = FlameFrontendClient<Channel>;

/// Connect to a Flame service without TLS (plaintext).
///
/// Use `connect_with_tls` for TLS-enabled connections.
pub async fn connect(addr: &str) -> Result<Connection, FlameError> {
    connect_with_tls(addr, None).await
}

/// Connect to a Flame service with optional TLS configuration.
///
/// # Arguments
/// * `addr` - The endpoint URL (use https:// for TLS, http:// for plaintext)
/// * `tls_config` - Optional TLS configuration for secure connections
///
/// # TLS Behavior
/// - If `addr` starts with `https://` and `tls_config` is `Some`, use provided TLS config
/// - If `addr` starts with `https://` and `tls_config` is `None`, use default TLS config (system CA)
/// - If `addr` starts with `http://`, TLS is not used regardless of `tls_config`
pub async fn connect_with_tls(
    addr: &str,
    tls_config: Option<&FlameClientTls>,
) -> Result<Connection, FlameError> {
    let mut channel_builder = Endpoint::from_shared(addr.to_string())
        .map_err(|_| FlameError::InvalidConfig(format!("invalid address <{addr}>")))?;

    // Apply TLS if endpoint uses https://
    if addr.starts_with("https://") {
        // Extract domain name from URL for TLS verification
        let url = Url::parse(addr)
            .map_err(|e| FlameError::InvalidConfig(format!("invalid URL <{}>: {}", addr, e)))?;
        let domain = url
            .host_str()
            .ok_or_else(|| FlameError::InvalidConfig(format!("no host in URL <{}>", addr)))?;

        let client_tls_config = if let Some(tls) = tls_config {
            // Check for insecure_skip_verify - if true, we skip TLS entirely (for dev only)
            if tls.insecure_skip_verify {
                tracing::warn!(
                    "TLS certificate verification disabled for {} - NOT RECOMMENDED FOR PRODUCTION",
                    addr
                );
                // Use default TLS config without custom CA (accepts any cert)
                tonic::transport::ClientTlsConfig::new().domain_name(domain)
            } else {
                tls.client_tls_config(domain)?
            }
        } else {
            // Use default TLS config (system CA bundle)
            tonic::transport::ClientTlsConfig::new().domain_name(domain)
        };

        channel_builder = channel_builder.tls_config(client_tls_config).map_err(|e| {
            FlameError::InvalidConfig(format!("TLS config error for <{}>: {}", addr, e))
        })?;
        tracing::debug!("TLS enabled for connection to {}", addr);
    }

    let channel = channel_builder.connect().await.map_err(|e| {
        FlameError::InvalidConfig(format!("failed to connect to <{}>: {}", addr, e))
    })?;

    Ok(Connection { channel })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub code: i32,
    pub message: Option<String>,
    #[serde(with = "serde_utc")]
    pub creation_time: DateTime<Utc>,
}

#[derive(Clone)]
pub struct Connection {
    pub(crate) channel: Channel,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SessionAttributes {
    pub id: SessionID,
    pub application: String,
    pub slots: u32,
    #[serde(with = "serde_message")]
    pub common_data: Option<CommonData>,
    pub min_instances: u32,
    pub max_instances: Option<u32>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ApplicationSchema {
    pub input: Option<String>,
    pub output: Option<String>,
    pub common_data: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ApplicationAttributes {
    pub shim: Option<Shim>,
    pub image: Option<String>,
    pub description: Option<String>,
    pub labels: Vec<String>,
    pub command: Option<String>,
    pub arguments: Vec<String>,
    pub environments: HashMap<String, String>,
    pub working_directory: Option<String>,
    pub max_instances: Option<u32>,
    #[serde(with = "serde_duration")]
    pub delay_release: Option<Duration>,
    pub schema: Option<ApplicationSchema>,
    pub url: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Application {
    pub name: ApplicationID,

    pub attributes: ApplicationAttributes,

    pub state: ApplicationState,
    #[serde(with = "serde_utc")]
    pub creation_time: DateTime<Utc>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Executor {
    pub id: String,
    pub state: ExecutorState,
    pub session_id: Option<String>,
    pub slots: u32,
    pub node: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Node {
    pub name: String,
    pub hostname: String,
    pub state: NodeState,
    pub cpu: u64,
    pub memory: u64,
    pub arch: String,
    pub os: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum NodeState {
    #[default]
    Unknown = 0,
    Ready = 1,
    NotReady = 2,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    #[serde(skip)]
    pub(crate) client: Option<FlameClient>,

    pub id: SessionID,
    pub slots: u32,
    pub application: String,
    #[serde(with = "serde_utc")]
    pub creation_time: DateTime<Utc>,

    pub state: SessionState,
    pub pending: i32,
    pub running: i32,
    pub succeed: i32,
    pub failed: i32,

    pub events: Vec<Event>,
    pub tasks: Option<Vec<Task>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskID,
    pub ssn_id: SessionID,

    pub state: TaskState,

    #[serde(with = "serde_message")]
    pub input: Option<TaskInput>,
    #[serde(with = "serde_message")]
    pub output: Option<TaskOutput>,

    pub events: Vec<Event>,
}

pub type TaskInformerPtr = Arc<Mutex<dyn TaskInformer>>;

pub trait TaskInformer: Send + Sync + 'static {
    fn on_update(&mut self, task: Task);
    fn on_error(&mut self, e: FlameError);
}

impl Task {
    pub fn is_completed(&self) -> bool {
        self.state.is_terminal()
    }

    pub fn is_succeed(&self) -> bool {
        self.state == TaskState::Succeed
    }

    pub fn is_failed(&self) -> bool {
        self.state == TaskState::Failed
    }

    pub fn is_cancelled(&self) -> bool {
        self.state == TaskState::Cancelled
    }
}

impl Connection {
    pub async fn create_session(&self, attrs: &SessionAttributes) -> Result<Session, FlameError> {
        trace_fn!("Connection::create_session");

        let create_ssn_req = CreateSessionRequest {
            session_id: attrs.id.clone(),
            session: Some(SessionSpec {
                application: attrs.application.clone(),
                slots: attrs.slots,
                common_data: attrs.common_data.clone().map(CommonData::into),
                min_instances: attrs.min_instances,
                max_instances: attrs.max_instances,
            }),
        };

        let mut client = FlameClient::new(self.channel.clone());
        let ssn = client.create_session(create_ssn_req).await?;
        let ssn = ssn.into_inner();

        let mut ssn = Session::from(&ssn);
        ssn.client = Some(client);

        Ok(ssn)
    }

    pub async fn list_session(&self) -> Result<Vec<Session>, FlameError> {
        let mut client = FlameClient::new(self.channel.clone());
        let ssn_list = client.list_session(ListSessionRequest {}).await?;

        Ok(ssn_list
            .into_inner()
            .sessions
            .iter()
            .map(Session::from)
            .collect())
    }

    pub async fn get_session(&self, id: &SessionID) -> Result<Session, FlameError> {
        let mut client = FlameClient::new(self.channel.clone());
        let ssn = client
            .get_session(GetSessionRequest {
                session_id: id.to_string(),
            })
            .await?;

        let ssn = ssn.into_inner();
        let mut ssn = Session::from(&ssn);
        ssn.client = Some(client);

        Ok(ssn)
    }

    pub async fn open_session(
        &self,
        id: &SessionID,
        spec: Option<&SessionAttributes>,
    ) -> Result<Session, FlameError> {
        let session_spec = spec.map(|attrs| SessionSpec {
            application: attrs.application.clone(),
            slots: attrs.slots,
            common_data: attrs.common_data.clone().map(CommonData::into),
            min_instances: attrs.min_instances,
            max_instances: attrs.max_instances,
        });

        let open_ssn_req = OpenSessionRequest {
            session_id: id.clone(),
            session: session_spec,
        };

        let mut client = FlameClient::new(self.channel.clone());
        let ssn = client.open_session(open_ssn_req).await?;
        let ssn = ssn.into_inner();

        let mut ssn = Session::from(&ssn);
        ssn.client = Some(client);

        Ok(ssn)
    }

    pub async fn close_session(&self, id: &str) -> Result<(), FlameError> {
        let mut client = FlameClient::new(self.channel.clone());
        client
            .close_session(CloseSessionRequest {
                session_id: id.to_string(),
            })
            .await?;

        Ok(())
    }

    pub async fn register_application(
        &self,
        name: String,
        app: ApplicationAttributes,
    ) -> Result<(), FlameError> {
        let mut client = FlameClient::new(self.channel.clone());

        let req = RegisterApplicationRequest {
            name,
            application: Some(ApplicationSpec::from(app)),
        };

        let res = client
            .register_application(Request::new(req))
            .await?
            .into_inner();

        if res.return_code < 0 {
            Err(FlameError::Network(res.message.unwrap_or_default()))
        } else {
            Ok(())
        }
    }

    pub async fn update_application(
        &self,
        name: String,
        app: ApplicationAttributes,
    ) -> Result<(), FlameError> {
        let mut client = FlameClient::new(self.channel.clone());

        let req = UpdateApplicationRequest {
            name,
            application: Some(ApplicationSpec::from(app)),
        };

        let res = client
            .update_application(Request::new(req))
            .await?
            .into_inner();

        if res.return_code < 0 {
            Err(FlameError::Network(res.message.unwrap_or_default()))
        } else {
            Ok(())
        }
    }

    pub async fn unregister_application(&self, name: String) -> Result<(), FlameError> {
        let mut client = FlameClient::new(self.channel.clone());

        let req = UnregisterApplicationRequest { name };

        let res = client
            .unregister_application(Request::new(req))
            .await?
            .into_inner();

        if res.return_code < 0 {
            Err(FlameError::Network(res.message.unwrap_or_default()))
        } else {
            Ok(())
        }
    }

    pub async fn list_application(&self) -> Result<Vec<Application>, FlameError> {
        let mut client = FlameClient::new(self.channel.clone());
        let app_list = client.list_application(ListApplicationRequest {}).await?;

        Ok(app_list
            .into_inner()
            .applications
            .iter()
            .map(Application::from)
            .collect())
    }

    pub async fn get_application(&self, name: &str) -> Result<Application, FlameError> {
        let mut client = FlameClient::new(self.channel.clone());
        let app = client
            .get_application(GetApplicationRequest {
                name: name.to_string(),
            })
            .await?;
        Ok(Application::from(&app.into_inner()))
    }

    pub async fn list_executor(&self) -> Result<Vec<Executor>, FlameError> {
        let mut client = FlameClient::new(self.channel.clone());
        let executor_list = client.list_executor(ListExecutorRequest {}).await?;
        Ok(executor_list
            .into_inner()
            .executors
            .iter()
            .map(Executor::from)
            .collect())
    }

    pub async fn list_node(&self) -> Result<Vec<Node>, FlameError> {
        let mut client = FlameClient::new(self.channel.clone());
        let node_list = client.list_nodes(ListNodesRequest {}).await?;
        Ok(node_list
            .into_inner()
            .nodes
            .iter()
            .map(Node::from)
            .collect())
    }

    pub async fn get_node(&self, name: &str) -> Result<Node, FlameError> {
        let mut client = FlameClient::new(self.channel.clone());
        let node = client
            .get_node(GetNodeRequest {
                name: name.to_string(),
            })
            .await?;
        let node = node
            .into_inner()
            .node
            .ok_or(FlameError::NotFound(format!("node <{}> not found", name)))?;
        Ok(Node::from(&node))
    }
}

impl Session {
    pub async fn create_task(&self, input: Option<TaskInput>) -> Result<Task, FlameError> {
        trace_fn!("Session::create_task");
        let mut client = self
            .client
            .clone()
            .ok_or(FlameError::Internal("no flame client".to_string()))?;

        let create_task_req = CreateTaskRequest {
            task: Some(TaskSpec {
                session_id: self.id.clone(),
                input: input.map(|input| input.to_vec()),
                output: None,
            }),
        };

        let task = client.create_task(create_task_req).await?;

        let task = task.into_inner();
        Ok(Task::from(&task))
    }

    pub async fn get_task(&self, id: &TaskID) -> Result<Task, FlameError> {
        trace_fn!("Session::get_task");
        let mut client = self
            .client
            .clone()
            .ok_or(FlameError::Internal("no flame client".to_string()))?;

        let get_task_req = GetTaskRequest {
            session_id: self.id.clone(),
            task_id: id.clone(),
        };
        let task = client.get_task(get_task_req).await?;

        let task = task.into_inner();
        Ok(Task::from(&task))
    }

    pub async fn list_tasks(&self) -> Result<Vec<Task>, FlameError> {
        // TODO (k82cn): Add top n tasks to avoid memory overflow.
        trace_fn!("Session::list_task");
        let mut client = self
            .client
            .clone()
            .ok_or(FlameError::Internal("no flame client".to_string()))?;
        let task_stream = client
            .list_task(Request::new(ListTaskRequest {
                session_id: self.id.to_string(),
            }))
            .await?;

        let mut task_list = vec![];

        let mut task_stream = task_stream.into_inner();
        while let Some(task) = task_stream.next().await {
            if let Ok(t) = task {
                task_list.push(Task::from(&t));
            }
        }

        Ok(task_list)
    }

    pub async fn run_task(
        &self,
        input: Option<TaskInput>,
        informer_ptr: TaskInformerPtr,
    ) -> Result<(), FlameError> {
        trace_fn!("Session::run_task");
        self.create_task(input)
            .and_then(|task| self.watch_task(task.ssn_id.clone(), task.id, informer_ptr))
            .await
    }

    pub async fn watch_task(
        &self,
        session_id: SessionID,
        task_id: TaskID,
        informer_ptr: TaskInformerPtr,
    ) -> Result<(), FlameError> {
        trace_fn!("Session::watch_task");
        let mut client = self
            .client
            .clone()
            .ok_or(FlameError::Internal("no flame client".to_string()))?;

        let watch_task_req = WatchTaskRequest {
            session_id,
            task_id,
        };
        let mut task_stream = client.watch_task(watch_task_req).await?.into_inner();
        while let Some(task) = task_stream.next().await {
            match task {
                Ok(t) => {
                    let mut informer = lock_ptr!(informer_ptr)?;
                    informer.on_update(Task::from(&t));
                }
                Err(e) => {
                    let mut informer = lock_ptr!(informer_ptr)?;
                    informer.on_error(FlameError::from(e.clone()));
                }
            }
        }
        Ok(())
    }

    pub async fn close(&self) -> Result<(), FlameError> {
        trace_fn!("Session::close");
        let mut client = self
            .client
            .clone()
            .ok_or(FlameError::Internal("no flame client".to_string()))?;

        let close_ssn_req = CloseSessionRequest {
            session_id: self.id.clone(),
        };

        client.close_session(close_ssn_req).await?;

        Ok(())
    }
}

impl From<&rpc::Task> for Task {
    fn from(task: &rpc::Task) -> Self {
        let metadata = task.metadata.clone().unwrap();
        let spec = task.spec.clone().unwrap();
        let status = task.status.clone().unwrap();
        Task {
            id: metadata.id,
            ssn_id: spec.session_id.clone(),
            input: spec.input.map(TaskInput::from),
            output: spec.output.map(TaskOutput::from),
            state: TaskState::try_from(status.state).unwrap_or(TaskState::default()),
            events: status.events.clone().into_iter().map(Event::from).collect(),
        }
    }
}

impl From<&rpc::Session> for Session {
    fn from(ssn: &rpc::Session) -> Self {
        let metadata = ssn.metadata.clone().unwrap();
        let status = ssn.status.clone().unwrap();
        let spec = ssn.spec.clone().unwrap();

        let naivedatetime_utc =
            DateTime::from_timestamp_millis(status.creation_time * 1000).unwrap();
        let creation_time = Utc.from_utc_datetime(&naivedatetime_utc.naive_utc());

        Session {
            client: None,
            id: metadata.id,
            slots: spec.slots,
            application: spec.application,
            creation_time,
            state: SessionState::try_from(status.state).unwrap_or(SessionState::default()),
            pending: status.pending,
            running: status.running,
            succeed: status.succeed,
            failed: status.failed,
            events: status.events.clone().into_iter().map(Event::from).collect(),
            tasks: None,
        }
    }
}

impl From<&rpc::Event> for Event {
    fn from(event: &rpc::Event) -> Self {
        let second = event.creation_time / 1000;
        let nanosecond = ((event.creation_time % 1000) * 1_000_000) as u32;

        Self {
            code: event.code,
            message: event.message.clone(),
            creation_time: DateTime::from_timestamp(second, nanosecond).unwrap(),
        }
    }
}

impl From<rpc::Event> for Event {
    fn from(event: rpc::Event) -> Self {
        Event::from(&event)
    }
}

impl From<&rpc::Application> for Application {
    fn from(app: &rpc::Application) -> Self {
        let metadata = app.metadata.clone().unwrap();
        let spec = app.spec.clone().unwrap();
        let status = app.status.unwrap();

        let naivedatetime_utc =
            DateTime::from_timestamp_millis(status.creation_time * 1000).unwrap();
        let creation_time = Utc.from_utc_datetime(&naivedatetime_utc.naive_utc());

        Self {
            name: metadata.name,
            attributes: ApplicationAttributes::from(spec),
            state: ApplicationState::from(status.state()),
            creation_time,
        }
    }
}

impl From<ApplicationAttributes> for ApplicationSpec {
    fn from(app: ApplicationAttributes) -> Self {
        Self {
            shim: app.shim.map(|s| s as i32).unwrap_or(0),
            image: app.image.clone(),
            description: app.description.clone(),
            labels: app.labels.clone(),
            command: app.command.clone(),
            arguments: app.arguments.clone(),
            environments: app
                .environments
                .clone()
                .into_iter()
                .map(|(key, value)| Environment { name: key, value })
                .collect(),
            working_directory: app.working_directory.clone(),
            max_instances: app.max_instances,
            delay_release: app.delay_release.map(|s| s.num_seconds()),
            schema: app.schema.clone().map(rpc::ApplicationSchema::from),
            url: app.url.clone(),
        }
    }
}

impl From<ApplicationSpec> for ApplicationAttributes {
    fn from(app: ApplicationSpec) -> Self {
        Self {
            shim: Some(Shim::from(
                rpc::Shim::try_from(app.shim).unwrap_or(rpc::Shim::Host),
            )),
            image: app.image.clone(),
            description: app.description.clone(),
            labels: app.labels.clone(),
            command: app.command.clone(),
            arguments: app.arguments.clone(),
            environments: app
                .environments
                .clone()
                .into_iter()
                .map(|env| (env.name, env.value))
                .collect(),
            working_directory: app.working_directory.clone().filter(|wd| !wd.is_empty()),
            max_instances: app.max_instances,
            delay_release: app.delay_release.map(Duration::seconds),
            schema: app.schema.clone().map(ApplicationSchema::from),
            url: app.url.clone(),
        }
    }
}

impl From<ApplicationSchema> for rpc::ApplicationSchema {
    fn from(schema: ApplicationSchema) -> Self {
        Self {
            input: schema.input,
            output: schema.output,
            common_data: schema.common_data,
        }
    }
}

impl From<rpc::ApplicationSchema> for ApplicationSchema {
    fn from(schema: rpc::ApplicationSchema) -> Self {
        Self {
            input: schema.input,
            output: schema.output,
            common_data: schema.common_data,
        }
    }
}

impl From<&rpc::Executor> for Executor {
    fn from(e: &rpc::Executor) -> Self {
        let spec = e.spec.clone().unwrap();
        let status = e.status.clone().unwrap();
        let metadata = e.metadata.clone().unwrap();

        let state = rpc::ExecutorState::try_from(status.state).unwrap().into();

        Executor {
            id: metadata.id,
            session_id: status.session_id,
            slots: spec.slots,
            node: spec.node,
            state,
        }
    }
}

impl From<rpc::Executor> for Executor {
    fn from(e: rpc::Executor) -> Self {
        Executor::from(&e)
    }
}

impl From<&rpc::Node> for Node {
    fn from(n: &rpc::Node) -> Self {
        let metadata = n.metadata.clone().unwrap_or_default();
        let spec = n.spec.clone().unwrap_or_default();
        let status = n.status.clone().unwrap_or_default();

        let state = match rpc::NodeState::try_from(status.state) {
            Ok(rpc::NodeState::Ready) => NodeState::Ready,
            Ok(rpc::NodeState::NotReady) => NodeState::NotReady,
            _ => NodeState::Unknown,
        };

        let capacity = status.capacity.unwrap_or_default();
        let info = status.info.unwrap_or_default();

        Node {
            name: metadata.name,
            hostname: spec.hostname,
            state,
            cpu: capacity.cpu,
            memory: capacity.memory,
            arch: info.arch,
            os: info.os,
        }
    }
}

impl From<rpc::Node> for Node {
    fn from(n: rpc::Node) -> Self {
        Node::from(&n)
    }
}

mod serde_duration {
    use chrono::Duration;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match duration {
            Some(duration) => serializer.serialize_i64(duration.num_seconds()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let seconds = i64::deserialize(deserializer)?;
        Ok(Some(Duration::seconds(seconds)))
    }
}

mod serde_utc {
    use chrono::{DateTime, Utc};
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(date: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(date.timestamp())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let timestamp = i64::deserialize(deserializer)?;
        DateTime::<Utc>::from_timestamp(timestamp, 0)
            .ok_or(serde::de::Error::custom("invalid timestamp"))
    }
}

mod serde_message {
    use bytes::Bytes;
    use prost::Message;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(message: &Option<Bytes>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match message {
            Some(message) => serializer.serialize_str(String::from_utf8_lossy(message).as_ref()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Bytes>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data: String = String::deserialize(deserializer)?;
        Ok(Some(Bytes::from(data.encode_to_vec())))
    }
}
