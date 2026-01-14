"""
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
"""

import threading
from typing import Optional, List, Dict, Any, Union
from urllib.parse import urlparse
from concurrent.futures import Future, ThreadPoolExecutor
import grpc
import pickle
from datetime import datetime, timezone

from .cache import put_object, get_object
from .types import (
    Task,
    Application,
    SessionAttributes,
    ApplicationAttributes,
    Event,
    SessionID,
    TaskID,
    ApplicationID,
    TaskInput,
    TaskOutput,
    CommonData,
    SessionState,
    TaskState,
    ApplicationState,
    Shim,
    FlameError,
    FlameErrorCode,
    TaskInformer,
    FlameContext,
    ApplicationSchema,
    short_name,
    ObjectRef,
)

from .types_pb2 import ApplicationSpec, SessionSpec, TaskSpec, Environment
from .frontend_pb2 import (
    RegisterApplicationRequest,
    UnregisterApplicationRequest,
    ListApplicationRequest,
    CreateSessionRequest,
    ListSessionRequest,
    GetSessionRequest,
    CloseSessionRequest,
    CreateTaskRequest,
    WatchTaskRequest,
    GetTaskRequest,
    GetApplicationRequest,
    OpenSessionRequest,
)
from .frontend_pb2_grpc import FrontendStub


def connect(addr: str) -> "Connection":
    """Connect to the Flame service."""
    return Connection.connect(addr)


def create_session(application: str, common_data: Any = None, session_id: Optional[str] = None, slots: int = 1) -> "Session":
    conn = ConnectionInstance.instance()
    return conn.create_session(SessionAttributes(id=session_id, application=application, common_data=common_data, slots=slots))


def open_session(session_id: SessionID) -> "Session":
    conn = ConnectionInstance.instance()
    return conn.open_session(session_id)


def register_application(name: str, app_attrs: Union[ApplicationAttributes, Dict[str, Any]]) -> None:
    conn = ConnectionInstance.instance()
    conn.register_application(name, app_attrs)


def unregister_application(name: str) -> None:
    conn = ConnectionInstance.instance()
    conn.unregister_application(name)


def list_applications() -> List[Application]:
    conn = ConnectionInstance.instance()
    return conn.list_applications()


def get_application(name: str) -> Application:
    conn = ConnectionInstance.instance()
    return conn.get_application(name)


def list_sessions() -> List["Session"]:
    conn = ConnectionInstance.instance()
    return conn.list_sessions()


def get_session(session_id: SessionID) -> "Session":
    conn = ConnectionInstance.instance()
    return conn.get_session(session_id)


def close_session(session_id: SessionID) -> "Session":
    conn = ConnectionInstance.instance()
    return conn.close_session(session_id)


class ConnectionInstance:
    """Connection instance."""

    _lock = threading.Lock()
    _connection = None
    _context = None

    @classmethod
    def instance(cls) -> "Connection":
        """Get the connection instance."""
        with cls._lock:
            if cls._connection is None:
                cls._context = FlameContext()
                cls._connection = connect(cls._context._endpoint)
            return cls._connection


