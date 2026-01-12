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

def test_register_application():
    flamepy.register_application("flmtestapp", flamepy.ApplicationAttributes(
        shim=flamepy.Shim.Host,
    ))

    app = flamepy.get_application("flmtestapp")
    assert app.name == "flmtestapp"
    assert app.shim == flamepy.Shim.Host
    assert app.state == flamepy.ApplicationState.ENABLED

    flamepy.unregister_application("flmtestapp")


def test_list_application():
    apps = flamepy.list_applications()
    assert len(apps) == 2

    for app in apps:
        assert app.name in ["flmexec", "flmping"]
        assert app.shim == flamepy.Shim.Host
        assert app.state == flamepy.ApplicationState.ENABLED


def test_application_with_url():
    """Test registering and retrieving an application with URL field."""
    test_url = "file:///opt/test-package.whl"
    
    # Register application with URL
    flamepy.register_application("flmtestapp-url", flamepy.ApplicationAttributes(
        shim=flamepy.Shim.Host,
        url=test_url,
        description="Test application with URL",
    ))

    # Retrieve and verify URL field
    app = flamepy.get_application("flmtestapp-url")
    assert app.name == "flmtestapp-url"
    assert app.shim == flamepy.Shim.Host
    assert app.state == flamepy.ApplicationState.ENABLED
    assert app.url == test_url, f"Expected url to be '{test_url}', got '{app.url}'"
    assert app.description == "Test application with URL"

    # Clean up
    flamepy.unregister_application("flmtestapp-url")
    
    
def test_application_without_url():
    """Test that application without URL field works correctly (backward compatibility)."""
    # Register application without URL
    flamepy.register_application("flmtestapp-no-url", flamepy.ApplicationAttributes(
        shim=flamepy.Shim.Host,
        description="Test application without URL",
    ))

    # Retrieve and verify URL field is None
    app = flamepy.get_application("flmtestapp-no-url")
    assert app.name == "flmtestapp-no-url"
    assert app.shim == flamepy.Shim.Host
    assert app.state == flamepy.ApplicationState.ENABLED
    assert app.url is None, f"Expected url to be None, got '{app.url}'"
    assert app.description == "Test application without URL"

    # Clean up
    flamepy.unregister_application("flmtestapp-no-url")

