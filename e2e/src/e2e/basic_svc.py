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
from typing import Optional

from e2e.api import (
    TestRequest,
    TestResponse,
    TestContext,
    ApplicationContextInfo,
    SessionContextInfo,
    TaskContextInfo,
)


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

        # Get the request (already deserialized by SDK)
        request: TestRequest = context.input

        # Get common data
        common_data = None
        if self._session_context is not None:
            cxt_data = self._session_context.common_data()
            if cxt_data is not None:
                common_data = cxt_data.common_data if hasattr(cxt_data, 'common_data') else None

        # Update common data if requested
        if request.update_common_data and self._session_context is not None:
            self._session_context.update_common_data(TestContext(common_data=request.input))

        # Build response
        response = TestResponse(
            output=request.input,
            common_data=common_data,
            service_state={
                "task_count": self._task_count,
                "session_enter_count": self._session_enter_count,
                "session_leave_count": self._session_leave_count,
            }
        )

        # Add task context information if requested
        if request.request_task_context:
            response.task_context = TaskContextInfo(
                task_id=context.task_id,
                session_id=context.session_id,
                has_input=context.input is not None,
                input_type=type(context.input).__name__ if context.input is not None else None,
            )

        # Add session context information if requested
        if request.request_session_context and self._session_context is not None:
            cxt_data = self._session_context.common_data()
            response.session_context = SessionContextInfo(
                session_id=self._session_context.session_id,
                has_common_data=cxt_data is not None,
                common_data_type=type(cxt_data).__name__ if cxt_data is not None else None,
            )

        # Add application context information if requested
        if request.request_application_context and self._session_context is not None:
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

        # Return response (SDK will serialize it)
        return flamepy.TaskOutput(data=response)

    def on_session_leave(self):
        """Handle session leave."""
        self._session_leave_count += 1
        self._session_context = None


if __name__ == "__main__":
    flamepy.service.run(BasicTestService())
