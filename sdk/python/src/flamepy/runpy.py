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
import sys
import site
import importlib
import tarfile
import zipfile
import shutil
from urllib.parse import urlparse
from typing import Any
from pathlib import Path

from .service import FlameService, SessionContext, TaskContext, TaskOutput
from .types import RunnerRequest, RunnerContext, ObjectRef
from .cache import get_object, put_object

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
    
    def _resolve_object_ref(self, value: Any) -> Any:
        """
        Resolve an ObjectRef to its actual value by fetching from cache.
        
        Args:
            value: The value to resolve. If it's an ObjectRef, fetch the data from cache.
                   Otherwise, return the value as is.
        
        Returns:
            The resolved value (unpickled if it was an ObjectRef).
            
        Raises:
            ValueError: If ObjectRef data cannot be retrieved from cache.
        """
        if isinstance(value, ObjectRef):
            logger.debug(f"Resolving ObjectRef: {value}")
            resolved_value = get_object(value)
            if resolved_value is None:
                raise ValueError(f"Failed to retrieve ObjectRef from cache: {value}")
            
            logger.debug(f"Resolved ObjectRef to type: {type(resolved_value)}")
            return resolved_value
        
        return value

    def _is_archive(self, file_path: str) -> bool:
        """
        Check if a file is an archive that needs to be extracted.
        
        Args:
            file_path: Path to the file
            
        Returns:
            True if the file is a supported archive format
        """
        archive_extensions = ['.tar.gz', '.tgz', '.tar.bz2', '.tbz2', '.tar.xz', '.txz', '.zip']
        return any(file_path.endswith(ext) for ext in archive_extensions)
    
    def _extract_archive(self, archive_path: str, extract_to: str) -> str:
        """
        Extract an archive to a directory.
        
        Args:
            archive_path: Path to the archive file
            extract_to: Directory to extract to
            
        Returns:
            Path to the extracted directory
            
        Raises:
            RuntimeError: If extraction fails
        """
        logger.info(f"Extracting archive: {archive_path} to {extract_to}")
        
        try:
            # Create extraction directory if it doesn't exist
            os.makedirs(extract_to, exist_ok=True)
            
            # Determine archive type and extract
            if archive_path.endswith('.zip'):
                with zipfile.ZipFile(archive_path, 'r') as zip_ref:
                    zip_ref.extractall(extract_to)
                logger.info(f"Extracted zip archive to {extract_to}")
            elif any(archive_path.endswith(ext) for ext in ['.tar.gz', '.tgz', '.tar.bz2', '.tbz2', '.tar.xz', '.txz', '.tar']):
                with tarfile.open(archive_path, 'r:*') as tar_ref:
                    tar_ref.extractall(extract_to)
                logger.info(f"Extracted tar archive to {extract_to}")
            else:
                raise RuntimeError(f"Unsupported archive format: {archive_path}")
            
            return extract_to
            
        except Exception as e:
            logger.error(f"Failed to extract archive: {e}", exc_info=True)
            raise RuntimeError(f"Archive extraction failed: {e}")
    
    def _install_package_from_url(self, url: str) -> None:
        """
        Install a package from a URL.

        Supports file:// URLs pointing to either directories or package files (archives).
        If the URL points to an archive file (.tar.gz, .zip, etc.), it will be extracted
        to the working directory first, then installed from the extracted directory.

        Args:
            url: The package URL (e.g., file:///opt/my-package.tar.gz or file:///opt/my-package)

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
        
        # If it's an archive file, extract it first
        install_path = package_path
        extracted_dir = None
        
        if os.path.isfile(package_path) and self._is_archive(package_path):
            logger.info(f"Package is an archive file, extracting...")
            
            # Get the working directory (default to /tmp if not set)
            working_dir = os.getcwd()
            extract_dir = os.path.join(working_dir, f"extracted_{os.path.basename(package_path).split('.')[0]}")
            
            # Extract the archive
            extracted_dir = self._extract_archive(package_path, extract_dir)
            
            # Use the extracted directory for installation
            install_path = extracted_dir
            logger.info(f"Will install from extracted directory: {install_path}")
        
        # Use sys.executable -m pip to install into the current virtual environment
        logger.info(f"Installing package: {install_path}")
        install_args = [sys.executable, "-m", "pip", "install", install_path]
        
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
            logger.info(f"Successfully installed package from: {install_path}")
            
            # Reload site packages to make the newly installed package available
            # This is necessary because the Python interpreter has already started
            logger.info("Reloading site packages to pick up newly installed package")
            importlib.reload(site)
            logger.info(f"Updated sys.path: {sys.path}")
            
        except subprocess.CalledProcessError as e:
            logger.error(f"Failed to install package: {e}")
            logger.error(f"stdout: {e.stdout}")
            logger.error(f"stderr: {e.stderr}")
            raise RuntimeError(f"Package installation failed: {e}")
        finally:
            # Clean up extracted directory if it was created
            # Note: We keep it for now as it might be needed during the session
            # Future enhancement could add cleanup in on_session_leave
            pass

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
        3. Resolves any ObjectRef instances in args/kwargs
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
                        f"has_kwargs={request.kwargs is not None}")
            
            # Resolve ObjectRef instances in args and kwargs
            invoke_args = ()
            invoke_kwargs = {}
            
            if request.args is not None:
                # Resolve any ObjectRef instances in args
                invoke_args = tuple(self._resolve_object_ref(arg) for arg in request.args)
                logger.debug(f"Resolved args: {len(invoke_args)} arguments")
            
            if request.kwargs is not None:
                # Resolve any ObjectRef instances in kwargs
                invoke_kwargs = {
                    key: self._resolve_object_ref(value)
                    for key, value in request.kwargs.items()
                }
                logger.debug(f"Resolved kwargs: {len(invoke_kwargs)} keyword arguments")
            
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
            
            # Update common data with the modified execution object to persist state
            # This is important for stateful classes where instance variables change
            logger.debug("Updating common data with modified execution object")
            updated_context = RunnerContext(execution_object=execution_object)
            self._ssn_ctx.update_common_data(updated_context)
            logger.debug("Common data updated successfully")

            # Put the result into cache and return ObjectRef
            # This enables efficient data transfer for large objects
            logger.debug("Putting result into cache")
            object_ref = put_object(context.session_id, result)
            logger.info(f"Result cached with ObjectRef: {object_ref}")
            
            # Return the ObjectRef as TaskOutput
            return TaskOutput(data=object_ref)
            
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
