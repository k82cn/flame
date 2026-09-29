# Frontend Service

The Frontend service is the client-facing API for Flame. It handles session management, task operations, and application registration.

## Service Definition

```protobuf
service Frontend {
  // Workspaces
  rpc CreateWorkspace(CreateWorkspaceRequest) returns (Workspace) {}
  rpc ListWorkspaces(ListWorkspacesRequest) returns (WorkspaceList) {}

  // Application Management
  rpc RegisterApplication(RegisterApplicationRequest) returns (Application) {}
  rpc UnregisterApplication(UnregisterApplicationRequest) returns (Result) {}
  rpc UpdateApplication(UpdateApplicationRequest) returns (Result) {}
  rpc GetApplication(GetApplicationRequest) returns (Application) {}
  rpc ListApplications(ListApplicationsRequest) returns (ApplicationList) {}

  // Executor Listing
  rpc ListExecutors(ListExecutorsRequest) returns (ExecutorList) {}

  // Node Operations
  rpc ListNodes(ListNodesRequest) returns (NodeList) {}
  rpc GetNode(GetNodeRequest) returns (GetNodeResponse) {}

  // Session Management
  rpc CreateSession(CreateSessionRequest) returns (Session) {}
  rpc DeleteSession(DeleteSessionRequest) returns (Session) {}
  rpc OpenSession(OpenSessionRequest) returns (Session) {}
  rpc CloseSession(CloseSessionRequest) returns (Session) {}
  rpc GetSession(GetSessionRequest) returns (Session) {}
  rpc ListSessions(ListSessionsRequest) returns (SessionList) {}

  // Task Operations
  rpc CreateTask(CreateTaskRequest) returns (Task) {}
  rpc GetTask(GetTaskRequest) returns (Task) {}
  rpc WatchTasks(stream WatchTaskRequest) returns (stream Task) {}
  rpc ListTasks(ListTasksRequest) returns (stream Task) {}
}
```

## Workspaces

`CreateWorkspaceRequest` contains the workspace `name`. Only `flmadmin` can
create a workspace. `ListWorkspacesRequest` has no fields and returns a
`WorkspaceList`.

## Application Management

### RegisterApplication

Registers a new application with Flame.

