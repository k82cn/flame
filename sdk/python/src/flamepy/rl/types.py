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

import inspect
from dataclasses import dataclass
from enum import IntEnum
from typing import Any, Optional, Tuple, Dict

from flamepy.core import ObjectRef


class RunnerServiceKind(IntEnum):
    """Runner service kind enumeration."""

    Stateful = 0
    Stateless = 1


@dataclass
class RunnerContext:
    """Context for runner session containing the shared execution object.

    This class encapsulates data shared within a session, including the
    execution object specific to the session.

    Attributes:
        execution_object: The execution object for the customized session. This can be
                          any Python object (function, class instance, etc.) that will
                          be used to execute tasks within the session.
        kind: The kind of the runner service, if specified.
    """

    execution_object: Any
    kind: Optional[RunnerServiceKind] = None

    def __post_init__(self) -> None:
        if self.kind is not None:
            return
        if (
            inspect.isfunction(self.execution_object)
            or inspect.isbuiltin(self.execution_object)
            or (
                inspect.isclass(self.execution_object)
                and self.execution_object.__module__ == "builtins"
            )
        ):
            self.kind = RunnerServiceKind.Stateless
        else:
            self.kind = RunnerServiceKind.Stateful


@dataclass
class RunnerRequest:
    """Request for runner task invocation.

    This class defines the input for each task and contains information about
    which method to invoke and what arguments to pass.

    Attributes:
        method: The name of the method to invoke within the customized application.
                Should be None if the execution object itself is a function or callable.
        args: A tuple containing positional arguments for the method. Optional.
                Can contain ObjectRef instances that will be resolved at runtime.
        kwargs: A dictionary of keyword arguments for the method. Optional.
                Can contain ObjectRef instances that will be resolved at runtime.

    Note: If both args and kwargs are None, the method will be called without arguments.
    """

    method: Optional[str] = None
    args: Optional[Tuple] = None
    kwargs: Optional[Dict[str, Any]] = None

    def __post_init__(self):
        """Validate RunnerRequest fields."""
        if self.method is not None and not isinstance(self.method, str):
            raise ValueError(
                f"method must be a string or None, got {type(self.method)}"
            )
        if self.args is not None and not isinstance(self.args, (tuple, list)):
            raise ValueError(
                f"args must be a tuple or list, got {type(self.args)}"
            )
        if self.kwargs is not None and not isinstance(self.kwargs, dict):
            raise ValueError(
                f"kwargs must be a dict, got {type(self.kwargs)}"
            )
