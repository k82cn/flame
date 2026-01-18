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

# Type aliases
from .types import (
    TaskID,
    SessionID,
    ApplicationID,
    Message,
    TaskInput,
    TaskOutput,
    CommonData,
)

# Constants
from .types import (
    DEFAULT_FLAME_CONF,
    DEFAULT_FLAME_ENDPOINT,
    DEFAULT_FLAME_CACHE_ENDPOINT,
)

# Enums
from .types import (
    SessionState,
    TaskState,
    ApplicationState,
    Shim,
    FlameErrorCode,
)

# Exception classes
from .types import (
    FlameError,
)

# Data classes
from .types import (
    Event,
    SessionAttributes,
    ApplicationSchema,
    ApplicationAttributes,
    Task,
    Application,
    FlamePackage,
)

# Context and utility classes
from .types import (
    TaskInformer,
    FlameContext,
)

# Utility functions
from .types import (
    short_name,
)

# Client functions
from .client import (
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
)

# Client classes
from .client import (
    ConnectionInstance,
    Connection,
    Session,
    TaskWatcher,
)

# Service constants
from .service import (
    FLAME_INSTANCE_ENDPOINT,
)

# Service context classes
from .service import (
    ApplicationContext,
    SessionContext,
    TaskContext,
    TaskOutput,
)

# Service base classes
from .service import (
    FlameService,
)

# Service implementation classes
from .service import (
    FlameInstanceServicer,
    FlameInstanceServer,
)

# Service functions
from .service import (
    run,
)

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
    # Utility functions
    "short_name",
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
    "ConnectionInstance",
    "Connection",
    "Session",
    "TaskWatcher",
    # Service constants
    "FLAME_INSTANCE_ENDPOINT",
    # Service context classes
    "ApplicationContext",
    "SessionContext",
    "TaskContext",
    "TaskOutput",
    # Service base classes
    "FlameService",
    # Service implementation classes
    "FlameInstanceServicer",
    "FlameInstanceServer",
    # Service functions
    "run",
]
