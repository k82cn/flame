
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

FLM_TEST_APP = "flme2e"

class MyTaskInformer(flamepy.TaskInformer):
    """Example task informer that prints task updates."""
    
    def on_update(self, task):
        pass
    
    def on_error(self, error):
        pass

@pytest.fixture(autouse=True, scope="module")
def setup_test_env():
    asyncio.run(flamepy.register_application(FLM_TEST_APP, flamepy.ApplicationAttributes(
        shim=flamepy.Shim.Host,
        command="uv",
        working_directory="/opt/e2e",
        arguments=["run", "src/e2e/service.py", "src/e2e/api.py"],
    )))

    yield
    
    asyncio.run(flamepy.unregister_application(FLM_TEST_APP))

@pytest.mark.asyncio
async def test_create_session():
    session = await flamepy.create_session(
        application=FLM_TEST_APP,
        common_data=b"shared data"
    )

    await session.invoke(b"task input data", MyTaskInformer())
    await session.close()


@pytest.mark.asyncio
async def test_invoke_multiple_tasks():
    session = await flamepy.create_session(
        application=FLM_TEST_APP,
        common_data=b"shared data"
    )

    for i in range(10):
        await session.invoke(b"task input data", MyTaskInformer())
    await session.close()


@pytest.mark.asyncio
async def test_invoke_multiple_sessions():
    for i in range(10):
        session = await flamepy.create_session(
            application=FLM_TEST_APP,
            common_data=b"shared data"
        )

        for i in range(10):
            task = await session.invoke(b"task input data", MyTaskInformer())
            assert task.state == flamepy.TaskState.SUCCEED
            # assert task.output == b"task output data"
            assert task.message is None
        await session.close()

