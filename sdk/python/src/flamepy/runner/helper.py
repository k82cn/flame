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

import cloudpickle
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Dict, Optional

from flamepy.core import ObjectRef, get_object
from flamepy.runner.types import RunnerRequest


class ErrorType(Enum):
    """Error types for RunnerError."""

    DECODE_ERROR = "decode_error"
    CACHE_RETRIEVAL_ERROR = "cache_retrieval_error"
    DATA_FORMAT_ERROR = "data_format_error"


class RunnerError(Exception):
    """Exception for runner helper errors.

    Attributes:
        error_type: The type of error (from ErrorType enum).
        message: Human-readable error message.
        cause: The underlying exception that caused this error (if any).
        key: The cache key involved (for cache retrieval errors).
        data_type: The data type involved (for data format errors).
    """

    def __init__(
        self,
        error_type: ErrorType,
        message: str,
        cause: Optional[Exception] = None,
        key: Optional[str] = None,
        data_type: Optional[str] = None,
    ):
        super().__init__(message)
        self.error_type = error_type
        self.cause = cause
        self.key = key
        self.data_type = data_type

    def __str__(self) -> str:
        return f"[{self.error_type.value}] {super().__str__()}"


@dataclass
class TaskInputData:
    """Structured response for task input data.

    Attributes:
        type: Always "input" for task input data.
        method: Method name or None for callable/function.
        args: Resolved positional arguments.
        kwargs: Resolved keyword arguments.
        metadata: Additional metadata about the retrieval.
    """

    type: str = field(default="input", init=False)
    method: str = None
    args: tuple = None
    kwargs: Dict[str, Any] = None
    metadata: Dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary representation."""
        return {
            "type": self.type,
            "method": self.method,
            "args": self.args,
            "kwargs": self.kwargs,
            "metadata": self.metadata,
        }


@dataclass
class TaskOutputData:
    """Structured response for task output data.

    Attributes:
        type: Always "output" for task output data.
        result: The actual result value.
        metadata: Additional metadata about the retrieval.
    """

    type: str = field(default="output", init=False)
    result: Any = None
    metadata: Dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary representation."""
        return {
            "type": self.type,
            "result": self.result,
            "metadata": self.metadata,
        }


def get_data(data: bytes) -> Dict[str, Any]:
    """Retrieve the real data from task input or output.

    This function takes the raw bytes from a Flame task's input or output,
    decodes the ObjectRef, retrieves the data from cache, and resolves
    any nested ObjectRef instances to their actual values.

    Args:
        data: Raw bytes from task input or output. This is expected to be
              an encoded ObjectRef pointing to either:
              - A RunnerRequest (for task input)
              - A result object (for task output)

    Returns:
        A dictionary containing the resolved data:

        For task input (RunnerRequest):
        {
            "type": "input",
            "method": str | None,  # Method name or None for callable
            "args": tuple | None,  # Resolved positional arguments
            "kwargs": dict | None,  # Resolved keyword arguments
            "metadata": dict       # Additional metadata
        }

        For task output (result):
        {
            "type": "output",
            "result": Any,  # The actual result value
            "metadata": dict  # Additional metadata
        }

    Raises:
        RunnerError: With error_type indicating the specific error:
            - ErrorType.DECODE_ERROR: If the data cannot be decoded as ObjectRef
            - ErrorType.CACHE_RETRIEVAL_ERROR: If the object cannot be retrieved from cache
            - ErrorType.DATA_FORMAT_ERROR: If the data format is not recognized

    Example:
        >>> from flamepy.runner import get_data
        >>> from flamepy.core import get_session
        >>>
        >>> # Get a session and its tasks
        >>> session = get_session("my-session-id")
        >>> for task in session.tasks:
        ...     if task.input:
        ...         input_data = get_data(task.input)
        ...         print(f"Task {task.id} input: {input_data}")
        ...     if task.output:
        ...         output_data = get_data(task.output)
        ...         print(f"Task {task.id} output: {output_data}")
    """
    # Step 1: Decode ObjectRef from bytes
    try:
        object_ref = ObjectRef.decode(data)
    except Exception as e:
        raise RunnerError(
            ErrorType.DECODE_ERROR,
            f"Failed to decode ObjectRef from data: {e}",
            cause=e,
        )

    # Step 2: Retrieve object from cache
    try:
        cached_data = get_object(object_ref)
    except Exception as e:
        raise RunnerError(
            ErrorType.CACHE_RETRIEVAL_ERROR,
            f"Failed to retrieve object from cache: {e}",
            cause=e,
            key=getattr(object_ref, "key", None),
        )

    # Step 3: Check if it's serialized data (bytes) that needs unpickling
    if isinstance(cached_data, bytes):
        try:
            cached_data = cloudpickle.loads(cached_data)
        except Exception:
            # Not pickled data, use as-is
            pass

    # Step 4: Determine type and process accordingly
    if isinstance(cached_data, RunnerRequest):
        # This is task input
        return _process_runner_request(cached_data, object_ref)
    else:
        # This is task output (result)
        output_data = TaskOutputData(
            result=cached_data,
            metadata={"object_ref_key": object_ref.key},
        )
        return output_data.to_dict()