class Connection:
    """Connection to the Flame service."""

    def __init__(self, addr: str, channel: grpc.Channel, frontend: FrontendStub):
        self.addr = addr
        self._channel = channel
        self._frontend = frontend
        self._executor = ThreadPoolExecutor(max_workers=10)

    @classmethod
    def connect(cls, addr: str) -> "Connection":
        """Establish a connection to the Flame service."""
        if not addr:
            raise FlameError(FlameErrorCode.INVALID_CONFIG, "address cannot be empty")

        try:
            parsed_addr = urlparse(addr)
            host = parsed_addr.hostname or parsed_addr.path
            port = parsed_addr.port or 8080

            # Create insecure channel
            channel = grpc.insecure_channel(f"{host}:{port}")

            # Wait for channel to be ready (with timeout)
            try:
                grpc.channel_ready_future(channel).result(timeout=10)
            except grpc.FutureTimeoutError:
                raise FlameError(FlameErrorCode.INVALID_CONFIG, f"timeout connecting to {addr}")

            # Create frontend stub
            frontend = FrontendStub(channel)

            return cls(addr, channel, frontend)

        except Exception as e:
            raise FlameError(FlameErrorCode.INVALID_CONFIG, f"failed to connect to {addr}: {str(e)}")

    def close(self) -> None:
        """Close the connection."""
        self._executor.shutdown(wait=True)
        self._channel.close()

    def register_application(self, name: str, app_attrs: Union[ApplicationAttributes, Dict[str, Any]]) -> None:
        """Register a new application."""
        if isinstance(app_attrs, dict):
            app_attrs = ApplicationAttributes(**app_attrs)

        schema = None
        if app_attrs.schema is not None:
            schema = ApplicationSchema(
                input=app_attrs.schema.input,
                output=app_attrs.schema.output,
                common_data=app_attrs.schema.common_data,
            )

        environments = []
        if app_attrs.environments is not None:
            for k, v in app_attrs.environments.items():
                environments.append(Environment(name=k, value=v))

        app_spec = ApplicationSpec(
            shim=app_attrs.shim,
            image=app_attrs.image,
            command=app_attrs.command,
            description=app_attrs.description,
            labels=app_attrs.labels or [],
            arguments=app_attrs.arguments or [],
            environments=environments,
            working_directory=app_attrs.working_directory,
            max_instances=app_attrs.max_instances,
            delay_release=app_attrs.delay_release,
            schema=schema,
            url=app_attrs.url,
        )

        request = RegisterApplicationRequest(name=name, application=app_spec)

        try:
            self._frontend.RegisterApplication(request)
        except grpc.RpcError as e:
            raise FlameError(
                FlameErrorCode.INTERNAL,
                f"failed to register application: {e.details()}",
            )

    def unregister_application(self, name: str) -> None:
        """Unregister an application."""
        request = UnregisterApplicationRequest(name=name)

        try:
            self._frontend.UnregisterApplication(request)
        except grpc.RpcError as e:
            raise FlameError(
                FlameErrorCode.INTERNAL,
                f"failed to unregister application: {e.details()}",
            )

    def list_applications(self) -> List[Application]:
        """List all applications."""
        request = ListApplicationRequest()

        try:
            response = self._frontend.ListApplication(request)

            applications = []
            for app in response.applications:
                schema = None
                if app.spec.schema is not None:
                    schema = ApplicationSchema(
                        input=app.spec.schema.input,
                        output=app.spec.schema.output,
                        common_data=app.spec.schema.common_data,
                    )
                environments = {}
                if app.spec.environments is not None:
                    for env in app.spec.environments:
                        environments[env.name] = env.value

                applications.append(
                    Application(
                        id=app.metadata.id,
                        name=app.metadata.name,
                        shim=Shim(app.spec.shim),
                        state=ApplicationState(app.status.state),
                        creation_time=datetime.fromtimestamp(app.status.creation_time / 1000, tz=timezone.utc),
                        image=app.spec.image,
                        command=app.spec.command,
                        arguments=list(app.spec.arguments),
                        environments=environments,
                        working_directory=app.spec.working_directory,
                        max_instances=app.spec.max_instances,
                        delay_release=app.spec.delay_release,
                        schema=schema,
                        url=app.spec.url if app.spec.HasField("url") else None,
                    )
                )

            return applications

        except grpc.RpcError as e:
            raise FlameError(FlameErrorCode.INTERNAL, f"failed to list applications: {e.details()}")

    def get_application(self, name: str) -> Application:
        """Get an application by name."""
        request = GetApplicationRequest(name=name)

        try:
            response = self._frontend.GetApplication(request)
            schema = None
            if response.spec.schema is not None:
                schema = ApplicationSchema(
                    input=response.spec.schema.input,
                    output=response.spec.schema.output,
                    common_data=response.spec.schema.common_data,
                )

            environments = {}
            if response.spec.environments is not None:
                for env in response.spec.environments:
                    environments[env.name] = env.value

            return Application(
                id=response.metadata.id,
                name=response.metadata.name,
                shim=Shim(response.spec.shim),
                state=ApplicationState(response.status.state),
                creation_time=datetime.fromtimestamp(response.status.creation_time / 1000, tz=timezone.utc),
                image=response.spec.image,
                command=response.spec.command,
                arguments=list(response.spec.arguments),
                environments=environments,
                working_directory=response.spec.working_directory,
                max_instances=response.spec.max_instances,
                delay_release=response.spec.delay_release,
                schema=schema,
                url=response.spec.url if response.spec.HasField("url") else None,
            )

        except grpc.RpcError as e:
            raise FlameError(FlameErrorCode.INTERNAL, f"failed to get application: {e.details()}")

    def create_session(self, attrs: SessionAttributes) -> "Session":
        """Create a new session."""

        session_id = short_name(attrs.application) if attrs.id is None else attrs.id

        common_data_bin = pickle.dumps(attrs.common_data, protocol=pickle.HIGHEST_PROTOCOL)

        data_expr = put_object(session_id, common_data_bin)

        session_spec = SessionSpec(
            application=attrs.application,
            slots=attrs.slots,
            common_data=data_expr.encode(),
        )

        request = CreateSessionRequest(session_id=session_id, session=session_spec)

        try:
            response = self._frontend.CreateSession(request)
            common_data_expr = ObjectRef.decode(response.spec.common_data) if response.spec.HasField("common_data") else None

            session = Session(
                connection=self,
                id=response.metadata.id,
                application=response.spec.application,
                slots=response.spec.slots,
                state=SessionState(response.status.state),
                creation_time=datetime.fromtimestamp(response.status.creation_time / 1000, tz=timezone.utc),
                pending=response.status.pending,
                running=response.status.running,
                succeed=response.status.succeed,
                failed=response.status.failed,
                completion_time=(datetime.fromtimestamp(response.status.completion_time / 1000, tz=timezone.utc) if response.status.HasField("completion_time") else None),
                common_data=common_data_expr,
            )
            return session
        except grpc.RpcError as e:
            raise FlameError(FlameErrorCode.INTERNAL, f"failed to create session: {e.details()}")

    def list_sessions(self) -> List["Session"]:
        """List all sessions."""
        request = ListSessionRequest()

        try:
            response = self._frontend.ListSession(request)

            sessions = []
            for session in response.sessions:
                common_data_expr = ObjectRef.decode(session.spec.common_data) if session.spec.HasField("common_data") else None

                sessions.append(
                    Session(
                        connection=self,
                        id=session.metadata.id,
                        application=session.spec.application,
                        slots=session.spec.slots,
                        state=SessionState(session.status.state),
                        creation_time=datetime.fromtimestamp(session.status.creation_time / 1000, tz=timezone.utc),
                        pending=session.status.pending,
                        running=session.status.running,
                        succeed=session.status.succeed,
                        failed=session.status.failed,
                        completion_time=(datetime.fromtimestamp(session.status.completion_time / 1000, tz=timezone.utc) if session.status.HasField("completion_time") else None),
                        common_data=common_data_expr,
                    )
                )

            return sessions

        except grpc.RpcError as e:
            raise FlameError(FlameErrorCode.INTERNAL, f"failed to list sessions: {e.details()}")

    def open_session(self, session_id: SessionID) -> "Session":
        """Open a session."""
        request = OpenSessionRequest(session_id=session_id)

        try:
            response = self._frontend.OpenSession(request)
            common_data_expr = ObjectRef.decode(response.spec.common_data) if response.spec.HasField("common_data") else None

            return Session(
                connection=self,
                id=response.metadata.id,
                application=response.spec.application,
                slots=response.spec.slots,
                state=SessionState(response.status.state),
                creation_time=datetime.fromtimestamp(response.status.creation_time / 1000, tz=timezone.utc),
                pending=response.status.pending,
                running=response.status.running,
                succeed=response.status.succeed,
                failed=response.status.failed,
                completion_time=(datetime.fromtimestamp(response.status.completion_time / 1000, tz=timezone.utc) if response.status.HasField("completion_time") else None),
                common_data=common_data_expr,
            )

        except grpc.RpcError as e:
            raise FlameError(FlameErrorCode.INTERNAL, f"failed to open session: {e.details()}")

    def get_session(self, session_id: SessionID) -> "Session":
        """Get a session by ID."""
        request = GetSessionRequest(session_id=session_id)

        try:
            response = self._frontend.GetSession(request)

            common_data_expr = ObjectRef.decode(response.spec.common_data) if response.spec.HasField("common_data") else None

            return Session(
                connection=self,
                id=response.metadata.id,
                application=response.spec.application,
                slots=response.spec.slots,
                state=SessionState(response.status.state),
                creation_time=datetime.fromtimestamp(response.status.creation_time / 1000, tz=timezone.utc),
                pending=response.status.pending,
                running=response.status.running,
                succeed=response.status.succeed,
                failed=response.status.failed,
                completion_time=(datetime.fromtimestamp(response.status.completion_time / 1000, tz=timezone.utc) if response.status.HasField("completion_time") else None),
                common_data=common_data_expr,
            )

        except grpc.RpcError as e:
            raise FlameError(FlameErrorCode.INTERNAL, f"failed to get session: {e.details()}")

    def close_session(self, session_id: SessionID) -> "Session":
        """Close a session."""
        request = CloseSessionRequest(session_id=session_id)

        try:
            response = self._frontend.CloseSession(request)

            common_data_expr = ObjectRef.decode(response.spec.common_data) if response.spec.HasField("common_data") else None

            return Session(
                connection=self,
                id=response.metadata.id,
                application=response.spec.application,
                slots=response.spec.slots,
                state=SessionState(response.status.state),
                creation_time=datetime.fromtimestamp(response.status.creation_time / 1000, tz=timezone.utc),
                pending=response.status.pending,
                running=response.status.running,
                succeed=response.status.succeed,
                failed=response.status.failed,
                completion_time=(datetime.fromtimestamp(response.status.completion_time / 1000, tz=timezone.utc) if response.status.HasField("completion_time") else None),
                common_data=common_data_expr,
            )

        except grpc.RpcError as e:
            raise FlameError(FlameErrorCode.INTERNAL, f"failed to close session: {e.details()}")


