# Flame Python SDK API Reference

## Overview

The Flame Python SDK provides a high-level interface for interacting with the Flame distributed computing framework. It uses synchronous gRPC clients and provides comprehensive error handling.

## Core Classes

### Connection

The main entry point for connecting to Flame services.

#### Methods

##### `connect(addr: str) -> Connection`
Establishes a connection to the Flame service.

**Parameters:**
- `addr` (str): The address of the Flame service (e.g., "http://localhost:8080")

**Returns:**
- `Connection`: A connection instance

**Raises:**
- `FlameError`: If the connection fails

**Example:**
```python
conn = Connection.connect("http://localhost:8080")
```

##### `close() -> None`
Closes the connection to the Flame service.

**Example:**
```python
conn.close()
```

##### `register_application(name: str, app_attrs: Union[ApplicationAttributes, Dict[str, Any]]) -> None`
Registers a new application with the Flame service.

**Parameters:**
- `name` (str): The name of the application
- `app_attrs` (Union[ApplicationAttributes, Dict[str, Any]]): Application attributes

**Example:**
```python
conn.register_application("my-app", {
    "shim": Shim.SHELL,
    "command": "python",
    "arguments": ["script.py"]
})
```

##### `unregister_application(name: str) -> None`
Unregisters an application from the Flame service.

**Parameters:**
- `name` (str): The name of the application to unregister

##### `list_applications() -> List[Application]`
Lists all registered applications.

**Returns:**
- `List[Application]`: List of applications

##### `create_session(attrs: SessionAttributes) -> Session`
Creates a new session.

**Parameters:**
- `attrs` (SessionAttributes): Session attributes

**Returns:**
- `Session`: The created session

##### `list_sessions() -> List[Session]`
Lists all sessions.

**Returns:**
- `List[Session]`: List of sessions

##### `get_session(session_id: SessionID) -> Session`
Gets a session by ID.

**Parameters:**
- `session_id` (SessionID): The session ID

**Returns:**
- `Session`: The session

##### `close_session(session_id: SessionID) -> Session`
Closes a session.

**Parameters:**
- `session_id` (SessionID): The session ID

**Returns:**
- `Session`: The closed session

### SessionClient

Client for session-specific operations.

#### Methods

##### `create_task(input_data: TaskInput) -> Task`
Creates a new task in the session.

**Parameters:**
- `input_data` (TaskInput): Task input data

**Returns:**
- `Task`: The created task

##### `get_task(task_id: TaskID) -> Task`
Gets a task by ID.

**Parameters:**
- `task_id` (TaskID): The task ID

**Returns:**
- `Task`: The task

##### `watch_task(task_id: TaskID) -> TaskWatcher`
Watches a task for updates.

**Parameters:**
- `task_id` (TaskID): The task ID

**Returns:**
- `TaskWatcher`: Iterator for task updates

##### `invoke(input_data: Any, informer: Optional[TaskInformer] = None) -> Any`
Invokes a task synchronously with the given input and optional informer.

This method blocks until the task completes or fails.

**Parameters:**
- `input_data` (Any): The input data for the task
- `informer` (Optional[TaskInformer]): Optional task informer for monitoring task progress

**Returns:**
- The task output (or None if informer is provided)

**Example:**
```python
result = session.invoke(b"input data")
print(result)
```

**Example (With Informer):**
```python
class MyInformer(flamepy.TaskInformer):
    def on_update(self, task):
        print(f"Task {task.id}: {task.state}")

informer = MyInformer()
session.invoke(b"input data", informer=informer)
```

##### `run(input_data: Any, informer: Optional[TaskInformer] = None) -> Future`
Runs a task asynchronously and returns a Future for async-style execution.

This method returns immediately with a Future object that can be used to retrieve the result later or run multiple tasks in parallel.

**Parameters:**
- `input_data` (Any): The input data for the task
- `informer` (Optional[TaskInformer]): Optional task informer for monitoring task progress

**Returns:**
- A `concurrent.futures.Future` object that will contain the result when the task completes

**Example (Single Task):**
```python
future = session.run(b"input data")
# Do other work...
result = future.result()  # Wait for completion
print(result)
```

**Example (Parallel Execution):**
```python
from concurrent.futures import wait

# Run multiple tasks in parallel
futures = [session.run(f"input {i}".encode()) for i in range(10)]

# Wait for all tasks to complete
wait(futures)
results = [f.result() for f in futures]
print(f"Completed {len(results)} tasks")
```

