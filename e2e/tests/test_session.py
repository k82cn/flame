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
from flamepy import SessionState
from e2e.api import TestRequest, TestResponse, TestContext
import string
import random
import threading
from concurrent.futures import wait

FLM_TEST_APP = "flme2e"


def random_string(size=8, chars=string.ascii_uppercase + string.digits) -> str:
    return "".join(random.choice(chars) for _ in range(size))


class TestTaskInformer(flamepy.TaskInformer):
    expected_output = None
    latest_state = None

    def __init__(self, expected_output):
        self.expected_output = expected_output

    def on_update(self, task):
        self.latest_state = task.state
        if task.state == flamepy.TaskState.SUCCEED:
            assert (
                task.output.output == self.expected_output
            ), f"Task output: {task.output.output}, Expected: {self.expected_output}"
        elif task.state == flamepy.TaskState.FAILED:
            for event in task.events:
                if event.code == flamepy.TaskState.FAILED:
                    raise flamepy.FlameError(
                        flamepy.FlameErrorCode.INTERNAL, f"{event.message}"
                    )

    def on_error(self, error):
        assert False, f"Task failed: {error}"


@pytest.fixture(autouse=True)
def setup_test_env():
    flamepy.register_application(
        FLM_TEST_APP,
        flamepy.ApplicationAttributes(
            shim=flamepy.Shim.Host,
            command="uv",
            working_directory="/opt/e2e",
            environments={"FLAME_LOG_LEVEL": "DEBUG"},
            arguments=["run", "src/e2e/service.py", "src/e2e/api.py"],
        ),
    )

    yield

    flamepy.unregister_application(FLM_TEST_APP)


def test_create_session():
    session = flamepy.create_session(
        application=FLM_TEST_APP, common_data=TestContext()
    )

    ssn_list = flamepy.list_sessions()
    assert len(ssn_list) == 1
    assert ssn_list[0].id == session.id
    assert ssn_list[0].application == FLM_TEST_APP
    assert ssn_list[0].state == SessionState.OPEN

    session.close()

    ssn_list = flamepy.list_sessions()
    assert len(ssn_list) == 1
    assert ssn_list[0].id == session.id
    assert ssn_list[0].application == FLM_TEST_APP
    assert ssn_list[0].state == SessionState.CLOSED


def test_invoke_task_without_common_data():
    session = flamepy.create_session(application=FLM_TEST_APP, common_data=None)

    ssn_list = flamepy.list_sessions()
    assert len(ssn_list) == 1
    assert ssn_list[0].id == session.id
    assert ssn_list[0].application == FLM_TEST_APP
    assert ssn_list[0].state == SessionState.OPEN

    input = random_string()

    output = session.invoke(TestRequest(input=input))
    assert output.output == input
    assert output.common_data is None

    session.close()


def test_invoke_task_with_common_data():
    sys_context = random_string()
    input = random_string()

    session = flamepy.create_session(
        application=FLM_TEST_APP, common_data=TestContext(common_data=sys_context)
    )

    ssn_list = flamepy.list_sessions()
    assert len(ssn_list) == 1
    assert ssn_list[0].id == session.id
    assert ssn_list[0].application == FLM_TEST_APP
    assert ssn_list[0].state == SessionState.OPEN

    output = session.invoke(TestRequest(input=input))
    assert output.output == input
    assert output.common_data == sys_context

    session.close()


def test_invoke_multiple_tasks_without_common_data():

    session = flamepy.create_session(application=FLM_TEST_APP, common_data=None)

    ssn_list = flamepy.list_sessions()
    assert len(ssn_list) == 1
    assert ssn_list[0].id == session.id
    assert ssn_list[0].application == FLM_TEST_APP
    assert ssn_list[0].state == SessionState.OPEN

    threads = []
    informers = []

    def invoke_task(session, input, informer):
        session.invoke(TestRequest(input=input), informer)

    for _ in range(10):
        input = random_string()
        informer = TestTaskInformer(input)
        informers.append(informer)
        thread = threading.Thread(target=invoke_task, args=(session, input, informer))
        thread.start()
        threads.append(thread)

    for thread in threads:
        thread.join()

    for informer in informers:
        assert informer.latest_state == flamepy.TaskState.SUCCEED

    session.close()


def test_run_multiple_tasks_with_futures():

    session = flamepy.create_session(application=FLM_TEST_APP, common_data=None)

    ssn_list = flamepy.list_sessions()
    assert len(ssn_list) == 1
    assert ssn_list[0].id == session.id
    assert ssn_list[0].application == FLM_TEST_APP
    assert ssn_list[0].state == SessionState.OPEN

    # Run multiple tasks in parallel using futures
    futures = []
    inputs = []
    for _ in range(10):
        input_str = random_string()
        inputs.append(input_str)
        future = session.run(TestRequest(input=input_str))
        futures.append(future)

    # Wait for all tasks to complete
    wait(futures)
    
    # Verify results
    for i, future in enumerate(futures):
        result = future.result()
        assert result.output == inputs[i]
        assert result.common_data is None

    session.close()


def test_update_common_data():
    sys_context = random_string()

    session = flamepy.create_session(
        application=FLM_TEST_APP, common_data=TestContext(common_data=sys_context)
    )

    ssn_list = flamepy.list_sessions()
    assert len(ssn_list) == 1
    assert ssn_list[0].id == session.id
    assert ssn_list[0].application == FLM_TEST_APP
    assert ssn_list[0].state == SessionState.OPEN

    previous_common_data = sys_context
    for _ in range(5):
        new_input_data = random_string()
        output = session.invoke(
            TestRequest(input=new_input_data, update_common_data=True)
        )
        assert output.output == new_input_data
        assert output.common_data == previous_common_data

        cxt = session.common_data()
        assert cxt.common_data == new_input_data

        previous_common_data = new_input_data

    session.close()
