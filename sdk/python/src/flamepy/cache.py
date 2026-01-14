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

import httpx
from pydantic import BaseModel
import logging
import contextlib
import pickle
from typing import Any, Optional

from .types import ObjectRef, FlameContext


@contextlib.contextmanager
def suppress_dependency_logs(level=logging.WARNING):
    """
    A context manager to temporarily suppress httpx and httpcore logs.
    """
    httpx_logger = logging.getLogger("httpx")
    httpcore_logger = logging.getLogger("httpcore")
    original_httpx_level = httpx_logger.level
    original_httpcore_level = httpcore_logger.level
    httpx_logger.setLevel(level)
    httpcore_logger.setLevel(level)
    try:
        yield
    finally:
        httpx_logger.setLevel(original_httpx_level)
        httpcore_logger.setLevel(original_httpcore_level)


class Object(BaseModel):
    """Object."""

    version: int
    data: list


class ObjectMetadata(BaseModel):
    """Object metadata."""

    endpoint: str
    version: int
    size: int


def put_object(session_id: str, obj: Any) -> "ObjectRef":
    """Put an object into the cache.
    
    Args:
        session_id: The session ID for the object
        obj: The object to cache (will be pickled)
        
    Returns:
        ObjectRef pointing to the cached object
        
    Raises:
        Exception: If cache endpoint is not configured or request fails
    """
    context = FlameContext()
    cache_endpoint = context.cache_endpoint
    
    # Serialize the object using pickle
    data = pickle.dumps(obj, protocol=pickle.HIGHEST_PROTOCOL)

    with suppress_dependency_logs():
        response = httpx.post(f"{cache_endpoint}/objects/{session_id}", data=data)
        response.raise_for_status()

    metadata = ObjectMetadata.model_validate(response.json())
    return ObjectRef(url=metadata.endpoint, version=metadata.version)


def get_object(ref: ObjectRef) -> Any:
    """Get an object from the cache.
    
    Args:
        ref: ObjectRef pointing to the cached object
        
    Returns:
        The deserialized object
        
    Raises:
        Exception: If request fails
    """
    with suppress_dependency_logs():
        response = httpx.get(ref.url)
        response.raise_for_status()

    obj = Object.model_validate(response.json())
    data = bytes(obj.data)

    # Update the version of the ObjectRef
    ref.version = obj.version

    # Deserialize the object using pickle
    return pickle.loads(data)


def update_object(ref: ObjectRef, new_obj: Any) -> "ObjectRef":
    """Update an object in the cache.
    
    Args:
        ref: ObjectRef pointing to the cached object to update
        new_obj: The new object to store (will be pickled)
        
    Returns:
        Updated ObjectRef with new version
        
    Raises:
        Exception: If request fails
    """
    # Serialize the new object using pickle
    new_data = pickle.dumps(new_obj, protocol=pickle.HIGHEST_PROTOCOL)
    
    obj = Object(version=ref.version, data=list(new_data))
    data = obj.model_dump_json()

    with suppress_dependency_logs():
        response = httpx.put(ref.url, data=data)
        response.raise_for_status()

    metadata = ObjectMetadata.model_validate(response.json())

    return ObjectRef(url=ref.url, version=metadata.version)
