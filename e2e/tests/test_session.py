
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
import asyncio
import pytest_asyncio
import flamepy
from flamepy import SessionState
from e2e.api import TestRequest, TestResponse, TestContext
import string
import random

FLM_TEST_APP = "flme2e"

def random_string(size=8, chars=string.ascii_uppercase + string.digits) -> str:
    return ''.join(random.choice(chars) for _ in range(size))

class MyTaskInformer(flamepy.TaskInformer):
    """Example task informer that prints task updates."""
    
    def on_update(self, task):
        pass
    
    def on_error(self, error):
        pass

@pytest.fixture(autouse=True)
def setup_test_env():
    asyncio.run(flamepy.register_application(FLM_TEST_APP, flamepy.ApplicationAttributes(
        shim=flamepy.Shim.Host,
        command="uv",
        working_directory="/opt/e2e",
        environments={
            "FLAME_LOG_LEVEL": "DEBUG"
        },
        arguments=["run", "src/e2e/service.py", "src/e2e/api.py"],
    )))

    yield

    asyncio.run(flamepy.unregister_application(FLM_TEST_APP))

@pytest.mark.asyncio
async def test_create_session():
    session = await flamepy.create_session(
        application=FLM_TEST_APP,
        common_data=TestContext()
    )

    ssn_list = await flamepy.list_sessions()
    assert len(ssn_list) == 1
    assert ssn_list[0].id == session.id
    assert ssn_list[0].application == FLM_TEST_APP
    assert ssn_list[0].state == SessionState.OPEN

    await session.close()

    ssn_list = await flamepy.list_sessions()
    assert len(ssn_list) == 1
    assert ssn_list[0].id == session.id
    assert ssn_list[0].application == FLM_TEST_APP
    assert ssn_list[0].state == SessionState.CLOSED


@pytest.mark.asyncio
async def test_invoke_task_without_common_data():
    session = await flamepy.create_session(
        application=FLM_TEST_APP,
        common_data=None
    )

    ssn_list = await flamepy.list_sessions()
    assert len(ssn_list) == 1
    assert ssn_list[0].id == session.id
    assert ssn_list[0].application == FLM_TEST_APP
    assert ssn_list[0].state == SessionState.OPEN

    input = random_string()

    resp = await session.invoke(TestRequest(input=input))
    output = TestResponse.from_json(resp)
    assert output.output == input
    assert output.common_data is None

    await session.close()

@pytest.mark.asyncio
async def test_invoke_task_with_common_data():
    sys_context = random_string()
    input = random_string()

    session = await flamepy.create_session(
        application=FLM_TEST_APP,
        common_data=TestContext(common_data=sys_context)
    )

    ssn_list = await flamepy.list_sessions()
    assert len(ssn_list) == 1
    assert ssn_list[0].id == session.id
    assert ssn_list[0].application == FLM_TEST_APP
    assert ssn_list[0].state == SessionState.OPEN

    resp = await session.invoke(TestRequest(input=input))
    output = TestResponse.from_json(resp)
    assert output.output == input
    assert output.common_data == sys_context

    await session.close()


@pytest.mark.asyncio
async def test_invoke_multiple_tasks():

    session = await flamepy.create_session(
        application = FLM_TEST_APP,
        common_data = None
    )

    ssn_list = await flamepy.list_sessions()
    assert len(ssn_list) == 1
    assert ssn_list[0].id == session.id
    assert ssn_list[0].application == FLM_TEST_APP
    assert ssn_list[0].state == SessionState.OPEN

    for i in range(10):
        await session.invoke(TestRequest(), MyTaskInformer())

    await session.close()