class Session:
    connection: Connection
    """Represents a computing session."""
    id: SessionID
    application: str
    slots: int
    state: SessionState
    creation_time: datetime
    pending: int = 0
    running: int = 0
    succeed: int = 0
    failed: int = 0
    completion_time: Optional[datetime] = None
    _common_data: Optional[ObjectRef] = None
    """Client for session-specific operations."""

    def __init__(
        self,
        connection: Connection,
        id: SessionID,
        application: str,
        slots: int,
        state: SessionState,
        creation_time: datetime,
        pending: int,
        running: int,
        succeed: int,
        failed: int,
        completion_time: Optional[datetime],
        common_data: Optional[ObjectRef] = None,
    ):
        self.connection = connection
        self.id = id
        self.application = application
        self.slots = slots
        self.state = state
        self.creation_time = creation_time
        self.pending = pending
        self.running = running
        self.succeed = succeed
        self.failed = failed
        self.completion_time = completion_time
        self.mutex = threading.Lock()
        self._common_data = common_data

    def common_data(self) -> Any:
        """Get the common data of Session."""
        self._common_data = get_object(self._common_data)

        return pickle.loads(self._common_data.data) if self._common_data is not None else None

    def create_task(self, input_data: Any) -> Task:
        """Create a new task in the session."""
        input_bin = pickle.dumps(input_data, protocol=pickle.HIGHEST_PROTOCOL)

        task_spec = TaskSpec(session_id=self.id, input=input_bin)

        request = CreateTaskRequest(task=task_spec)

        try:
            response = self.connection._frontend.CreateTask(request)

            return Task(
                id=response.metadata.id,
                session_id=self.id,
                state=TaskState(response.status.state),
                creation_time=datetime.fromtimestamp(response.status.creation_time / 1000, tz=timezone.utc),
                input=input_data,
                completion_time=(datetime.fromtimestamp(response.status.completion_time / 1000, tz=timezone.utc) if response.status.HasField("completion_time") else None),
                events=[
                    Event(
                        code=event.code,
                        message=event.message,
                        creation_time=datetime.fromtimestamp(event.creation_time / 1000, tz=timezone.utc),
                    )
                    for event in response.status.events
                ],
            )

        except grpc.RpcError as e:
            raise FlameError(FlameErrorCode.INTERNAL, f"failed to create task: {e.details()}")

    def get_task(self, task_id: TaskID) -> Task:
        """Get a task by ID."""
        request = GetTaskRequest(task_id=task_id, session_id=self.id)

        try:
            response = self.connection._frontend.GetTask(request)

            return Task(
                id=response.metadata.id,
                session_id=self.id,
                state=TaskState(response.status.state),
                creation_time=datetime.fromtimestamp(response.status.creation_time / 1000, tz=timezone.utc),
                input=pickle.loads(response.spec.input) if response.spec.input is not None else None,
                output=pickle.loads(response.spec.output) if response.spec.output is not None else None,
                completion_time=(datetime.fromtimestamp(response.status.completion_time / 1000, tz=timezone.utc) if response.status.HasField("completion_time") else None),
                events=[
                    Event(
                        code=event.code,
                        message=event.message,
                        creation_time=datetime.fromtimestamp(event.creation_time / 1000, tz=timezone.utc),
                    )
                    for event in response.status.events
                ],
            )

        except grpc.RpcError as e:
            raise FlameError(FlameErrorCode.INTERNAL, f"failed to get task: {e.details()}")

    def watch_task(self, task_id: TaskID) -> "TaskWatcher":
        """Watch a task for updates."""
        request = WatchTaskRequest(task_id=task_id, session_id=self.id)

        try:
            stream = self.connection._frontend.WatchTask(request)
            return TaskWatcher(stream)

        except grpc.RpcError as e:
            raise FlameError(FlameErrorCode.INTERNAL, f"failed to watch task: {e.details()}")

    def invoke(self, input_data: Any, informer: Optional[TaskInformer] = None) -> Any:
        """Invoke a task with the given input and optional informer (synchronous).
        
        This method blocks until the task completes or fails.
        
        Args:
            input_data: The input data for the task
            informer: Optional task informer for monitoring task progress
            
        Returns:
            The task output (or None if informer is provided)
            
        Example:
            >>> result = session.invoke(b"input data")
            >>> print(result)
        """
        return self._invoke_impl(input_data, informer)
    
    def run(self, input_data: Any, informer: Optional[TaskInformer] = None) -> Future:
        """Run a task asynchronously and return a Future (async-style execution).
        
        This method returns immediately with a Future object that can be used to
        retrieve the result later or run multiple tasks in parallel.
        
        Args:
            input_data: The input data for the task
            informer: Optional task informer for monitoring task progress
            
        Returns:
            A Future object that will contain the result when the task completes
            
        Example (single task):
            >>> future = session.run(b"input data")
            >>> result = future.result()  # Wait for completion
            
        Example (parallel execution):
            >>> from concurrent.futures import wait
            >>> futures = [session.run(f"input {i}".encode()) for i in range(10)]
            >>> wait(futures)
            >>> results = [f.result() for f in futures]
        """
        return self.connection._executor.submit(self._invoke_impl, input_data, informer)
    
    def _invoke_impl(self, input_data: Any, informer: Optional[TaskInformer] = None) -> Any:
        """Internal implementation of invoke/run."""
        task = self.create_task(input_data)
        watcher = self.watch_task(task.id)

        for task in watcher:
            # If informer is provided, use it to update the task and
            # return None to indicate that the task is handled by the informer.
            if informer is not None:
                with self.mutex:
                    informer.on_update(task)
                if task.is_completed():
                    return None

            # If the task is failed, raise an error.
            if task.is_failed():
                for event in task.events:
                    if event.code == TaskState.FAILED:
                        raise FlameError(FlameErrorCode.INTERNAL, f"{event.message}")
            # If the task is completed, return the output.
            elif task.is_completed():
                return task.output

    def close(self) -> None:
        """Close the session."""
        self.connection.close_session(self.id)


class TaskWatcher:
    """Iterator for watching task updates."""

    def __init__(self, stream):
        self._stream = stream

    def __iter__(self):
        return self

    def __next__(self) -> Task:
        try:
            response = next(self._stream)

            return Task(
                id=response.metadata.id,
                session_id=response.spec.session_id,
                state=TaskState(response.status.state),
                creation_time=datetime.fromtimestamp(response.status.creation_time / 1000, tz=timezone.utc),
                input=pickle.loads(response.spec.input) if response.spec.HasField("input") else None,
                output=pickle.loads(response.spec.output) if response.spec.HasField("output") else None,
                completion_time=(datetime.fromtimestamp(response.status.completion_time / 1000, tz=timezone.utc) if response.status.HasField("completion_time") else None),
                events=[
                    Event(
                        code=event.code,
                        message=event.message,
                        creation_time=datetime.fromtimestamp(event.creation_time / 1000, tz=timezone.utc),
                    )
                    for event in response.status.events
                ],
            )

        except StopIteration:
            raise
        except Exception as e:
            raise FlameError(FlameErrorCode.INTERNAL, f"failed to watch task: {str(e)}")
