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

import flamepy
import json
from typing import Optional
from dataclasses import asdict

from e2e.api import (
    TestRequest,
    TestResponse,
    TestContext,
    ApplicationContextInfo,
    SessionContextInfo,
    TaskContextInfo,
)
from flamepy.core import ObjectRef, get_object


class BasicTestService(flamepy.FlameService):
    """Service that implements FlameService for testing with context information."""

    def __init__(self):
        self._session_context: Optional[flamepy.SessionContext] = None
        self._task_count = 0
        self._session_enter_count = 0
        self._session_leave_count = 0

    def on_session_enter(self, context: flamepy.SessionContext):
        """Handle session enter and store context."""
        self._session_context = context
        self._session_enter_count += 1
        self._task_count = 0

    def on_task_invoke(self, context: flamepy.TaskContext) -> flamepy.TaskOutput:
        """Handle task invoke and return response with optional context information."""
        self._task_count += 1

        # Deserialize task input from bytes using JSON
        request: TestRequest = None
        if context.input is not None:
            request_dict = json.loads(context.input.decode('utf-8'))
            request = TestRequest(**request_dict)

        # Get common data - deserialize from bytes using JSON
        common_data = None
        if self._session_context is not None:
            common_data_bytes = self._session_context.common_data()
            if common_data_bytes is not None:
                # Decode bytes to ObjectRef, get from cache, then deserialize from JSON
                object_ref = ObjectRef.decode(common_data_bytes)
                serialized_ctx = get_object(object_ref)
                ctx_dict = json.loads(serialized_ctx.decode('utf-8'))
                cxt_data = TestContext(**ctx_dict)
                common_data = cxt_data.common_data if hasattr(cxt_data, 'common_data') else None

        # Update common data if requested
        # Note: Since update_common_data() was removed from SessionContext,
        # we can't update it directly. This test service stores the update locally
        # but it won't persist across tasks. For production use, use agent module.
        updated_context = None
        if request and request.update_common_data and self._session_context is not None:
            # Store updated context locally for this response
            updated_context = TestContext(common_data=request.input)
            # Note: This won't persist - SessionContext doesn't support updates anymore
            # For persistent updates, the client should recreate the session with new common_data

        # Use updated context if available, otherwise use original
        response_common_data = updated_context.common_data if updated_context else common_data
        
        # Build response
        response = TestResponse(
            output=request.input if request else None,
            common_data=response_common_data,
            service_state={
                "task_count": self._task_count,
                "session_enter_count": self._session_enter_count,
                "session_leave_count": self._session_leave_count,
            }
        )

        # Add task context information if requested
        if request and request.request_task_context:
            response.task_context = TaskContextInfo(
                task_id=context.task_id,
                session_id=context.session_id,
                has_input=context.input is not None,
                input_type=type(request).__name__ if request else None,
            )

        # Add session context information if requested
        if request and request.request_session_context and self._session_context is not None:
            common_data_bytes = self._session_context.common_data()
            cxt_data = None
            if common_data_bytes is not None:
                # Decode and deserialize to get the actual context object using JSON
                object_ref = ObjectRef.decode(common_data_bytes)
                serialized_ctx = get_object(object_ref)
                ctx_dict = json.loads(serialized_ctx.decode('utf-8'))
                cxt_data = TestContext(**ctx_dict)
            
            response.session_context = SessionContextInfo(
                session_id=self._session_context.session_id,
                has_common_data=cxt_data is not None,
                common_data_type=type(cxt_data).__name__ if cxt_data is not None else None,
            )

        # Add application context information if requested
        if request and request.request_application_context and self._session_context is not None:
            app_ctx = self._session_context.application
            
            app_info = ApplicationContextInfo(
                name=app_ctx.name,
                shim=app_ctx.shim.name,
                image=app_ctx.image,
                command=app_ctx.command,
                working_directory=app_ctx.working_directory,
                url=app_ctx.url,
            )
            
            response.application_context = app_info
            
            # Also add to session_context if it exists
            if response.session_context is not None:
                response.session_context.application = app_info

        # Serialize response to bytes using JSON for core API
        response_dict = asdict(response)
        response_bytes = json.dumps(response_dict).encode('utf-8')
        return flamepy.TaskOutput(data=response_bytes)

    def on_session_leave(self):
        """Handle session leave."""
        self._session_leave_count += 1
        self._session_context = None


if __name__ == "__main__":
    flamepy.run(BasicTestService())