**Example (With Callbacks):**
```python
def task_done_callback(future):
    try:
        result = future.result()
        print(f"Task completed: {result}")
    except Exception as e:
        print(f"Task failed: {e}")

future = session.run(b"input data")
future.add_done_callback(task_done_callback)
```

## Data Types

### SessionAttributes

Attributes for creating a session.

**Fields:**
- `application` (str): The application name
- `slots` (int): Number of slots
- `common_data` (Optional[bytes]): Common data for the session

### ApplicationAttributes

Attributes for an application.

**Fields:**
- `name` (str): Application name
- `shim` (Shim): Shim type
- `url` (Optional[str]): URL
- `command` (Optional[str]): Command
- `arguments` (Optional[List[str]]): Command arguments
- `environments` (Optional[List[str]]): Environment variables
- `working_directory` (Optional[str]): Working directory

### Session

Represents a computing session.

**Fields:**
- `id` (SessionID): Session ID
- `application` (str): Application name
- `slots` (int): Number of slots
- `state` (SessionState): Session state
- `creation_time` (datetime): Creation time
- `pending` (int): Number of pending tasks
- `running` (int): Number of running tasks
- `succeed` (int): Number of succeeded tasks
- `failed` (int): Number of failed tasks
- `completion_time` (Optional[datetime]): Completion time

### Task

Represents a computing task.

**Fields:**
- `id` (TaskID): Task ID
- `session_id` (SessionID): Session ID
- `state` (TaskState): Task state
- `creation_time` (datetime): Creation time
- `input` (Optional[bytes]): Task input
- `output` (Optional[bytes]): Task output
- `completion_time` (Optional[datetime]): Completion time

**Methods:**
- `is_completed() -> bool`: Check if the task is completed

### Application

Represents a distributed application.

**Fields:**
- `id` (ApplicationID): Application ID
- `name` (str): Application name
- `shim` (Shim): Shim type
- `state` (ApplicationState): Application state
- `creation_time` (datetime): Creation time
- `url` (Optional[str]): URL
- `command` (Optional[str]): Command
- `arguments` (Optional[List[str]]): Arguments
- `environments` (Optional[List[str]]): Environment variables
- `working_directory` (Optional[str]): Working directory

## Enums

### SessionState
- `OPEN = 0`: Session is open
- `CLOSED = 1`: Session is closed

### TaskState
- `PENDING = 0`: Task is pending
- `RUNNING = 1`: Task is running
- `SUCCEED = 2`: Task succeeded
- `FAILED = 3`: Task failed

### ApplicationState
- `ENABLED = 0`: Application is enabled
- `DISABLED = 1`: Application is disabled

### Shim
- `Host = 0`: Host shim
- `WASM = 1`: WebAssembly shim

### FlameErrorCode
- `INVALID_CONFIG = 0`: Invalid configuration
- `INVALID_STATE = 1`: Invalid state
- `INVALID_ARGUMENT = 2`: Invalid argument
- `INTERNAL = 3`: Internal error

## Error Handling

### FlameError

Custom exception for Flame SDK errors.

**Attributes:**
- `code` (FlameErrorCode): Error code
- `message` (str): Error message

**Example:**
```python
try:
    conn = Connection.connect("invalid://address")
except FlameError as e:
    if e.code == FlameErrorCode.INVALID_CONFIG:
        print(f"Configuration error: {e.message}")
```

## Task Monitoring

### TaskInformer

Interface for task updates.

**Methods:**
- `on_update(task: Task) -> None`: Called when a task is updated
- `on_error(error: FlameError) -> None`: Called when an error occurs

**Example:**
```python
class MyTaskInformer(TaskInformer):
    def on_update(self, task):
        print(f"Task {task.id}: {task.state}")
    
    def on_error(self, error):
        print(f"Error: {error}")

informer = MyTaskInformer()
session_client.run_task(b"input data", informer)
```

### TaskWatcher

Iterator for watching task updates.

**Example:**
```python
for update in session_client.watch_task(task.id):
    print(f"Task state: {update.state}")
    if update.is_completed():
        break
```

## Type Aliases

- `TaskID = str`: Task identifier
- `SessionID = str`: Session identifier
- `ApplicationID = str`: Application identifier
- `Message = bytes`: Message data
- `TaskInput = Message`: Task input data
- `TaskOutput = Message`: Task output data
- `CommonData = Message`: Common data

## Constants

- `DEFAULT_FLAME_CONF = "flame-conf.yaml"`: Default configuration file
- `DEFAULT_CONTEXT_NAME = "flame"`: Default context name
- `DEFAULT_FLAME_ENDPOINT = "http://127.0.0.1:8080"`: Default endpoint 