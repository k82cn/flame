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

import logging
import os

_log_level_str = os.getenv("FLAME_LOG_LEVEL", "INFO").upper()
_log_level_map = {
    "CRITICAL": logging.CRITICAL,
    "ERROR": logging.ERROR,
    "WARNING": logging.WARNING,
    "INFO": logging.INFO,
    "DEBUG": logging.DEBUG,
}

_log_level = (
    _log_level_map[_log_level_str] if _log_level_str in _log_level_map else logging.INFO
)

logging.basicConfig(level=_log_level)

# Export all core classes/types at top level
from .core import (
    # Type aliases
    TaskID,
    SessionID,
    ApplicationID,
    Message,
    TaskInput,
    TaskOutput,
    CommonData,
    # Constants
    DEFAULT_FLAME_CONF,
    DEFAULT_FLAME_ENDPOINT,
    DEFAULT_FLAME_CACHE_ENDPOINT,
    # Enums
    SessionState,
    TaskState,
    ApplicationState,
    Shim,
    FlameErrorCode,
    # Exception classes
    FlameError,
    # Data classes
    Event,
    SessionAttributes,
    ApplicationSchema,
    ApplicationAttributes,
    Task,
    Application,
    FlamePackage,
    # Context and utility classes
    TaskInformer,
    FlameContext,
    # Client functions
    connect,
    create_session,
    open_session,
    register_application,
    unregister_application,
    list_applications,
    get_application,
    list_sessions,
    get_session,
    close_session,
    # Client classes
    Connection,
    Session,
    TaskWatcher,
    # Service constants
    FLAME_INSTANCE_ENDPOINT,
    # Service context classes
    ApplicationContext,
    SessionContext,
    TaskContext,
    # Service base classes
    FlameService,
    # Service functions
    run,
)

# Import submodules for rl, agent, and cache (only as submodules)
from . import agent
from . import rl
from . import cache

__version__ = "0.3.0"

__all__ = [
    # Type aliases
    "TaskID",
    "SessionID",
    "ApplicationID",
    "Message",
    "TaskInput",
    "TaskOutput",
    "CommonData",
    # Constants
    "DEFAULT_FLAME_CONF",
    "DEFAULT_FLAME_ENDPOINT",
    "DEFAULT_FLAME_CACHE_ENDPOINT",
    # Enums
    "SessionState",
    "TaskState",
    "ApplicationState",
    "Shim",
    "FlameErrorCode",
    # Exception classes
    "FlameError",
    # Data classes
    "Event",
    "SessionAttributes",
    "ApplicationSchema",
    "ApplicationAttributes",
    "Task",
    "Application",
    "FlamePackage",
    # Context and utility classes
    "TaskInformer",
    "FlameContext",
    # Client functions
    "connect",
    "create_session",
    "open_session",
    "register_application",
    "unregister_application",
    "list_applications",
    "get_application",
    "list_sessions",
    "get_session",
    "close_session",
    # Client classes
    "Connection",
    "Session",
    "TaskWatcher",
    # Service constants
    "FLAME_INSTANCE_ENDPOINT",
    # Service context classes
    "ApplicationContext",
    "SessionContext",
    "TaskContext",
    # Service base classes
    "FlameService",
    # Service functions
    "run",
    # Submodules (rl, agent, and cache only)
    "agent",
    "rl",
    "cache",
]
