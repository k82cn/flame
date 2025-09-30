

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

_log_level = _log_level_map[_log_level_str] if _log_level_str in _log_level_map else logging.INFO

logging.basicConfig(level=_log_level)

from .types import (
    # Type aliases
    TaskID,
    SessionID,
    ApplicationID,
    Message,
    TaskInput,
    TaskOutput,
    CommonData,
    
    # Enums
    SessionState,
    TaskState,
    ApplicationState,
    Shim,

    FlameErrorCode,
    
    # Classes
    FlameError,
    SessionAttributes,
    ApplicationAttributes,
    Task,
    Application,
    FlameContext,
    TaskInformer,
    Request,
    Response,
)

from .client import (Connection, Session, TaskWatcher, 
    connect, create_session, register_application, unregister_application, list_applications, get_application, 
    list_sessions, get_session, close_session
)
from .service import (
    FlameService,
    ApplicationContext, SessionContext, TaskContext, TaskOutput,
    run
)
from .instance import FlameInstance

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
    
    # Enums
    "SessionState",
    "TaskState", 
    "ApplicationState",
    "Shim",
    "FlameErrorCode",

    # Service classes
    "FlameService",
    "ApplicationContext",
    "SessionContext",
    "TaskContext",
    "TaskOutput",
    "run",

    # Classes
    "FlameError",
    "SessionAttributes",
    "ApplicationAttributes", 
    "Task",
    "Application",
    "FlameContext",
    "TaskInformer",
    "Request",
    "Response",
    
    # Client classes
    "Connection",
    "connect",
    "create_session",
    "register_application",
    "unregister_application",
    "list_applications",
    "get_application",
    "list_sessions",
    "get_session",
    "close_session",
    "TaskWatcher",
    "Session", 
    "Task",
    "TaskInput",
    "TaskOutput",
    "CommonData",

    # Instance classes
    "FlameInstance",
] 