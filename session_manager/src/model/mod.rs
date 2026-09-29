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

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use bytes::Bytes;
use chrono::{DateTime, Duration, Utc};
use stdng::lock_ptr;

use common::apis::{
    Application, ApplicationState, Executor, ExecutorID, ExecutorState, Node, NodeState,
    ResourceRequirement, Session, SessionPath, SessionState, Shim, Task, TaskName, TaskState,
};
use common::FlameError;

pub type SessionInfoPtr = Arc<SessionInfo>;
pub type TaskInfoPtr = Arc<TaskInfo>;
pub type ExecutorInfoPtr = Arc<ExecutorInfo>;
pub type NodeInfoPtr = Arc<NodeInfo>;
pub type AppInfoPtr = Arc<AppInfo>;

#[derive(Debug, Default, Clone)]
pub struct TaskInfo {
    pub task: TaskName,
    pub session: SessionPath,

    pub creation_time: DateTime<Utc>,
    pub completion_time: Option<DateTime<Utc>>,

    pub state: TaskState,
    pub affinity: HashSet<Bytes>,
}

#[derive(Debug, Default, Clone)]
pub struct SessionInfo {
    pub session: SessionPath,
    pub application: String,

    pub tasks_status: HashMap<TaskState, i32>,

    pub creation_time: DateTime<Utc>,
    pub completion_time: Option<DateTime<Utc>>,

    pub state: SessionState,
    pub min_instances: u32,
    pub max_instances: Option<u32>,
    pub batch_size: u32,
    pub priority: u32,
    pub resreq: Option<ResourceRequirement>,
    pub retry_count: u32,
    pub task_index: HashMap<TaskState, BTreeMap<TaskName, TaskInfoPtr>>,
}

impl SessionInfo {
    pub fn is_ready(&self, retry_limits: u32) -> bool {
        self.retry_count < retry_limits
    }
}

#[derive(Clone, Debug, Default)]
pub struct ExecutorInfo {
    pub id: ExecutorID,
    pub node: String,
    pub resreq: ResourceRequirement,
    pub shim: Shim,
    /// Application owning the retained service instance.
    pub application: String,
    pub task: Option<TaskName>,
    pub session: Option<SessionPath>,

    pub creation_time: DateTime<Utc>,
    /// Last in-memory lifecycle update, used to age Idle retained instances.
    pub latest_updated_timestamp: DateTime<Utc>,
    pub state: ExecutorState,
    /// Last accepted volatile attributes for the retained service instance.
    pub attributes: HashSet<Bytes>,
}

#[derive(Clone, Debug, Default)]
pub struct NodeInfo {
    pub name: String,
    pub allocatable: ResourceRequirement,
    pub state: NodeState,
}

#[derive(Clone, Debug, Default)]
pub struct AppInfo {
    pub name: String,
    pub state: ApplicationState,
    pub shim: Shim, // Required shim type for the application
    pub max_instances: u32,
    pub delay_release: Duration,
}

impl From<Application> for AppInfo {
    fn from(app: Application) -> Self {
        AppInfo::from(&app)
    }
}

impl From<&Node> for NodeInfo {
    fn from(node: &Node) -> Self {
        NodeInfo {
            name: node.name.clone(),
            allocatable: node.allocatable.clone(),
            state: node.state,
        }
    }
}

impl From<&Application> for AppInfo {
    fn from(app: &Application) -> Self {
        AppInfo {
            name: app.gid.to_string(),
            state: app.state,
            shim: app.shim, // Get shim from application
            max_instances: app.max_instances,
            delay_release: app.delay_release,
        }
    }
}

impl From<&Executor> for ExecutorInfo {
    fn from(exec: &Executor) -> Self {
        ExecutorInfo {
            id: exec.id.clone(),
            node: exec.node.clone(),
            resreq: exec.resreq.clone(),
            shim: exec.shim,
            application: exec.application.clone(),
            task: exec.task,
            session: exec.session.clone(),
            creation_time: exec.creation_time,
            latest_updated_timestamp: exec.latest_updated_timestamp,
            state: exec.state,
            attributes: exec.attributes.clone(),
        }
    }
}

impl From<&Task> for TaskInfo {
    fn from(task: &Task) -> Self {
        TaskInfo {
            task: task.number,
            session: task.session.clone(),
            creation_time: task.creation_time,
            completion_time: task.completion_time,
            state: task.state,
            affinity: task.affinity.clone(),
        }
    }
}

impl TryFrom<&Session> for SessionInfo {
    type Error = FlameError;

    fn try_from(ssn: &Session) -> Result<Self, Self::Error> {
        let mut tasks_status = HashMap::new();
        for (k, v) in &ssn.tasks_index {
            tasks_status.insert(*k, v.len() as i32);
        }
        let mut task_index = HashMap::new();
        for (state, tasks) in ssn
            .tasks_index
            .iter()
            .filter(|(state, _)| !state.is_terminal())
        {
            let mut task_infos = BTreeMap::new();
            for (task, task_data) in tasks {
                task_infos.insert(*task, Arc::new(TaskInfo::from(&*lock_ptr!(task_data)?)));
            }
            task_index.insert(*state, task_infos);
        }

        Ok(SessionInfo {
            session: ssn.gid.clone(),
            application: ssn.application.clone(),
            tasks_status,
            creation_time: ssn.creation_time,
            completion_time: ssn.completion_time,
            state: ssn.status.state,
            min_instances: ssn.min_instances,
            max_instances: ssn.max_instances,
            batch_size: 1,
            priority: ssn.priority,
            resreq: ssn.resreq.clone(),
            retry_count: ssn.retry_count,
            task_index,
        })
    }
}

/// Filter for listing nodes.
/// All fields are Option:
/// - `None` = ignore this filter (match all)
/// - `Some(value)` = match exactly (empty vec matches nothing)
pub struct NodeFilter {
    /// Filter by node state
    pub state: Option<NodeState>,
    /// Filter by node names
    pub names: Option<Vec<String>>,
}

impl NodeFilter {
    /// Creates a new empty filter (matches all nodes).
    pub const fn new() -> Self {
        Self {
            state: None,
            names: None,
        }
    }

    /// Creates a filter for a specific state.
    pub const fn by_state(state: NodeState) -> Self {
        Self {
            state: Some(state),
            names: None,
        }
    }

    /// Creates a filter for specific node names.
    pub fn by_names(names: Vec<String>) -> Self {
        Self {
            state: None,
            names: Some(names),
        }
    }
}

impl Default for NodeFilter {
    fn default() -> Self {
        Self::new()
    }
}

pub const ALL_NODE: Option<NodeFilter> = None;
