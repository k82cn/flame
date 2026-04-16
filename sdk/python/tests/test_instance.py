"""Tests for flamepy.agent.instance module."""

import types

import cloudpickle
import pytest

from flamepy.agent.instance import FlameInstance
from flamepy.core.service import ApplicationContext, SessionContext, TaskContext
from flamepy.core.types import TaskOutput


class DummyObjectRef:
    """Dummy ObjectRef for testing."""

    def __init__(self, data=b"test"):
        self._data = data

    @classmethod
    def decode(cls, data: bytes) -> "DummyObjectRef":
        return cls(data)

    def encode(self) -> bytes:
        return self._data


@pytest.fixture
def flame_instance():
    """Create a fresh FlameInstance for testing."""
    return FlameInstance()


def test_flameinstance_init(flame_instance):
    """Test FlameInstance initializes with correct defaults."""
    assert flame_instance._entrypoint is None
    assert flame_instance._parameter is None
    assert flame_instance._object_ref is None


def test_entrypoint_decorator_registers_function(flame_instance):
    """Test that entrypoint decorator registers the function."""

    @flame_instance.entrypoint
    def my_handler(data):
        return data

    assert flame_instance._entrypoint is my_handler
    assert flame_instance._parameter is not None
    assert flame_instance._parameter.name == "data"


def test_entrypoint_decorator_zero_params(flame_instance):
    """Test entrypoint decorator with zero-parameter function."""

    @flame_instance.entrypoint
    def no_params():
        return "done"

    assert flame_instance._entrypoint is no_params
    assert flame_instance._parameter is None


def test_entrypoint_decorator_rejects_multiple_params():
    """Test entrypoint decorator rejects functions with multiple params."""
    fi = FlameInstance()

    with pytest.raises(AssertionError):

        @fi.entrypoint
        def bad_handler(a, b, c):
            pass


def test_on_session_enter_decodes_object_ref(flame_instance, monkeypatch):
    """Test on_session_enter decodes ObjectRef from common_data."""
    dummy_ref = DummyObjectRef(b"session-data")

    monkeypatch.setattr(
        "flamepy.agent.instance.ObjectRef",
        types.SimpleNamespace(decode=lambda data: dummy_ref),
    )

    app_ctx = ApplicationContext(name="test-app")
    session_ctx = SessionContext(
        _common_data=b"encoded-ref",
        session_id="sess-1",
        application=app_ctx,
    )

    flame_instance.on_session_enter(session_ctx)
    assert flame_instance._object_ref is dummy_ref


def test_on_session_enter_handles_none_common_data(flame_instance):
    """Test on_session_enter handles None common_data."""
    app_ctx = ApplicationContext(name="test-app")
    session_ctx = SessionContext(
        _common_data=None,
        session_id="sess-1",
        application=app_ctx,
    )

    flame_instance.on_session_enter(session_ctx)
    assert flame_instance._object_ref is None


def test_on_task_invoke_calls_entrypoint(flame_instance, monkeypatch):
    """Test on_task_invoke calls registered entrypoint with deserialized input."""
    received_input = []

    @flame_instance.entrypoint
    def handler(data):
        received_input.append(data)
        return {"result": "ok"}

    monkeypatch.setattr(
        "flamepy.agent.instance.cloudpickle",
        types.SimpleNamespace(
            loads=lambda x: {"key": "value"},
            dumps=cloudpickle.dumps,
            DEFAULT_PROTOCOL=cloudpickle.DEFAULT_PROTOCOL,
        ),
    )

    task_ctx = TaskContext(
        task_id="task-1",
        session_id="sess-1",
        input=b"serialized-input",
    )

    result = flame_instance.on_task_invoke(task_ctx)

    assert len(received_input) == 1
    assert received_input[0] == {"key": "value"}
    assert isinstance(result, TaskOutput)


def test_on_task_invoke_with_none_input(flame_instance, monkeypatch):
    """Test on_task_invoke with None input."""
    received_input = []

    @flame_instance.entrypoint
    def handler(data):
        received_input.append(data)
        return None

    task_ctx = TaskContext(
        task_id="task-1",
        session_id="sess-1",
        input=None,
    )

    flame_instance.on_task_invoke(task_ctx)

    assert len(received_input) == 1
    assert received_input[0] is None


def test_on_task_invoke_without_entrypoint(flame_instance):
    """Test on_task_invoke returns None when no entrypoint is registered."""
    task_ctx = TaskContext(
        task_id="task-1",
        session_id="sess-1",
        input=b"data",
    )

    result = flame_instance.on_task_invoke(task_ctx)
    assert result is None


def test_on_task_invoke_with_zero_param_entrypoint(flame_instance, monkeypatch):
    """Test on_task_invoke with zero-parameter entrypoint."""

    @flame_instance.entrypoint
    def no_params():
        return "done"

    monkeypatch.setattr(
        "flamepy.agent.instance.cloudpickle",
        types.SimpleNamespace(
            loads=lambda x: "ignored",
            dumps=cloudpickle.dumps,
            DEFAULT_PROTOCOL=cloudpickle.DEFAULT_PROTOCOL,
        ),
    )

    task_ctx = TaskContext(
        task_id="task-1",
        session_id="sess-1",
        input=b"ignored",
    )

    result = flame_instance.on_task_invoke(task_ctx)
    assert isinstance(result, TaskOutput)


def test_on_session_leave_clears_object_ref(flame_instance):
    """Test on_session_leave clears the object reference."""
    flame_instance._object_ref = DummyObjectRef()

    flame_instance.on_session_leave()

    assert flame_instance._object_ref is None


def test_context_returns_deserialized_data(flame_instance, monkeypatch):
    """Test context() returns deserialized data from cache."""
    flame_instance._object_ref = DummyObjectRef()

    monkeypatch.setattr(
        "flamepy.agent.instance.get_object",
        lambda ref: b"serialized-ctx",
    )
    monkeypatch.setattr(
        "flamepy.agent.instance.cloudpickle",
        types.SimpleNamespace(loads=lambda x: {"ctx_key": "ctx_value"}),
    )

    result = flame_instance.context()
    assert result == {"ctx_key": "ctx_value"}


def test_context_returns_none_when_no_ref(flame_instance):
    """Test context() returns None when no object_ref."""
    flame_instance._object_ref = None

    result = flame_instance.context()
    assert result is None


def test_update_context_serializes_and_updates(flame_instance, monkeypatch):
    """Test update_context() serializes data and updates cache."""
    flame_instance._object_ref = DummyObjectRef()
    updated_refs = []

    def mock_update(ref, data):
        updated_refs.append((ref, data))
        return DummyObjectRef(data)

    monkeypatch.setattr("flamepy.agent.instance.update_object", mock_update)
    monkeypatch.setattr(
        "flamepy.agent.instance.cloudpickle",
        types.SimpleNamespace(
            dumps=lambda x, protocol=None: b"serialized:" + str(x).encode(),
            DEFAULT_PROTOCOL=4,
        ),
    )

    flame_instance.update_context({"new": "data"})

    assert len(updated_refs) == 1
    assert updated_refs[0][1] == b"serialized:{'new': 'data'}"


def test_update_context_noop_when_no_ref(flame_instance, monkeypatch):
    """Test update_context() does nothing when no object_ref."""
    flame_instance._object_ref = None
    called = []

    monkeypatch.setattr(
        "flamepy.agent.instance.update_object",
        lambda ref, data: called.append(True),
    )

    flame_instance.update_context({"data": 1})

    assert len(called) == 0
