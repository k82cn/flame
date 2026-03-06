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
from typing import Any, Dict, Optional, Tuple

from flamepy.core import ObjectRef, get_object
from flamepy.runner.types import RunnerRequest


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
            "kwargs": dict | None  # Resolved keyword arguments
        }
        
        For task output (result):
        {
            "type": "output",
            "result": Any  # The actual result value
        }
    
    Raises:
        ValueError: If the data cannot be decoded or retrieved from cache
        TypeError: If the data format is not recognized
    """
    # Step 1: Decode ObjectRef from bytes
    try:
        object_ref = ObjectRef.decode(data)
    except Exception as e:
        raise ValueError(f"Failed to decode ObjectRef from data: {e}")
    
    # Step 2: Retrieve object from cache
    try:
        cached_data = get_object(object_ref)
    except Exception as e:
        raise ValueError(f"Failed to retrieve object from cache: {e}")
    
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
        return _process_runner_request(cached_data)
    else:
        # This is task output (result)
        return {
            "type": "output",
            "result": cached_data
        }


def _process_runner_request(request: RunnerRequest) -> Dict[str, Any]:
    """Process a RunnerRequest and resolve any ObjectRef instances."""
    
    # Resolve args
    resolved_args = None
    if request.args is not None:
        resolved_args = tuple(_resolve_value(arg) for arg in request.args)
    
    # Resolve kwargs
    resolved_kwargs = None
    if request.kwargs is not None:
        resolved_kwargs = {
            key: _resolve_value(value) 
            for key, value in request.kwargs.items()
        }
    
    return {
        "type": "input",
        "method": request.method,
        "args": resolved_args,
        "kwargs": resolved_kwargs
    }


def _resolve_value(value: Any) -> Any:
    """Resolve a value, fetching from cache if it's an ObjectRef."""
    
    if isinstance(value, ObjectRef):
        return get_object(value)
    
    # Handle bytes that might be encoded ObjectRef
    if isinstance(value, bytes):
        try:
            object_ref = ObjectRef.decode(value)
            return get_object(object_ref)
        except Exception:
            # Not an ObjectRef, return as-is
            return value
    
    return value
