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

use super::{ApplicationState, ExecutorState, SessionGID, SessionState, TaskState};
use crate::FlameError;
use rpc::flame::v1 as rpc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionPredicate {
    Ready,
}

impl SessionPredicate {
    pub fn matches(self, retry_count: u32, retry_limits: u32) -> bool {
        match self {
            Self::Ready => retry_count < retry_limits,
        }
    }
}

/// Filter for tasks owned by one session.
pub struct TaskFilter {
    pub workspace: String,
    /// Owning session.
    pub session: String,
    /// Task states to include. `None` matches every state.
    pub states: Option<Vec<TaskState>>,
}

impl TaskFilter {
    /// Returns the owning session's globally scoped name.
    pub fn session(&self) -> SessionGID {
        SessionGID::new(&self.workspace, &self.session)
    }

    /// Creates a filter for every task in a session.
    pub fn by_session(workspace: impl Into<String>, session: impl Into<String>) -> Self {
        Self {
            workspace: workspace.into(),
            session: session.into(),
            states: None,
        }
    }

    /// Creates a filter for tasks in any of the provided states.
    pub fn by_session_states(
        workspace: impl Into<String>,
        session: impl Into<String>,
        states: impl Into<Vec<TaskState>>,
    ) -> Self {
        Self {
            workspace: workspace.into(),
            session: session.into(),
            states: Some(states.into()),
        }
    }

    /// Creates a filter for non-terminal tasks in a session.
    pub fn non_terminal(workspace: impl Into<String>, session: impl Into<String>) -> Self {
        Self::by_session_states(
            workspace,
            session,
            vec![TaskState::Pending, TaskState::Running],
        )
    }
}

/// Filter for listing sessions.
/// All fields are Option:
/// - `None` = ignore this filter (match all)
/// - `Some(value)` = match exactly (empty vec matches nothing)
pub struct SessionFilter {
    pub workspace: Option<String>,
    /// Filter by owning application
    pub application: Option<String>,
    /// Filter by session state
    pub state: Option<SessionState>,
    /// Filter by session names
    pub names: Option<Vec<String>>,
    /// Additional in-memory predicate filter.
    pub predicate: Option<SessionPredicate>,
    /// Maximum number of matching sessions to return.
    pub limit: Option<usize>,
}

impl SessionFilter {
    /// Returns explicitly named sessions when a workspace is specified.
    /// `None` means the filter does not identify scoped sessions;
    /// an empty vector means no sessions match the name filter.
    pub fn session(&self) -> Option<Vec<SessionGID>> {
        let workspace = self.workspace.as_ref()?;
        Some(
            self.names
                .as_ref()?
                .iter()
                .map(|name| SessionGID::new(workspace, name))
                .collect(),
        )
    }

    /// Creates a new empty filter (matches all sessions).
    pub const fn new() -> Self {
        Self {
            workspace: None,
            application: None,
            state: None,
            names: None,
            predicate: None,
            limit: None,
        }
    }

    /// Creates a filter for a specific state.
    pub const fn by_state(state: SessionState) -> Self {
        Self {
            workspace: None,
            application: None,
            state: Some(state),
            names: None,
            predicate: None,
            limit: None,
        }
    }

    /// Creates a filter for specific session names.
    pub fn by_names(names: Vec<String>) -> Self {
        Self {
            workspace: None,
            application: None,
            state: None,
            names: Some(names),
            predicate: None,
            limit: None,
        }
    }

    /// Creates a filter for an application's sessions.
    pub fn by_application(application: impl Into<String>) -> Self {
        Self {
            workspace: None,
            application: Some(application.into()),
            state: None,
            names: None,
            predicate: None,
            limit: None,
        }
    }

    /// Creates a filter for an application's sessions in a specific state.
    pub fn by_application_state(application: impl Into<String>, state: SessionState) -> Self {
        Self {
            workspace: None,
            application: Some(application.into()),
            state: Some(state),
            names: None,
            predicate: None,
            limit: None,
        }
    }

    /// Adds an in-memory predicate filter.
    pub const fn with_predicate(mut self, predicate: SessionPredicate) -> Self {
        self.predicate = Some(predicate);
        self
    }

    /// Limits the number of matching sessions returned.
    pub const fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

impl Default for SessionFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl TryFrom<rpc::ListSessionsRequest> for SessionFilter {
    type Error = FlameError;

