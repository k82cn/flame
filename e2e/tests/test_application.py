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

@pytest.mark.asyncio
async def test_register_application():
    await flamepy.register_application("flmtestapp", flamepy.ApplicationAttributes(
        shim=flamepy.Shim.Host,
    ))

    app = await flamepy.get_application("flmtestapp")
    assert app.name == "flmtestapp"
    assert app.shim == flamepy.Shim.Host
    assert app.state == flamepy.ApplicationState.ENABLED

    await flamepy.unregister_application("flmtestapp")


@pytest.mark.asyncio
async def test_list_application():
    apps = await flamepy.list_applications()
    assert len(apps) == 2

    for app in apps:
        assert app.name in ["flmexec", "flmping"]
        assert app.shim == flamepy.Shim.Host
        assert app.state == flamepy.ApplicationState.ENABLED

