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

import logging
import pickle
import os
import subprocess
from urllib.parse import urlparse
from typing import Any

from .service import FlameService, SessionContext, TaskContext, TaskOutput
from .types import RunnerRequest, RunnerContext
from .cache import get_object

logger = logging.getLogger(__name__)


class FlameRunpyService(FlameService):
    """
    Common Python service for Flame that executes customized Python applications.
    
    This service allows users to execute arbitrary Python functions and objects
    remotely without building custom container images. It supports method invocation
    with various input types including positional args, keyword args, and large objects.
    """

    def __init__(self):
        """Initialize the FlameRunpyService."""
        self._ssn_ctx: SessionContext = None

    def _install_package_from_url(self, url: str) -> None:
        """
        Install a package from a URL.

        Supports file:// URLs pointing to either directories or package files.
        Directories are installed in editable mode (-e).

        Args:
            url: The package URL (e.g., file:///opt/my-package)

        Raises:
            FileNotFoundError: If the package path does not exist
            RuntimeError: If package installation fails
        """

        logger.info(f"Installing package from URL: {url}")

        # Parse the URL to extract the path
        parsed_url = urlparse(url)

        # Currently only support file:// scheme
        if parsed_url.scheme != 'file':
            logger.warning(f"Unsupported URL scheme: {parsed_url.scheme}, skipping package installation")
            return
        
        package_path = parsed_url.path
        logger.info(f"Package path: {package_path}")
        
        # Check if the path exists
        if not os.path.exists(package_path):
            logger.error(f"Package path does not exist: {package_path}")
            raise FileNotFoundError(f"Package path not found: {package_path}")
        
        # Determine if it's a directory or file
        if os.path.isdir(package_path):
            # Install directory in editable mode
            logger.info(f"Installing directory package in editable mode: {package_path}")
            install_args = ["uv", "pip", "install", "-e", package_path]
        else:
            # Install package file (wheel, etc.)
            logger.info(f"Installing package file: {package_path}")
            install_args = ["uv", "pip", "install", package_path]
        
        try:
            result = subprocess.run(
                install_args,
                capture_output=True,
                text=True,
                check=True
            )
            logger.info(f"Package installation output: {result.stdout}")
            if result.stderr:
                logger.warning(f"Package installation stderr: {result.stderr}")
            logger.info(f"Successfully installed package from: {package_path}")
        except subprocess.CalledProcessError as e:
            logger.error(f"Failed to install package: {e}")
            logger.error(f"stdout: {e.stdout}")
            logger.error(f"stderr: {e.stderr}")
            raise RuntimeError(f"Package installation failed: {e}")

    def on_session_enter(self, context: SessionContext) -> bool:
        """
        Handle session enter event.
        
        If the application URL is specified, install the package into the current .venv.
        
        Args:
            context: Session context containing application and session information
            
        Returns:
            True if successful, False otherwise
        """
        logger.info(f"Entering session: {context.session_id}")
        logger.debug(f"Application: {context.application.name}")
        logger.info(f"Application context: {context.application}")
        logger.info(f"Application URL value: {repr(context.application.url)}")

        # Store the session context for use in task invocation
        self._ssn_ctx = context
        
        # Install package if URL is specified
        if context.application.url:
            logger.info(f"Application URL specified: {context.application.url}")
            self._install_package_from_url(context.application.url)
        else:
            logger.info("No application URL specified, skipping package installation")
        
        logger.info("Session entered successfully")
        return True

    def on_task_invoke(self, context: TaskContext) -> TaskOutput:
        """
        Handle task invoke event.
        
        This method:
        1. Retrieves the execution object from session context
        2. Deserializes the RunnerRequest from task input
        3. Determines the invocation input (args, kwargs, or input_object)
        4. Executes the requested method on the execution object
        5. Returns the result as TaskOutput
        
        Args:
            context: Task context containing task ID, session ID, and input
            
        Returns:
            TaskOutput containing the result of the execution
            
        Raises:
            ValueError: If the input format is invalid or execution fails
        """
        logger.info(f"Invoking task: {context.task_id}")
        
        try:
            # Get the execution object from session context
            common_data = self._ssn_ctx.common_data()
            if not isinstance(common_data, RunnerContext):
                raise ValueError(
                    f"Expected RunnerContext in common_data, got {type(common_data)}"
                )
            
            execution_object = common_data.execution_object
            if execution_object is None:
                raise ValueError("Execution object is None in RunnerContext")
            
            logger.debug(f"Execution object type: {type(execution_object)}")
            
            # Get the RunnerRequest from task input
            # Note: context.input is already unpickled by the service layer
            if context.input is None:
                raise ValueError("Task input is None")
            
            request = context.input
            if not isinstance(request, RunnerRequest):
                raise ValueError(
                    f"Expected RunnerRequest in task input, got {type(request)}"
                )
            
            logger.debug(f"RunnerRequest: method={request.method}, "
                        f"has_args={request.args is not None}, "
                        f"has_kwargs={request.kwargs is not None}, "
                        f"has_input_object={request.input_object is not None}")
            
            # Determine the invocation input
            invoke_args = ()
            invoke_kwargs = {}
            
            if request.input_object is not None:
                # Unpickle the large object from cache
                obj_expr = get_object(request.input_object)
                if obj_expr is None or obj_expr.data is None:
                    raise ValueError("Failed to retrieve input_object from cache")
                
                input_data = pickle.loads(obj_expr.data)
                logger.debug(f"Loaded input_object from cache: {type(input_data)}")
                
                # The input_object should be used as args or kwargs
                # For simplicity, treat it as the first positional argument
                invoke_args = (input_data,)
            elif request.args is not None:
                invoke_args = request.args
            elif request.kwargs is not None:
                invoke_kwargs = request.kwargs
            
            # Execute the requested method
            if request.method is None:
                # The execution object itself is callable
                if not callable(execution_object):
                    raise ValueError(
                        f"Execution object is not callable: {type(execution_object)}"
                    )
                logger.debug(f"Invoking callable with args={invoke_args}, kwargs={invoke_kwargs}")
                result = execution_object(*invoke_args, **invoke_kwargs)
            else:
                # Invoke a specific method on the execution object
                if not hasattr(execution_object, request.method):
                    raise ValueError(
                        f"Execution object has no method '{request.method}'"
                    )
                
                method = getattr(execution_object, request.method)
                if not callable(method):
                    raise ValueError(
                        f"Attribute '{request.method}' is not callable"
                    )
                
                logger.debug(f"Invoking method '{request.method}' with args={invoke_args}, "
                           f"kwargs={invoke_kwargs}")
                result = method(*invoke_args, **invoke_kwargs)
            
            logger.info(f"Task {context.task_id} completed successfully")
            logger.debug(f"Result type: {type(result)}")
            
            # Return the result as TaskOutput
            return TaskOutput(data=result)
            
        except Exception as e:
            logger.error(f"Error in task {context.task_id}: {e}", exc_info=True)
            raise

    def on_session_leave(self) -> bool:
        """
        Handle session leave event.
        
        This method performs cleanup at session end. In the current implementation,
        there are no packages to uninstall. Future versions will handle cleanup of
        temporarily installed packages.
        
        Returns:
            True if successful, False otherwise
        """
        logger.info(f"Leaving session: {self._ssn_ctx.session_id if self._ssn_ctx else 'unknown'}")
        
        # Clean up session context
        self._ssn_ctx = None
        
        # Future implementation will:
        # 1. Uninstall any temporary packages that were installed
        # 2. Clean up any temporary files
        
        logger.info("Session left successfully")
        return True


def main():
    """Main entrypoint for the flamepy.runpy module."""
    from . import run
    
    logger.info("Starting FlameRunpyService")
    service = FlameRunpyService()
    run(service)


if __name__ == "__main__":
    main()
