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

import pytest
import flamepy
from e2e.api import TestRequest, TestResponse, TestContext
from tests.utils import random_string

FLM_TEST_SVC_APP = "flme2e-svc"
FLM_TEST_SVC_APP_URL = "file:///opt/e2e"


@pytest.fixture(scope="module", autouse=True)
def setup_test_env():
    """Setup test environment with BasicTestService."""
    flamepy.register_application(
        FLM_TEST_SVC_APP,
        flamepy.ApplicationAttributes(
            shim=flamepy.Shim.Host,
            command="uv",
            working_directory="/opt/e2e",
            environments={"FLAME_LOG_LEVEL": "DEBUG"},
            arguments=["run", "src/e2e/basic_svc.py", "src/e2e/api.py"],
            url=FLM_TEST_SVC_APP_URL,
        ),
    )

    yield

    # Clean up all sessions before unregistering
    sessions = flamepy.list_sessions()
    for sess in sessions:
        try:
            flamepy.close_session(sess.id)
        except:
            pass
    
    flamepy.unregister_application(FLM_TEST_SVC_APP)


def test_basic_service_invoke():
    """Test basic invocation of BasicTestService."""
    session = flamepy.create_session(application=FLM_TEST_SVC_APP, common_data=None)

    input_data = random_string()
    request = TestRequest(input=input_data)
    
    response = session.invoke(request)
    
    assert response.output == input_data
    assert response.common_data is None
    assert response.service_state is not None
    assert response.service_state["task_count"] == 1
    assert response.service_state["session_enter_count"] == 1

    session.close()


def test_task_context_info():
    """Test that task context information is correctly returned."""
    session = flamepy.create_session(application=FLM_TEST_SVC_APP, common_data=None)

    input_data = random_string()
    request = TestRequest(
        input=input_data,
        request_task_context=True,
    )

    response = session.invoke(request)
    
    # Check basic response
    assert response.output == input_data
    
    # Check task context is present
    assert response.task_context is not None
    assert response.task_context.task_id is not None
    assert response.task_context.session_id == session.id
    assert response.task_context.has_input is True
    assert response.task_context.input_type == "TestRequest"

    session.close()


def test_session_context_info():
    """Test that session context information is correctly returned."""
    session = flamepy.create_session(application=FLM_TEST_SVC_APP, common_data=None)

    request = TestRequest(
        input="test",
        request_session_context=True,
    )

    response = session.invoke(request)
    
    # Check session context is present
    assert response.session_context is not None
    assert response.session_context.session_id == session.id
    assert response.session_context.has_common_data is False
    assert response.session_context.common_data_type is None

    session.close()


def test_application_context_info():
    """Test that application context information is correctly returned."""
    session = flamepy.create_session(application=FLM_TEST_SVC_APP, common_data=None)

    request = TestRequest(
        input="test",
        request_application_context=True,
    )

    response = session.invoke(request)
    
    # Check application context is present
    assert response.application_context is not None
    assert response.application_context.name == FLM_TEST_SVC_APP
    assert response.application_context.shim is not None
    assert "Host" in response.application_context.shim or "host" in response.application_context.shim.lower()
    assert response.application_context.command == "uv"
    assert response.application_context.working_directory == "/opt/e2e"
    assert response.application_context.url == FLM_TEST_SVC_APP_URL

    session.close()


def test_all_context_info():
    """Test that all context information is correctly returned together."""
    session = flamepy.create_session(application=FLM_TEST_SVC_APP, common_data=None)

    input_data = random_string(16)
    request = TestRequest(
        input=input_data,
        request_task_context=True,
        request_session_context=True,
        request_application_context=True,
    )

    response = session.invoke(request)
    
    # Check output
    assert response.output == input_data
    
    # Check all contexts are present
    assert response.task_context is not None
    assert response.session_context is not None
    assert response.application_context is not None
    
    # Check task context details
    assert response.task_context.task_id is not None
    assert response.task_context.session_id == session.id
    
    # Check session context details
    assert response.session_context.session_id == session.id
    assert response.session_context.application is not None
    assert response.session_context.application.name == FLM_TEST_SVC_APP
    
    # Check application context details
    assert response.application_context.name == FLM_TEST_SVC_APP
    assert response.application_context.command == "uv"
    assert response.application_context.working_directory == "/opt/e2e"
    assert response.application_context.url == FLM_TEST_SVC_APP_URL

    session.close()


