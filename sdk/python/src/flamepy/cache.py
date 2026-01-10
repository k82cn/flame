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

from .types import ObjectExpr, DataSource, FlameContext


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


def put_object(session_id: str, data: bytes) -> "ObjectExpr":
    """Put an object into the cache."""
    context = FlameContext()
    if context._cache_endpoint is None or data is None:
        return ObjectExpr(source=DataSource.LOCAL, data=data)

    with suppress_dependency_logs():
        response = httpx.post(f"{context._cache_endpoint}/objects/{session_id}", data=data)
        response.raise_for_status()

    metadata = ObjectMetadata.model_validate(response.json())
    return ObjectExpr(source=DataSource.REMOTE, url=metadata.endpoint, data=data, version=metadata.version)


def get_object(de: ObjectExpr) -> "ObjectExpr":
    """Get an object from the cache."""
    if de.source != DataSource.REMOTE:
        return de

    with suppress_dependency_logs():
        response = httpx.get(de.url)
        response.raise_for_status()

    obj = Object.model_validate(response.json())

    de.data = bytes(obj.data)
    de.version = obj.version

    return de


def update_object(de: ObjectExpr) -> "ObjectExpr":
    """Update an object in the cache."""
    if de.source != DataSource.REMOTE:
        return de

    obj = Object(version=de.version, data=list(de.data))
    data = obj.model_dump_json()

    with suppress_dependency_logs():
        response = httpx.put(de.url, data=data)
        response.raise_for_status()

    metadata = ObjectMetadata.model_validate(response.json())

    de.version = metadata.version

    return de
