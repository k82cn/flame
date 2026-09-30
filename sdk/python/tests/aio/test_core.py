"""AsyncIO frontend client integration tests."""

import asyncio

import pytest

from flamepy.core import aio
from flamepy.core import client as sync_client
from flamepy.core._bridge import LoopThread
from flamepy.core.types import ApplicationAttributes, FlameError, SessionAttributes, TaskState


def test_secure_session_spec_includes_cache_token(monkeypatch):
    from flamepy.core.aio import cache as aio_cache
    from flamepy.core.aio.client import Connection

    signed = []

    async def sign(application):
        signed.append(application)
        return "signed-app-token"

    monkeypatch.setattr(aio_cache, "cached_app_token", sign)
    connection = object.__new__(Connection)
    connection.addr = "https://session-manager:8080"
    spec = asyncio.run(connection._session_spec_with_cache_token(SessionAttributes(application="app")))
    assert spec.tokens["flame_cache"] == "signed-app-token"
    assert signed == ["app"]

    explicit = SessionAttributes(application="app", tokens={"flame_cache": "supplied"})
    spec = asyncio.run(connection._session_spec_with_cache_token(explicit))
    assert spec.tokens["flame_cache"] == "supplied"
    assert signed == ["app"]


def test_aio_frontend_api_parity(frontend_server):
    endpoint, service, server_loop = frontend_server

    async def exercise():
        async with await aio.connect(endpoint) as connection:
            await connection.register_application("app", ApplicationAttributes())
            await connection.unregister_application("app")
            assert len(await connection.list_applications()) == 1
            assert (await connection.get_application("app")).name == "app"
            assert await connection.get_application("missing") is None
            assert await connection.list_executors() == []
            assert await connection.list_nodes() == []
            session = await connection.create_session(SessionAttributes(application="app"))
            assert session.common_data() == b""
            assert (await connection.open_session(session.id)).id == session.id
            assert (await connection.get_session(session.id)).id == session.id
            assert len(await connection.list_sessions()) == 1
            assert (await session.create_task(b"input")).id == "task-1"
            assert (await session.get_task("task-1")).output == b"done"
            assert [task.id async for task in session.list_tasks()] == ["task-1"]
            updates = session.watch_task("task-1")
            assert (await updates.__anext__()).state == TaskState.SUCCEED
            updates.close()
            assert await session.run(b"input") == b"done"
            submitted = session.submit(b"input")
            assert isinstance(submitted, asyncio.Task)
            assert await submitted == b"done"
            assert await session.run(b"input") == b"done"
            await session.close()

    asyncio.run(exercise())
    assert service.watches == 1


def test_aio_watchers_share_session_stream_and_close_reports_error(frontend_server):
    endpoint, service, _ = frontend_server

    async def exercise():
        async with await aio.connect(endpoint) as connection:
            session = await connection.create_session(SessionAttributes(application="app"))
            first = session.watch_task("task-1")
            second = session.watch_task("task-1")
            assert (await first.__anext__()).state == TaskState.SUCCEED
            assert (await second.__anext__()).state == TaskState.SUCCEED
            with pytest.raises(StopAsyncIteration):
                await first.__anext__()

            held = session.watch_task("hold")
            assert (await held.__anext__()).state == TaskState.PENDING
            second_held = session.watch_task("hold")
            assert (await second_held.__anext__()).state == TaskState.PENDING
            assert service.watch_requests.count("hold") == 2
            await session.close()
            with pytest.raises(FlameError, match="session closed"):
                await held.__anext__()
            with pytest.raises(FlameError, match="session closed"):
                await second_held.__anext__()

    asyncio.run(exercise())
    assert service.watches == 1


def test_aio_watch_failure_and_cancellation(frontend_server):
    endpoint, service, server_loop = frontend_server

    async def exercise():
        connection = await aio.connect(endpoint)
        try:
            session = await connection.create_session(SessionAttributes(application="app"))
            watcher = session.watch_task("error")
            assert (await watcher.__anext__()).state == TaskState.PENDING
            with pytest.raises(FlameError, match="watch failed"):
                await watcher.__anext__()
            result = session.submit(b"hold")
            result.cancel()
            with pytest.raises(asyncio.CancelledError):
                await result
            await asyncio.sleep(0)
        finally:
            await connection.close()

    asyncio.run(exercise())


def test_aio_watch_timeout_and_connection_close_error(frontend_server):
    endpoint, _, _ = frontend_server

    async def exercise():
        connection = await aio.connect(endpoint)
        session = await connection.create_session(SessionAttributes(application="app"))
        watcher = session.watch_task("hold", timeout=0.01)
        assert (await watcher.__anext__()).state == TaskState.PENDING
        with pytest.raises(TimeoutError, match="watch_task timed out"):
            await watcher.__anext__()

        result = session.submit(b"hold")
        await connection.close()
        with pytest.raises(FlameError, match="connection closed"):
            await result

    asyncio.run(exercise())


def test_aio_submit_then_close_settles_submitted_task(frontend_server):
    endpoint, _, _ = frontend_server

    async def exercise():
        connection = await aio.connect(endpoint)
        session = await connection.create_session(SessionAttributes(application="app"))
        submitted = session.submit(b"input")
        await connection.close()
        assert submitted.done()
        with pytest.raises(FlameError, match="connection closed"):
            await submitted

    asyncio.run(exercise())


def test_aio_submit_reports_create_task_error_on_task(frontend_server):
    endpoint, service, _ = frontend_server

    async def exercise():
        async with await aio.connect(endpoint) as connection:
            session = await connection.create_session(SessionAttributes(application="app"))
            service.reject_create_task = True
            submitted = session.submit(b"input")
            with pytest.raises(FlameError, match="task rejected"):
                await submitted

    asyncio.run(exercise())


def test_aio_submit_close_during_create_task_settles_submitted_task(frontend_server):
    endpoint, service, server_loop = frontend_server
    service.create_task_gate = asyncio.Event()

    async def exercise():
        connection = await aio.connect(endpoint)
        session = await connection.create_session(SessionAttributes(application="app"))
        submitted = session.submit(b"input")
        assert await asyncio.to_thread(service.create_task_started.wait, 3)
        await connection.close()
        assert submitted.done()
        with pytest.raises(FlameError, match="connection closed"):
            await submitted

    try:
        asyncio.run(exercise())
    finally:
        server_loop.loop.call_soon_threadsafe(service.create_task_gate.set)


def test_aio_connection_rejects_other_loop(frontend_server):
    endpoint, _, _ = frontend_server
    owner = LoopThread("test-aio-owner")

    async def make_connection():
        return await aio.connect(endpoint)

    connection = owner.call(make_connection())

    async def misuse():
        with pytest.raises(RuntimeError, match="another event loop"):
            await connection.list_nodes()

    try:
        asyncio.run(misuse())
    finally:
        owner.call(connection.close())
        owner.close()


def test_asyncio_wrap_future_on_sync_core_result(frontend_server):
    endpoint, _, _ = frontend_server
    connection = sync_client.connect(endpoint)
    try:
        session = connection.create_session(SessionAttributes(application="app"))

        async def wait_for_result():
            future = session.submit(b"input")
            return await asyncio.wrap_future(future)

        assert asyncio.run(wait_for_result()) == b"done"
    finally:
        connection.close()
