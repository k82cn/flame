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

import asyncio
import os
import grpc
from abc import ABC, abstractmethod
from typing import Optional, Dict, Any, Union
from dataclasses import dataclass
import logging
from concurrent import futures

from .types import Shim, FlameError, FlameErrorCode
from .shim_pb2_grpc import InstanceServicer, add_InstanceServicer_to_server
from .types_pb2 import Result, EmptyRequest, TaskResult as TaskResultProto

logger = logging.getLogger(__name__)

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
    session_id: str
    application: ApplicationContext
    common_data: Optional[bytes] = None


@dataclass
class TaskContext:
    """Context for a task."""
    task_id: str
    session_id: str
    input: Optional[bytes] = None


@dataclass
class TaskOutput:
    """Output from a task."""
    data: Optional[bytes] = None


class FlameService:
    """Base class for implementing Flame services."""
    
    @abstractmethod
    async def on_session_enter(self, context: SessionContext):
        """
        Called when entering a session.
        
        Args:
            context: Session context information
            
        Returns:
            True if successful, False otherwise
        """
        pass
    
    @abstractmethod
    async def on_task_invoke(self, context: TaskContext) -> TaskOutput:
        """
        Called when a task is invoked.
        
        Args:
            context: Task context information
            
        Returns:
            Task output
        """
        pass
    
    @abstractmethod
    async def on_session_leave(self):
        """
        Called when leaving a session.
        
        Returns:
            True if successful, False otherwise
        """
        pass


class FlmInstanceServicer(InstanceServicer):
    """gRPC servicer implementation for GrpcShim service."""
    
    def __init__(self, service: FlameService):
        self._service = service
    
    async def OnSessionEnter(self, request, context):
        """Handle OnSessionEnter RPC call."""
        logger.debug("OnSessionEnter")
        try:

            logger.debug(f"OnSessionEnter request: {request}")

            # Convert protobuf request to SessionContext
            app_context = ApplicationContext(
                name=request.application.name,
                shim=Shim(request.application.shim),
                image=request.application.image if request.application.HasField("image") else None,
                command=request.application.command if request.application.HasField("command") else None
            )
            
            logger.debug(f"app_context: {app_context}")

            session_context = SessionContext(
                session_id=request.session_id,
                application=app_context,
                common_data=request.common_data if request.HasField("common_data") else None
            )

            logger.debug(f"session_context: {session_context}")
            
            # Call the service implementation
            logger.debug("Calling on_session_enter")
            await self._service.on_session_enter(session_context)
            logger.debug("on_session_enter returned")
            
            # Return result
            return Result(
                return_code=0,
            )
            
        except Exception as e:
            logger.error(f"Error in OnSessionEnter: {e}")
            return Result(
                return_code=-1,
                message=f"{str(e)}"
            )
    
    async def OnTaskInvoke(self, request, context):
        """Handle OnTaskInvoke RPC call."""
        logger.debug("OnTaskInvoke")
        try:
            # Convert protobuf request to TaskContext
            task_context = TaskContext(
                task_id=request.task_id,
                session_id=request.session_id,
                input=request.input if request.HasField("input") else None
            )

            logger.debug(f"task_context: {task_context}")
            
            # Call the service implementation
            logger.debug("Calling on_task_invoke")
            output = await self._service.on_task_invoke(task_context)
            logger.debug("on_task_invoke returned")

            # Return task output
            return TaskResultProto(
                return_code=0,
                output=output.data,
                message=None
            )
            
        except Exception as e:
            logger.error(f"Error in OnTaskInvoke: {e}")
            return TaskResultProto(
                return_code=-1,
                output=None,
                message=f"{str(e)}"
            )
    
    async def OnSessionLeave(self, request, context):
        """Handle OnSessionLeave RPC call."""
        logger.debug("OnSessionLeave")
        try:
            # Call the service implementation
            logger.debug("Calling on_session_leave")
            await self._service.on_session_leave()
            logger.debug("on_session_leave returned")
            # Return result
            return Result(
                return_code=0,
            )
            
        except Exception as e:
            logger.error(f"Error in OnSessionLeave: {e}")
            return Result(
                return_code=-1,
                message=f"{str(e)}"
            )

class FlmInstanceServer:
    """Server for gRPC shim services."""
    
    def __init__(self, service: FlameService):
        self._service = service
        self._server = None
    
    async def start(self):
        """Start the gRPC server."""
        try:
            # Create gRPC server
            self._server = grpc.aio.server()
            
            # Add servicer to server
            shim_servicer = FlmInstanceServicer(self._service)
            add_InstanceServicer_to_server(shim_servicer, self._server)

             # Listen on Unix socket
            socket_path = f"/tmp/flame/shim/fsi.sock"

            exec_id = os.getenv('FLAME_EXECUTOR_ID')
            if exec_id is not None:
                socket_path = f"/tmp/flame/shim/{exec_id}/fsi.sock"

            self._server.add_insecure_port(f"unix://{socket_path}")
            logger.debug(f"Flame Python instance service started on Unix socket: {socket_path}")

            # Start server
            await self._server.start()
            # Keep server running
            await self._server.wait_for_termination()
            
        except Exception as e:
            raise FlameError(
                FlameErrorCode.INTERNAL,
                f"Failed to start gRPC instance server: {str(e)}"
            )
    
    async def stop(self):
        """Stop the gRPC server."""
        if self._server:
            await self._server.stop(grace=5)
            logger.info("gRPC instance server stopped")


def run(service: FlameService):
    """
    Run a gRPC shim server.
    
    Args:
        service: The shim service implementation
    """

    server = FlmInstanceServer(service)
    asyncio.run(server.start())