def _process_runner_request(request: RunnerRequest, object_ref: ObjectRef = None) -> Dict[str, Any]:
    """Process a RunnerRequest and resolve any ObjectRef instances.

    Args:
        request: The RunnerRequest to process.
        object_ref: Optional ObjectRef for metadata.

    Returns:
        Dictionary with resolved input data.
    """
    # Resolve args (recursively handles nested structures)
    resolved_args = None
    if request.args is not None:
        resolved_args = tuple(_resolve_value(arg) for arg in request.args)

    # Resolve kwargs (recursively handles nested structures)
    resolved_kwargs = None
    if request.kwargs is not None:
        resolved_kwargs = {key: _resolve_value(value) for key, value in request.kwargs.items()}

    metadata = {}
    if object_ref is not None:
        metadata["object_ref_key"] = object_ref.key

    input_data = TaskInputData(
        method=request.method,
        args=resolved_args,
        kwargs=resolved_kwargs,
        metadata=metadata,
    )
    return input_data.to_dict()


def _resolve_value(value: Any, max_depth: int = 10, _current_depth: int = 0) -> Any:
    """Resolve a value, fetching from cache if it's an ObjectRef.

    Recursively resolves nested structures (lists, dicts, tuples) that may
    contain ObjectRef instances.

    Args:
        value: The value to resolve.
        max_depth: Maximum recursion depth to prevent infinite loops.
        _current_depth: Current recursion depth (internal use).

    Returns:
        The resolved value with all ObjectRef instances replaced by their actual data.

    Raises:
        RunnerError: With ErrorType.CACHE_RETRIEVAL_ERROR if an ObjectRef cannot be resolved.
    """
    # Prevent infinite recursion
    if _current_depth > max_depth:
        return value

    # Handle ObjectRef directly
    if isinstance(value, ObjectRef):
        try:
            return get_object(value)
        except Exception as e:
            raise RunnerError(
                ErrorType.CACHE_RETRIEVAL_ERROR,
                f"Failed to resolve ObjectRef: {e}",
                cause=e,
                key=getattr(value, "key", None),
            )

    # Handle bytes that might be encoded ObjectRef
    if isinstance(value, bytes):
        try:
            object_ref = ObjectRef.decode(value)
            return get_object(object_ref)
        except Exception:
            # Not an ObjectRef, return as-is
            return value

    # Handle lists - recursively resolve each element
    if isinstance(value, list):
        return [_resolve_value(item, max_depth, _current_depth + 1) for item in value]

    # Handle tuples - recursively resolve each element
    if isinstance(value, tuple):
        return tuple(_resolve_value(item, max_depth, _current_depth + 1) for item in value)

    # Handle dicts - recursively resolve each value
    if isinstance(value, dict):
        return {k: _resolve_value(v, max_depth, _current_depth + 1) for k, v in value.items()}

    # Return other types as-is
    return value
