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

import os
import time
import grpc
import pickle
from abc import ABC, abstractmethod
from typing import Optional, Dict, Any, Union
from dataclasses import dataclass
import logging
from concurrent import futures

from .types import Shim, FlameError, FlameErrorCode, ObjectExpr
from .cache import get_object, update_object
from .shim_pb2_grpc import InstanceServicer, add_InstanceServicer_to_server
from .types_pb2 import (
    Result,
    EmptyRequest,
    TaskResult as TaskResultProto,
)

logger = logging.getLogger(__name__)

FLAME_INSTANCE_ENDPOINT = "FLAME_INSTANCE_ENDPOINT"


class TraceFn:
    def __init__(self, name: str):
        self.name = name
        logger.debug(f"{name} Enter")

    def __del__(self):
        logger.debug(f"{self.name} Exit")


@dataclass
class ApplicationContext:
    """Context for an application."""

    name: str
    shim: Shim
    image: Optional[str] = None
    command: Optional[str] = None


@dataclass
class SessionContext:
    """Context for a session."""

    _data_expr: ObjectExpr

    session_id: str
    application: ApplicationContext

    def common_data(self) -> Any:
        """Get the common data."""
        self._data_expr = get_object(self._data_expr)
        return pickle.loads(self._data_expr.data) if self._data_expr is not None else None

    def update_common_data(self, data: Any):
        """Update the common data."""
        if self._data_expr is None:
            return

        self._data_expr.data = pickle.dumps(data, protocol=pickle.HIGHEST_PROTOCOL)
        self._data_expr = update_object(self._data_expr)


@dataclass
class TaskContext:
    """Context for a task."""

    task_id: str
    session_id: str
    input: Any


@dataclass
class TaskOutput:
    """Output from a task."""

    data: Any


class FlameService:
    """Base class for implementing Flame services."""

    @abstractmethod
    def on_session_enter(self, context: SessionContext):
        """
        Called when entering a session.

        Args:
            context: Session context information

        Returns:
            True if successful, False otherwise
        """
        pass

    @abstractmethod
    def on_task_invoke(self, context: TaskContext) -> TaskOutput:
        """
        Called when a task is invoked.

        Args:
            context: Task context information

        Returns:
            Task output
        """
        pass

    @abstractmethod
    def on_session_leave(self):
        """
        Called when leaving a session.

        Returns:
            True if successful, False otherwise
        """
        pass


class FlameInstanceServicer(InstanceServicer):
    """gRPC servicer implementation for GrpcShim service."""

    def __init__(self, service: FlameService):
        self._service = service

    def OnSessionEnter(self, request, context):
        """Handle OnSessionEnter RPC call."""
        _trace_fn = TraceFn("OnSessionEnter")

        try:

            logger.debug(f"OnSessionEnter request: {request}")

            # Convert protobuf request to SessionContext
            app_context = ApplicationContext(
                name=request.application.name,
                shim=Shim(request.application.shim),
                image=(request.application.image if request.application.HasField("image") else None),
                command=(request.application.command if request.application.HasField("command") else None),
            )

            logger.debug(f"app_context: {app_context}")

            common_data_expr = ObjectExpr.decode(request.common_data) if request.HasField("common_data") else None

            session_context = SessionContext(
                _data_expr=common_data_expr,
                session_id=request.session_id,
                application=app_context,
            )

            logger.debug(f"session_context: {session_context}")

            # Call the service implementation
            self._service.on_session_enter(session_context)
            logger.debug("on_session_enter completed successfully")

            # Return result
            return Result(
                return_code=0,
            )

        except Exception as e:
            logger.error(f"Error in OnSessionEnter: {e}")
            return Result(return_code=-1, message=f"{str(e)}")

    def OnTaskInvoke(self, request, context):
        """Handle OnTaskInvoke RPC call."""
        _trace_fn = TraceFn("OnTaskInvoke")

        try:
            # Convert protobuf request to TaskContext
            task_context = TaskContext(
                task_id=request.task_id,
                session_id=request.session_id,
                input=pickle.loads(request.input) if request.HasField("input") else None,
            )

            logger.debug(f"task_context: {task_context}")

            # Call the service implementation
            output = self._service.on_task_invoke(task_context)
            logger.debug("on_task_invoke completed successfully")

            output_data = None
            if output is not None and output.data is not None:
                output_data = pickle.dumps(output.data, protocol=pickle.HIGHEST_PROTOCOL)

            # Return task output
            return TaskResultProto(return_code=0, output=output_data, message=None)

        except Exception as e:
            logger.error(f"Error in OnTaskInvoke: {e}")
            return TaskResultProto(return_code=-1, output=None, message=f"{str(e)}")

    def OnSessionLeave(self, request, context):
        """Handle OnSessionLeave RPC call."""
        _trace_fn = TraceFn("OnSessionLeave")

        try:
            # Call the service implementation
            self._service.on_session_leave()
            logger.debug("on_session_leave completed successfully")

            # Return result
            return Result(
                return_code=0,
            )

        except Exception as e:
            logger.error(f"Error in OnSessionLeave: {e}")
            return Result(return_code=-1, message=f"{str(e)}")


class FlameInstanceServer:
    """Server for gRPC shim services."""

    def __init__(self, service: FlameService):
        self._service = service
        self._server = None

    def start(self):
        """Start the gRPC server."""
        try:
            # Create gRPC server
            self._server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))

            # Add servicer to server
            shim_servicer = FlameInstanceServicer(self._service)
            add_InstanceServicer_to_server(shim_servicer, self._server)

            # Listen on Unix socket
            endpoint = os.getenv(FLAME_INSTANCE_ENDPOINT)
            if endpoint is not None:
                self._server.add_insecure_port(f"unix://{endpoint}")
                logger.debug(f"Flame Python instance service started on Unix socket: {endpoint}")
            else:
                raise FlameError(FlameErrorCode.INVALID_CONFIG, "FLAME_INSTANCE_ENDPOINT not found")

            # Start server
            self._server.start()
            # Keep server running
            self._server.wait_for_termination()

        except Exception as e:
            raise FlameError(
                FlameErrorCode.INTERNAL,
                f"Failed to start gRPC instance server: {str(e)}",
            )

    def stop(self):
        """Stop the gRPC server."""
        if self._server:
            self._server.stop(grace=5)
            logger.info("gRPC instance server stopped")


def run(service: FlameService):
    """
    Run a gRPC shim server.

    Args:
        service: The shim service implementation
    """

    server = FlameInstanceServer(service)
    server.start()
