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

from concurrent.futures import wait

import pytest
import flamepy
from e2e.api import TestRequest
from e2e.helpers import invoke_task
from tests.utils import random_string


FLM_TEST_SVC_APP = "flme2e-svc"
FLM_TEST_SVC_APP_URL = "file:///opt/e2e"


@pytest.fixture(scope="module", autouse=True)
def setup_test_env():
    flamepy.register_application(
        FLM_TEST_SVC_APP,
        flamepy.ApplicationAttributes(
            command="${FLAME_HOME}/bin/uv",
            working_directory="/opt/e2e",
            environments={"FLAME_LOG_LEVEL": "DEBUG"},
            arguments=["run", "src/e2e/basic_svc.py", "src/e2e/api.py"],
            url=FLM_TEST_SVC_APP_URL,
        ),
    )

    yield

    sessions = flamepy.list_sessions()
    for sess in sessions:
        try:
            flamepy.close_session(sess.id)
        except:
            pass

    flamepy.unregister_application(FLM_TEST_SVC_APP)


def test_batch_session_basic():
    session = flamepy.create_session(
        application=FLM_TEST_SVC_APP,
        batch_size=2,
        min_instances=2,
    )

    task_num = 4
    for i in range(task_num):
        request = TestRequest(input=f"batch_task_{i}")
        response = invoke_task(session, request)
        assert response.output == f"batch_task_{i}"

    session.close()


def test_batch_session_parallel_tasks():
    session = flamepy.create_session(
        application=FLM_TEST_SVC_APP,
        batch_size=2,
        min_instances=2,
    )

    task_num = 4
    futures = []
    for i in range(task_num):
        future = session.run(f"parallel_batch_task_{i}".encode())
        futures.append(future)

    wait(futures)
    results = [f.result() for f in futures]

    assert len(results) == task_num

    session.close()
