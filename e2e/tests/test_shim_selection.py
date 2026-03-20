"""
Copyright 2026 The Flame Authors.
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

"""
E2E tests for shim selection feature (Issue #379).

These tests verify the shim selection logic that ensures applications are
matched with executors that support the required shim type (Host or Wasm).

Test scenarios:
1. Positive case: App with default shim (Host) matches executor with shim "Host"
2. Negative case: App with shim "Wasm" fails to match executor with shim "Host"
3. Default behavior: App without explicit shim defaults to "Host"

Note: The Python SDK does not expose the shim field in ApplicationAttributes.
The shim is configured at the backend level:
- Applications default to Host shim when not specified
- Executors get their shim from executor-manager configuration (flame-cluster.yaml)
- The scheduler filters executors by shim compatibility

To test the negative case (Wasm app on Host executor), we use gRPC directly
to register an application with shim=Wasm.
"""

import time
import pytest
import grpc
import flamepy
from e2e.api import TestRequest
from e2e.helpers import invoke_task
from flamepy.proto.frontend_pb2 import (
    RegisterApplicationRequest,
    UnregisterApplicationRequest,
    CreateSessionRequest,
    CloseSessionRequest,
    GetSessionRequest,
)
from flamepy.proto.frontend_pb2_grpc import FrontendStub
from flamepy.proto.types_pb2 import ApplicationSpec, SessionSpec, Shim
from flamepy.core.types import FlameContext


FLM_SHIM_TEST_APP = "flme2e-shim-test"
FLM_SHIM_TEST_APP_URL = "file:///opt/e2e"


@pytest.fixture(scope="module", autouse=True)
def setup_shim_test_app():
    """Setup test application for shim selection tests."""
    flamepy.register_application(
        FLM_SHIM_TEST_APP,
        flamepy.ApplicationAttributes(
            command="${FLAME_HOME}/bin/uv",
            working_directory="/opt/e2e",
            environments={"FLAME_LOG_LEVEL": "DEBUG"},
            arguments=["run", "src/e2e/basic_svc.py", "src/e2e/api.py"],
            url=FLM_SHIM_TEST_APP_URL,
        ),
    )

    yield

    sessions = flamepy.list_sessions()
    for sess in sessions:
        try:
            if sess.application == FLM_SHIM_TEST_APP:
                flamepy.close_session(sess.id)
        except:
            pass

    flamepy.unregister_application(FLM_SHIM_TEST_APP)


def get_grpc_stub():
    """Get a gRPC stub for direct API access."""
    ctx = FlameContext()
    endpoint = ctx.endpoint
    if endpoint.startswith("http://"):
        endpoint = endpoint[7:]
    elif endpoint.startswith("https://"):
        endpoint = endpoint[8:]

    if "/" in endpoint:
        endpoint = endpoint.split("/")[0]

    if ":" not in endpoint:
        endpoint = f"{endpoint}:8080"

    channel = grpc.insecure_channel(endpoint)
    return FrontendStub(channel), channel


class TestShimSelectionPositive:
    """Test positive case: App with Host shim matches Host executor."""

    def test_host_app_on_host_executor(self):
        """
        Test that an application with default shim (Host) successfully
        runs on an executor with Host shim.
        """
        session = flamepy.create_session(application=FLM_SHIM_TEST_APP, common_data=None)

        try:
            assert session is not None
            assert session.application == FLM_SHIM_TEST_APP
            assert session.state == flamepy.SessionState.OPEN

            request = TestRequest(input="shim_test_input")
            response = invoke_task(session, request)

            assert response.output == "shim_test_input"

        finally:
            session.close()


class TestShimSelectionNegative:
    """Test negative case: App with Wasm shim fails to match Host executor."""

    def test_wasm_app_on_host_executor_stays_pending(self):
        """
        Test that an application with Wasm shim cannot be scheduled
        on an executor with Host shim.

        The session should remain in pending state because no compatible
        executor is available (the e2e environment only has Host executors).

        This test uses gRPC directly to register an app with shim=Wasm
        since the Python SDK doesn't expose the shim field.
        """
        app_name = "flmtest-shim-wasm"
        stub, channel = get_grpc_stub()

        try:
            app_spec = ApplicationSpec(
                shim=Shim.Wasm,
                command="/bin/echo",
                arguments=["hello", "from", "wasm", "shim"],
                description="Test app for Wasm shim selection",
            )

            request = RegisterApplicationRequest(name=app_name, application=app_spec)
            stub.RegisterApplication(request)

            session_id = f"test-wasm-session-{int(time.time())}"
            session_spec = SessionSpec(
                application=app_name,
                slots=1,
            )
            create_request = CreateSessionRequest(session_id=session_id, session=session_spec)
            response = stub.CreateSession(create_request)

            assert response.metadata.id == session_id

            time.sleep(3)

            get_request = GetSessionRequest(session_id=session_id)
            session_status = stub.GetSession(get_request)

            assert session_status.status.state == 0  # Open state

            close_request = CloseSessionRequest(session_id=session_id)
            stub.CloseSession(close_request)

        finally:
            try:
                unregister_request = UnregisterApplicationRequest(name=app_name)
                stub.UnregisterApplication(unregister_request)
            except:
                pass
            channel.close()

    def test_wasm_app_task_stays_pending(self):
        """
        Test that a task created for a Wasm application stays pending
        when only Host executors are available.

        This verifies the scheduler correctly filters out incompatible executors.
        """
        app_name = "flmtest-shim-wasm-task"
        stub, channel = get_grpc_stub()

        try:
            app_spec = ApplicationSpec(
                shim=Shim.Wasm,
                command="/bin/echo",
                arguments=["hello"],
                description="Test app for Wasm shim task pending",
            )

            request = RegisterApplicationRequest(name=app_name, application=app_spec)
            stub.RegisterApplication(request)

            session = flamepy.create_session(application=app_name)

            input_data = b"test input for wasm"
            task = session.create_task(input_data)

            time.sleep(3)

            task_status = session.get_task(task.id)

            assert task_status.state == flamepy.TaskState.PENDING, f"Task should remain PENDING when no compatible executor available, got {task_status.state}"

            try:
                session.close()
            except:
                pass

        finally:
            try:
                unregister_request = UnregisterApplicationRequest(name=app_name)
                stub.UnregisterApplication(unregister_request)
            except:
                pass
            channel.close()


class TestShimSelectionDefault:
    """Test default behavior: App without shim defaults to Host."""

    def test_app_without_shim_defaults_to_host(self):
        """
        Test that an application registered without explicit shim
        defaults to Host and successfully matches Host executors.
        """
        session = flamepy.create_session(application=FLM_SHIM_TEST_APP, common_data=None)

        try:
            assert session is not None
            assert session.application == FLM_SHIM_TEST_APP

            request = TestRequest(input="default_shim_test")
            response = invoke_task(session, request)

            assert response.output == "default_shim_test"

        finally:
            session.close()

    def test_existing_apps_work_without_shim(self):
        """
        Test that pre-existing applications (flmexec, flmping, flmrun)
        continue to work after the shim selection feature is added.

        This is a regression test to ensure backward compatibility.
        """
        apps = flamepy.list_applications()

        app_names = [app.name for app in apps]
        standard_apps = ["flmexec", "flmping", "flmrun"]

        found_app = None
        for app_name in standard_apps:
            if app_name in app_names:
                found_app = app_name
                break

        assert found_app is not None, f"At least one standard app should exist: {standard_apps}"

        session = flamepy.create_session(application=found_app)

        try:
            assert session is not None
            assert session.application == found_app
            assert session.state == flamepy.SessionState.OPEN

        finally:
            session.close()


class TestShimSelectionIntegration:
    """Integration tests for shim selection with real workloads."""

    def test_host_shim_task_execution(self):
        """
        Test a complete workflow with Host shim.
        """
        session = flamepy.create_session(application=FLM_SHIM_TEST_APP, common_data=None)

        try:
            request = TestRequest(input="integration_test_input")
            response = invoke_task(session, request)

            assert response.output == "integration_test_input"
            assert response.service_state is not None
            assert response.service_state["task_count"] == 1

        finally:
            session.close()