    fn try_from(request: rpc::ListSessionsRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            workspace: request.workspace,
            application: request.application,
            state: request.state.map(SessionState::try_from).transpose()?,
            names: None,
            predicate: None,
            limit: None,
        })
    }
}

pub const OPEN_SESSION: Option<SessionFilter> = Some(SessionFilter::by_state(SessionState::Open));
pub const READY_SESSION: Option<SessionFilter> =
    Some(SessionFilter::by_state(SessionState::Open).with_predicate(SessionPredicate::Ready));

/// Filter for listing executors.
/// All fields are Option:
/// - `None` = ignore this filter (match all)
/// - `Some(value)` = match exactly (empty vec/string matches nothing)
pub struct ExecutorFilter {
    /// Filter by executor state
    pub state: Option<ExecutorState>,
    /// Filter by executor names
    pub names: Option<Vec<String>>,
    /// Filter by node name
    pub node: Option<String>,
}

impl ExecutorFilter {
    /// Creates a new empty filter (matches all executors).
    pub const fn new() -> Self {
        Self {
            state: None,
            names: None,
            node: None,
        }
    }

    /// Creates a filter for a specific state.
    pub const fn by_state(state: ExecutorState) -> Self {
        Self {
            state: Some(state),
            names: None,
            node: None,
        }
    }

    /// Creates a filter for a specific node.
    pub fn by_node(node: impl Into<String>) -> Self {
        Self {
            state: None,
            names: None,
            node: Some(node.into()),
        }
    }

    /// Creates a filter for specific executor names.
    pub fn by_names(names: Vec<String>) -> Self {
        Self {
            state: None,
            names: Some(names),
            node: None,
        }
    }
}

impl Default for ExecutorFilter {
    fn default() -> Self {
        Self::new()
    }
}

pub const IDLE_EXECUTOR: Option<ExecutorFilter> =
    Some(ExecutorFilter::by_state(ExecutorState::Idle));
pub const VOID_EXECUTOR: Option<ExecutorFilter> =
    Some(ExecutorFilter::by_state(ExecutorState::Void));
pub const UNBINDING_EXECUTOR: Option<ExecutorFilter> =
    Some(ExecutorFilter::by_state(ExecutorState::Unbinding));
pub const BOUND_EXECUTOR: Option<ExecutorFilter> =
    Some(ExecutorFilter::by_state(ExecutorState::Bound));
pub const BINDING_EXECUTOR: Option<ExecutorFilter> =
    Some(ExecutorFilter::by_state(ExecutorState::Binding));

pub const ALL_EXECUTOR: Option<ExecutorFilter> = None;

/// Filter for listing applications.
/// All fields are Option:
/// - `None` = ignore this filter (match all)
/// - `Some(value)` = match exactly
pub struct ApplicationFilter {
    pub workspace: Option<String>,
    /// Filter by application state
    pub state: Option<ApplicationState>,
}

impl ApplicationFilter {
    /// Creates a new empty filter (matches all applications).
    pub const fn new() -> Self {
        Self {
            workspace: None,
            state: None,
        }
    }

    /// Creates a filter for a specific application state.
    pub const fn by_state(state: ApplicationState) -> Self {
        Self {
            workspace: None,
            state: Some(state),
        }
    }
}

impl Default for ApplicationFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl TryFrom<rpc::ListApplicationsRequest> for ApplicationFilter {
    type Error = FlameError;

    fn try_from(request: rpc::ListApplicationsRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            workspace: request.workspace,
            state: request.state.map(ApplicationState::try_from).transpose()?,
        })
    }
}

pub const ALL_APPLICATION: Option<ApplicationFilter> = None;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_filter_preserves_optional_scope_and_multiple_names() {
        let mut filter = SessionFilter::new();
        assert_eq!(filter.session(), None);
        filter.workspace = Some("research".to_string());
        assert_eq!(filter.session(), None);
        filter.names = Some(Vec::new());
        assert_eq!(filter.session(), Some(Vec::new()));
        filter.names = Some(vec!["first".to_string()]);
        assert_eq!(
            filter.session(),
            Some(vec![SessionGID::new("research", "first")])
        );
        filter.names.as_mut().unwrap().push("second".to_string());
        assert_eq!(
            filter.session(),
            Some(vec![
                SessionGID::new("research", "first"),
                SessionGID::new("research", "second"),
            ])
        );
        filter.workspace = None;
        assert_eq!(filter.session(), None);
    }
}
