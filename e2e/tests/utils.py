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

import random
import string
import subprocess
import time
from pathlib import Path
from typing import Mapping, Optional, Sequence

import flamepy
import pytest

E2E_PROJECT = Path(__file__).resolve().parents[1]


def deploy_e2e_application(
    name: str,
    *,
    command: Optional[str] = None,
    arguments: Optional[Sequence[str]] = None,
    description: Optional[str] = None,
    shim: Optional[str] = None,
    image: Optional[str] = None,
    environments: Optional[Mapping[str, str]] = None,
) -> None:
    """Deploy the E2E project through the same package path as user applications."""
    args = ["flmctl", "deploy", "--name", name, "--application", str(E2E_PROJECT)]
    if command is not None:
        args.extend(["--command", command])
    if arguments is not None:
        args.extend(f"--argument={argument}" for argument in arguments)
    if description is not None:
        args.extend(["--description", description])
    if shim is not None:
        args.extend(["--shim", shim])
    if image is not None:
        args.extend(["--image", image])
    if environments is not None:
        args.extend(f"--env={key}={value}" for key, value in environments.items())

    try:
        result = subprocess.run(args, capture_output=True, text=True, check=False)
    except OSError as exc:
        pytest.fail(f"Could not run flmctl deploy: {exc}")
    if result.returncode:
        pytest.fail(f"E2E package deployment failed: {result.stderr or result.stdout}")


def random_string(size=16) -> str:
    """Generate a random string of specified size.

    Args:
        size: Length of the random string (default: 16)

    Returns:
        A random string of the specified size
    """
    return "".join(random.choice(string.ascii_letters + string.digits) for _ in range(size))


def get_application_by_name(name: str):
    """Resolve a name in the default workspace or a canonical app ID."""
    workspace, app_name = name.split("/", 1) if "/" in name else ("default", name)
    return flamepy.get_application(app_name, workspace=workspace)


def application(name: str) -> str:
    application = get_application_by_name(name)
    if application is None:
        raise LookupError(f"Application '{name}' was not found")
    return f"{application.workspace}/{application.name}"


def unregister_application_by_name(name: str) -> None:
    application = get_application_by_name(name)
    if application is not None:
        flamepy.unregister_application(application.name, workspace=application.workspace)


def create_session_by_application_name(application: str, *args, **kwargs):
    """Create a session using the resolved application ID."""
    app = get_application_by_name(application)
    return flamepy.create_session(app.name, *args, workspace=app.workspace, **kwargs)


def wait_for_application_deleted(name: str, timeout: float = 10.0) -> None:
    """Wait for FSM to physically remove a disabled application."""
    deadline = time.monotonic() + timeout
    application = get_application_by_name(name)
    while application is not None and time.monotonic() < deadline:
        time.sleep(0.1)
        application = get_application_by_name(name)

    if application is not None:
        pytest.fail(f"Application '{name}' was not deleted within {timeout} seconds; last state: {application.state}")
