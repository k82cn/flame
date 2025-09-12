

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

if os.getenv("FLAME_LOG_LEVEL", "INFO") == "DEBUG":
    logging.basicConfig(level=logging.DEBUG, filename="flamepy.log")
else:
    logging.basicConfig(level=logging.INFO, filename="flamepy.log")

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

from .client import Connection, Session, TaskWatcher, connect, create_session, register_application, unregister_application, list_applications, get_application
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
    "Session",
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
    "TaskWatcher",
    "Session", 
    "Task",
    "TaskInput",
    "TaskOutput",
    "CommonData",

    # Instance classes
    "FlameInstance",
] 