**Request:** `RegisterApplicationRequest`

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Unique name for the application |
| `application` | [ApplicationSpec](types.md#applicationspec) | Application specification |
| `workspace` | string | Workspace name; empty selects the default workspace |

**Response:** [Application](types.md#application)

**Example:**
```python
import flamepy

flamepy.register_application("my-app", {
    "shim": flamepy.Shim.HOST,
    "image": "my-registry/my-app:latest",
    "command": "/usr/bin/my-app",
})
```

### UnregisterApplication

Removes an application registration.

**Request:** `UnregisterApplicationRequest`

| Field | Type | Description |
|-------|------|-------------|
| `application` | string | Application name to unregister |
| `workspace` | string | Owning workspace; empty selects `default` |

**Response:** [Result](types.md#result)

### UpdateApplication

Updates an existing application registration.

**Request:** `UpdateApplicationRequest`

| Field | Type | Description |
|-------|------|-------------|
| `application` | string | Application name to update |
| `spec` | [ApplicationSpec](types.md#applicationspec) | Replacement application specification |
| `workspace` | string | Owning workspace; empty selects `default` |

**Response:** [Result](types.md#result)

### GetApplication

Retrieves application details by workspace and name.

**Request:** `GetApplicationRequest`

| Field | Type | Description |
|-------|------|-------------|
| `application` | string | Application name |
| `workspace` | string | Owning workspace; empty selects `default` |

**Response:** [Application](types.md#application)

### ListApplications

Lists registered applications, optionally filtered by state, name, and workspace.

**Request:** `ListApplicationsRequest`

| Field | Type | Description |
|-------|------|-------------|
| `state` | optional [ApplicationState](types.md#applicationstate) | Application state filter |
| `name` | optional string | Application name filter |
| `workspace` | optional string | Workspace name filter |

**Response:** [ApplicationList](types.md#applicationlist)

## Session Management

### CreateSession

Creates a new session for task execution.

**Request:** `CreateSessionRequest`

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Caller-chosen session name within its workspace |
| `session` | [SessionSpec](types.md#sessionspec) | Session specification |
| `workspace` | string | Owning workspace; empty selects `default` |

**Response:** [Session](types.md#session)

**Example:**
```python
import flamepy

session = flamepy.create_session(
    application="my-app",
    workspace="default",
    name="run-1",
    resreq=flamepy.ResourceRequirement.from_string("cpu=1,mem=1g"),
    min_instances=2,
    max_instances=10,
)
```

### DeleteSession

Deletes a session and its persisted task records.

**Request:** `DeleteSessionRequest`

| Field | Type | Description |
|-------|------|-------------|
| `session` | string | Session name to delete |
| `workspace` | string | Owning workspace; empty selects `default` |

**Response:** [Session](types.md#session)

### OpenSession

Opens an existing session or creates one if spec is provided.

**Request:** `OpenSessionRequest`

| Field | Type | Description |
|-------|------|-------------|
| `session` | string | Session name to open |
| `spec` | [SessionSpec](types.md#sessionspec) | Optional spec for creation |
| `workspace` | string | Owning workspace; empty selects `default` |

**Response:** [Session](types.md#session)

### CloseSession

Closes a session, preventing new task submissions.

**Request:** `CloseSessionRequest`

| Field | Type | Description |
|-------|------|-------------|
| `session` | string | Session name to close |
| `workspace` | string | Owning workspace; empty selects `default` |

**Response:** [Session](types.md#session)

### GetSession

Retrieves session details.

**Request:** `GetSessionRequest`

| Field | Type | Description |
|-------|------|-------------|
| `session` | string | Session name |
| `workspace` | string | Owning workspace; empty selects `default` |

**Response:** [Session](types.md#session)

### ListSessions

Lists sessions in a workspace, optionally filtered by application name, state, and session name.

**Request:** `ListSessionsRequest`

| Field | Type | Description |
|-------|------|-------------|
| `application` | optional string | Application name filter |
| `state` | optional [SessionState](types.md#sessionstate) | Session state filter |
| `name` | optional string | Session name filter |
| `workspace` | string | Owning workspace; empty selects `default` |

**Response:** [SessionList](types.md#sessionlist)

## Task Operations

### CreateTask

Creates a new task within a session.

**Request:** `CreateTaskRequest`

| Field | Type | Description |
|-------|------|-------------|
| `task` | [TaskSpec](types.md#taskspec) | Task specification |

**Response:** [Task](types.md#task)

**Example:**
```python
task = session.create_task(b"input data")
```

### GetTask

Retrieves task details.

**Request:** `GetTaskRequest`

| Field | Type | Description |
|-------|------|-------------|
| `task` | int64 | Server-assigned task number |
| `session` | string | Parent session name |
| `workspace` | string | Owning workspace; empty selects `default` |

**Response:** [Task](types.md#task)

### WatchTasks

Registers task numbers on one bidirectional stream and returns status updates for
those tasks. The first response for each registered task is its current status,
which may already be terminal. The stream then sends later status updates until
the task reaches a terminal state. Intermediate updates may be coalesced under
load, so callers should use the latest received status rather than expect every
transition. All registrations on a stream must use the same workspace and session.
Closing the request side after registration still allows outstanding task
updates to arrive.

**Request:** `stream WatchTaskRequest`

| Field | Type | Description |
|-------|------|-------------|
| `task` | int64 | Server-assigned task number to register |
| `session` | string | Parent session name |
| `workspace` | string | Owning workspace; empty selects `default` |

**Response:** `stream` [Task](types.md#task)

**Example:**
```python
for update in session.watch_task(task.name):
    # The first update is the current status, not necessarily Pending.
    print(f"State: {update.state}")
    if update.is_completed():
        break
```

### ListTasks

Streams all tasks in a session.

**Request:** `ListTasksRequest`

| Field | Type | Description |
|-------|------|-------------|
| `session` | string | Parent session name |
| `workspace` | string | Owning workspace; empty selects `default` |

**Response:** `stream` [Task](types.md#task)

## Node Operations

### ListNodes

Lists all registered nodes in the cluster.

**Request:** `ListNodesRequest` (empty)

**Response:** [NodeList](types.md#nodelist)

### GetNode

Retrieves details for a specific node.

**Request:** `GetNodeRequest`

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Node name |

**Response:** `GetNodeResponse`

| Field | Type | Description |
|-------|------|-------------|
| `node` | [Node](types.md#node) | Node details |

## Executor Operations

### ListExecutors

Lists executors in a workspace, optionally filtered by application.

**Request:** `ListExecutorsRequest`

| Field | Type | Description |
|-------|------|-------------|
| `application` | optional string | Application name filter |
| `workspace` | string | Owning workspace; empty selects `default` |

**Response:** [ExecutorList](types.md#executorlist)
