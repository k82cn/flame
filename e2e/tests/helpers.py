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

import json
from dataclasses import asdict
from typing import Optional, Any
from flamepy.cache import ObjectRef
from flamepy.cache.cache import put_object, get_object
from flamepy.core.types import short_name

from e2e.api import (
    TestRequest, 
    TestResponse, 
    TestContext,
    TaskContextInfo,
    SessionContextInfo,
    ApplicationContextInfo,
)


def serialize_common_data(common_data: Optional[TestContext], app_name: str) -> Optional[bytes]:
    """
    Serialize common data to bytes for core API.
    
    Uses JSON serialization, then puts in cache to get ObjectRef, then encodes to bytes.
    
    Args:
        common_data: TestContext object to serialize, or None
        app_name: Application name for generating session ID
        
    Returns:
        bytes representation of ObjectRef, or None if common_data is None
    """
    if common_data is None:
        return None
    
    # Serialize with JSON
    serialized_ctx = json.dumps(asdict(common_data)).encode('utf-8')
    # Put in cache to get ObjectRef
    temp_session_id = short_name(app_name)
    object_ref = put_object(temp_session_id, serialized_ctx)
    # Encode ObjectRef to bytes for core API
    return object_ref.encode()


def deserialize_common_data(common_data_bytes: Optional[bytes]) -> Optional[TestContext]:
    """
    Deserialize common data from bytes.
    
    Decodes bytes to ObjectRef, gets from cache, then deserializes from JSON.
    
    Args:
        common_data_bytes: bytes representation of ObjectRef, or None
        
    Returns:
        TestContext object, or None if common_data_bytes is None
    """
    if common_data_bytes is None:
        return None
    
    # Decode bytes to ObjectRef
    object_ref = ObjectRef.decode(common_data_bytes)
    # Get from cache (returns JSON bytes)
    serialized_ctx = get_object(object_ref)
    # Deserialize from JSON
    ctx_dict = json.loads(serialized_ctx.decode('utf-8'))
    return TestContext(**ctx_dict)


def serialize_request(request: TestRequest) -> bytes:
    """
    Serialize a TestRequest to bytes using JSON.
    
    Args:
        request: TestRequest object
        
    Returns:
        bytes representation of the request
    """
    request_dict = asdict(request)
    return json.dumps(request_dict).encode('utf-8')


def deserialize_response(response_bytes: bytes) -> TestResponse:
    """
    Deserialize bytes to TestResponse using JSON.
    
    Args:
        response_bytes: bytes representation of the response
        
    Returns:
        TestResponse object
    """
    response_dict = json.loads(response_bytes.decode('utf-8'))
    
    # Convert nested dictionaries to proper dataclass instances
    if 'task_context' in response_dict and response_dict['task_context'] is not None:
        response_dict['task_context'] = TaskContextInfo(**response_dict['task_context'])
    
    if 'session_context' in response_dict and response_dict['session_context'] is not None:
        session_ctx_dict = response_dict['session_context']
        # Convert nested application context if present
        if 'application' in session_ctx_dict and session_ctx_dict['application'] is not None:
            session_ctx_dict['application'] = ApplicationContextInfo(**session_ctx_dict['application'])
        response_dict['session_context'] = SessionContextInfo(**session_ctx_dict)
    
    if 'application_context' in response_dict and response_dict['application_context'] is not None:
        response_dict['application_context'] = ApplicationContextInfo(**response_dict['application_context'])
    
    return TestResponse(**response_dict)


def invoke_task(session, request: TestRequest) -> TestResponse:
    """
    Helper function to invoke a task and deserialize the response.
    
    Args:
        session: Session object
        request: TestRequest object
        
    Returns:
        TestResponse object
    """
    request_bytes = serialize_request(request)
    response_bytes = session.invoke(request_bytes)
    return deserialize_response(response_bytes)
