"""In-process frontend gRPC fixture shared by sync and aio core tests."""

# Generated gRPC method names are capitalized.
# ruff: noqa: N802

import asyncio
import threading

import grpc
import pytest

from flamepy.core._bridge import LoopThread
from flamepy.core.types import SessionState, TaskState
from flamepy.proto import types_pb2 as pb
from flamepy.proto.frontend_pb2_grpc import FrontendServicer, add_FrontendServicer_to_server

TLS_TEST_CONFIG = None


class FrontendFixture(FrontendServicer):
    APPLICATION = "app"
    SESSION = "sess-1"

    def __init__(self):
        self.watches = 0
        self.watch_requests = []
        self.release = asyncio.Event()
        self.requests = []
        self.reject_close = False
        self.reject_create_task = False
        self.create_task_gate = None
        self.create_task_started = threading.Event()

    def _session(self, session=SESSION, name=None):
        name = name or session
        return pb.Session(
            metadata=pb.Metadata(id="00000000-0000-4000-8000-000000000001", name=name, workspace="default"),
            spec=pb.SessionSpec(application=self.APPLICATION, common_data=b""),
            status=pb.SessionStatus(state=SessionState.OPEN, creation_time=1, events=[pb.Event(code=1001, message="test", creation_time=1)]),
        )

    def _task(self, task=1, state=TaskState.SUCCEED, session=SESSION):
        task_name = str(task)
        return pb.Task(
            metadata=pb.Metadata(id="00000000-0000-4000-8000-000000000002", name=task_name, workspace="default"),
            spec=pb.TaskSpec(session=session, workspace="default", input=b"", output=b"done"),
            status=pb.TaskStatus(state=state, creation_time=1),
        )

    async def RegisterApplication(self, request, context):
        self.requests.append(request)
        return await self.GetApplication(type("Request", (), {"application": self.APPLICATION, "workspace": "default"})(), context)

    async def UnregisterApplication(self, request, context):
        return pb.Result()

    async def ListApplications(self, request, context):
        return pb.ApplicationList(applications=[await self.GetApplication(type("Request", (), {"application": self.APPLICATION, "workspace": "default"})(), context)])

    async def GetApplication(self, request, context):
        if request.application == "missing" and request.workspace == "default":
            await context.abort(grpc.StatusCode.NOT_FOUND, "missing")
        app = pb.Application(
            metadata=pb.Metadata(id="00000000-0000-4000-8000-000000000003", name="app", workspace="default"),
            status=pb.ApplicationStatus(state=0, creation_time=1),
        )
        app.spec.image = ""
        app.spec.schema.CopyFrom(pb.ApplicationSchema(input=""))
        return app

    async def ListExecutors(self, request, context):
        self.last_executor_application = request.application if request.HasField("application") else None
        return pb.ExecutorList()

    async def ListNodes(self, request, context):
        return pb.NodeList()

    async def CreateSession(self, request, context):
        self.requests.append(request)
        return self._session(name=request.name)

    async def OpenSession(self, request, context):
        return self._session(request.session)

    async def GetSession(self, request, context):
        return self._session(request.session)

    async def ListSessions(self, request, context):
        return pb.SessionList(sessions=[self._session()])

    async def CloseSession(self, request, context):
        if self.reject_close:
            await context.abort(grpc.StatusCode.FAILED_PRECONDITION, "close rejected")
        return self._session(request.session)

    async def CreateTask(self, request, context):
        if self.create_task_gate is not None:
            self.create_task_started.set()
            await self.create_task_gate.wait()
        if self.reject_create_task:
            await context.abort(grpc.StatusCode.INVALID_ARGUMENT, "task rejected")
        self.requests.append(request)
        task = 2 if request.task.input == b"hold" else 1
        return self._task(task, TaskState.PENDING, request.task.session)

    async def GetTask(self, request, context):
        return self._task(request.task, session=request.session)

    async def ListTasks(self, request, context):
        yield self._task(session=request.session)

    async def WatchTasks(self, requests, context):
        self.watches += 1
        updates = asyncio.Queue()
        release_tasks = []

        async def after_release(task, session):
            await self.release.wait()
            await updates.put(self._task(task, session=session))

        async def receive():
            async for request in requests:
                self.watch_requests.append(request.task)
                if request.task == 2:
                    await updates.put(self._task(2, TaskState.PENDING, request.session))
                    release_tasks.append(asyncio.create_task(after_release(2, request.session)))
                elif request.task == 9:
                    await updates.put(self._task(9, TaskState.PENDING, request.session))
                    await updates.put(None)
                else:
                    await updates.put(self._task(request.task, session=request.session))

        reader = asyncio.create_task(receive())
        try:
            while True:
                update = await updates.get()
                if update is None:
                    await context.abort(grpc.StatusCode.INTERNAL, "watch failed")
                yield update
        finally:
            reader.cancel()
            for task in release_tasks:
                task.cancel()
            await asyncio.gather(reader, *release_tasks, return_exceptions=True)


@pytest.fixture
def frontend_server():
    bridge = LoopThread("test-frontend-server")
    fixture = FrontendFixture()

    async def start():
        server = grpc.aio.server()
        add_FrontendServicer_to_server(fixture, server)
        port = server.add_insecure_port("127.0.0.1:0")
        await server.start()
        return server, port

    server, port = bridge.call(start())
    try:
        yield f"http://127.0.0.1:{port}", fixture, bridge
    finally:
        bridge.call(server.stop(0))
        bridge.close()