def test_service_state_tracking():
    """Test that service state is maintained across multiple tasks."""
    session = flamepy.create_session(application=FLM_TEST_SVC_APP, common_data=None)

    num_tasks = 5
    for i in range(1, num_tasks + 1):
        request = TestRequest(input=f"task_{i}")
        response = session.invoke(request)
        
        # Check service state increments
        assert response.service_state is not None
        assert response.service_state["task_count"] == i
        assert response.service_state["session_enter_count"] == 1
        assert response.service_state["session_leave_count"] == 0

    session.close()


def test_common_data_without_context_request():
    """Test common data handling without requesting context info."""
    sys_context = random_string()
    session = flamepy.create_session(
        application=FLM_TEST_SVC_APP, 
        common_data=TestContext(common_data=sys_context)
    )

    input_data = random_string()
    request = TestRequest(input=input_data)

    response = session.invoke(request)
    
    assert response.output == input_data
    assert response.common_data == sys_context

    session.close()


def test_common_data_with_session_context():
    """Test that common data information is correctly reported in session context."""
    common_data = TestContext(common_data=random_string())
    session = flamepy.create_session(
        application=FLM_TEST_SVC_APP, 
        common_data=common_data
    )

    request = TestRequest(
        input="test",
        request_session_context=True,
    )

    response = session.invoke(request)
    
    # Check session context reports common data
    assert response.session_context is not None
    assert response.session_context.has_common_data is True
    assert response.session_context.common_data_type == "TestContext"

    session.close()


def test_update_common_data():
    """Test updating common data through BasicTestService."""
    sys_context = random_string()

    session = flamepy.create_session(
        application=FLM_TEST_SVC_APP, 
        common_data=TestContext(common_data=sys_context)
    )

    previous_common_data = sys_context
    for _ in range(3):
        new_input_data = random_string()
        request = TestRequest(
            input=new_input_data, 
            update_common_data=True
        )
        response = session.invoke(request)
        
        assert response.output == new_input_data
        assert response.common_data == previous_common_data

        # Verify the update took effect by checking in next iteration
        previous_common_data = new_input_data

    session.close()


def test_multiple_sessions_isolation():
    """Test that service state is isolated across different sessions."""
    # First session
    session1 = flamepy.create_session(application=FLM_TEST_SVC_APP, common_data=None)
    
    for i in range(3):
        request = TestRequest(input=f"session1_task_{i}")
        response = session1.invoke(request)
        assert response.service_state["task_count"] == i + 1
    
    session1.close()
    
    # Second session should reset state
    session2 = flamepy.create_session(application=FLM_TEST_SVC_APP, common_data=None)
    
    request = TestRequest(input="session2_task_1")
    response = session2.invoke(request)
    
    # Task count should be reset for new session
    assert response.service_state["task_count"] == 1
    assert response.service_state["session_enter_count"] == 1
    
    session2.close()


def test_context_info_selective_request():
    """Test that context info is only returned when explicitly requested."""
    session = flamepy.create_session(application=FLM_TEST_SVC_APP, common_data=None)

    # Request only task context
    request = TestRequest(
        input="test",
        request_task_context=True,
        request_session_context=False,
        request_application_context=False,
    )
    response = session.invoke(request)
    
    assert response.task_context is not None
    assert response.session_context is None
    assert response.application_context is None

    # Request only session context
    request = TestRequest(
        input="test",
        request_task_context=False,
        request_session_context=True,
        request_application_context=False,
    )
    response = session.invoke(request)
    
    assert response.task_context is None
    assert response.session_context is not None
    assert response.application_context is None

    # Request only application context
    request = TestRequest(
        input="test",
        request_task_context=False,
        request_session_context=False,
        request_application_context=True,
    )
    response = session.invoke(request)
    
    assert response.task_context is None
    assert response.session_context is None
    assert response.application_context is not None

    session.close()
