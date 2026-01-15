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
import os
import tarfile
import shutil
import inspect
from concurrent.futures import Future
from pathlib import Path
from typing import Any, List, Optional, Callable
from urllib.parse import urlparse
from functools import wraps

from .types import (
    FlameContext,
    FlameError,
    FlameErrorCode,
    ObjectRef,
    RunnerContext,
    RunnerRequest,
    ApplicationAttributes,
    Shim,
    RunnerServiceKind,
)
from .client import create_session, get_application, register_application, unregister_application
from .cache import get_object


logger = logging.getLogger(__name__)


class ObjectFuture:
    """Encapsulates a future that resolves to an ObjectRef.
    
    This class manages asynchronous and deferred computation results in runner services.
    The underlying future is expected to always yield an ObjectRef instance when resolved.
    
    Attributes:
        _future: A Future that will resolve to an ObjectRef
    """
    
    def __init__(self, future: Future):
        """Initialize an ObjectFuture.
        
        Args:
            future: A Future that resolves to an ObjectRef
        """
        self._future = future
    
    def ref(self) -> ObjectRef:
        """Get the ObjectRef by waiting for the future to complete.
        
        This method is primarily intended for internal use within the Flame SDK,
        providing direct access to the encapsulated object reference.
        
        Returns:
            The ObjectRef from the completed future
        """
        return self._future.result()
    
    def get(self) -> Any:
        """Retrieve the concrete object that this ObjectFuture represents.
        
        This method fetches the ObjectRef via the future, then uses cache.get_object
        to retrieve the actual underlying object.
        
        Returns:
            The deserialized object from the cache
        """
        object_ref = self._future.result()
        return get_object(object_ref)


class RunnerService:
    """Encapsulates an execution object for remote invocation within Flame.
    
    This class creates a session with the flamepy.runpy service and dynamically
    generates wrapper methods for all public methods of the execution object.
    Each wrapper submits tasks to the session and returns ObjectFuture instances.
    
    Attributes:
        _app: The name of the application registered in Flame
        _execution_object: The Python execution object being managed
        _session: The Flame session for task execution
    """
    
    def __init__(self, app: str, execution_object: Any, kind: Optional[RunnerServiceKind] = None):
        """Initialize a RunnerService.
        
        Args:
            app: The name of the application registered in Flame.
                 The associated service must be flamepy.runpy.
            execution_object: The Python execution object to be managed and
                             exposed as a remote service.
            kind: The runner service kind, if specified.
        """
        self._app = app
        self._execution_object = execution_object
        self._function_wrapper = None  # For callable functions
        
        # Create a session with flamepy.runpy service
        # The common_data is set using a RunnerContext that includes the execution_object
        runner_context = RunnerContext(execution_object=execution_object, kind=kind)
        self._session = create_session(application=app, common_data=runner_context)
        
        logger.info(f"Created RunnerService for app '{app}' with session '{self._session.id}'")
        
        # Generate wrapper methods for all public methods of the execution object
        self._generate_wrappers()
    
    def _generate_wrappers(self) -> None:
        """Generate wrapper functions for all public methods of the execution object.
        
        This method inspects the execution object and creates a wrapper for each
        public method (not starting with '_'). Each wrapper:
        - Converts ObjectFuture arguments to ObjectRef
        - Constructs a RunnerRequest
        - Submits a task via _session.run()
        - Returns an ObjectFuture
        """
        # Determine if execution_object is a function or has methods
        if callable(self._execution_object) and not inspect.isclass(self._execution_object):
            # It's a function, create a wrapper for direct invocation
            self._create_function_wrapper()
        else:
            # It's a class or instance, wrap all public methods
            self._create_method_wrappers()
    
    def _create_function_wrapper(self) -> None:
        """Create a wrapper for a callable execution object (function)."""
        def wrapper(*args, **kwargs):
            # Convert ObjectFuture arguments to ObjectRef
            converted_args = tuple(
                arg.ref() if isinstance(arg, ObjectFuture) else arg
                for arg in args
            )
            converted_kwargs = {
                key: value.ref() if isinstance(value, ObjectFuture) else value
                for key, value in kwargs.items()
            }
            
            # Create a RunnerRequest with method=None for direct callable invocation
            request = RunnerRequest(
                method=None,
                args=converted_args if converted_args else None,
                kwargs=converted_kwargs if converted_kwargs else None,
            )
            
            # Submit task and return ObjectFuture
            future = self._session.run(request)
            return ObjectFuture(future)
        
        # Store the wrapper so __call__ can use it
        self._function_wrapper = wrapper
        logger.debug(f"Created callable wrapper for function execution object")
    
    def _create_method_wrappers(self) -> None:
        """Create wrappers for all public methods of a class/instance."""
        # Get all public methods (not starting with '_')
        for attr_name in dir(self._execution_object):
            if attr_name.startswith('_'):
                continue
            
            attr = getattr(self._execution_object, attr_name)
            if not callable(attr):
                continue
            
            # Create a wrapper for this method
            wrapper = self._create_method_wrapper(attr_name)
            setattr(self, attr_name, wrapper)
            logger.debug(f"Created wrapper for method '{attr_name}'")
    
    def _create_method_wrapper(self, method_name: str) -> Callable:
        """Create a wrapper function for a specific method.
        
        Args:
            method_name: The name of the method to wrap
            
        Returns:
            A wrapper function that submits tasks and returns ObjectFuture
        """
        def wrapper(*args, **kwargs):
            # Convert ObjectFuture arguments to ObjectRef
            converted_args = tuple(
                arg.ref() if isinstance(arg, ObjectFuture) else arg
                for arg in args
            )
            converted_kwargs = {
                key: value.ref() if isinstance(value, ObjectFuture) else value
                for key, value in kwargs.items()
            }
            
            # Create a RunnerRequest for this method
            request = RunnerRequest(
                method=method_name,
                args=converted_args if converted_args else None,
                kwargs=converted_kwargs if converted_kwargs else None,
            )
            
            # Submit task and return ObjectFuture
            future = self._session.run(request)
            return ObjectFuture(future)
        
        return wrapper
    
    def __call__(self, *args, **kwargs) -> ObjectFuture:
        """Make RunnerService callable for function execution objects.
        
        This method allows calling the service directly when the execution object
        is a function (not a class or instance).
        
        Args:
            *args: Positional arguments to pass to the function
            **kwargs: Keyword arguments to pass to the function
            
        Returns:
            ObjectFuture that resolves to the function's result
            
        Raises:
            TypeError: If the execution object is not a callable function
        """
        if self._function_wrapper is None:
            raise TypeError(
                f"RunnerService for app '{self._app}' is not callable. "
                "The execution object is a class or instance, not a function. "
                "Call specific methods instead."
            )
        return self._function_wrapper(*args, **kwargs)
    
    def close(self) -> None:
        """Gracefully close the RunnerService and clean up resources.
        
        This closes the underlying session.
        """
        logger.info(f"Closing RunnerService for app '{self._app}'")
        self._session.close()


class Runner:
    """Context manager for managing lifecycle and deployment of Python packages in Flame.
    
    This class automates the packaging, uploading, registration, and cleanup of
    Python applications within Flame. It uses the context manager protocol to
    ensure proper setup and teardown.
    
    Attributes:
        _name: The name of the application/package
        _services: List of RunnerService instances created within this context
        _package_path: Path to the created package file
        _app_registered: Whether the application was successfully registered
    """
    
    def __init__(self, name: str):
        """Initialize a Runner.
        
        Args:
            name: The name of the application/package
        """
        self._name = name
        self._services: List[RunnerService] = []
        self._package_path: Optional[str] = None
        self._app_registered = False
        self._context = FlameContext()
        
        logger.info(f"Initialized Runner '{name}'")
    
    def __enter__(self) -> "Runner":
        """Enter the context manager and set up the application environment.
        
        Steps:
        1. Package the current working directory into a .tar.gz archive
        2. Upload the package to the storage location
        3. Retrieve the flmrun application template
        4. Register a new application with the package URL
        
        Returns:
            self for use in the with statement
            
        Raises:
            FlameError: If setup fails at any step
        """
        logger.info(f"Entering Runner context for '{self._name}'")
        
        # Check that package configuration is available
        if self._context.package is None:
            raise FlameError(
                FlameErrorCode.INVALID_CONFIG,
                "Package configuration is not set in FlameContext. "
                "Please configure the 'package' field in your flame.yaml."
            )
        
        # Step 1: Package the current working directory
        self._package_path = self._create_package()
        logger.info(f"Created package: {self._package_path}")
        
        # Step 2: Upload the package to storage
        storage_url = self._upload_package()
        logger.info(f"Uploaded package to: {storage_url}")
        
        # Step 3: Retrieve the flmrun application template
        try:
            flmrun_app = get_application("flmrun")
            logger.info(f"Retrieved flmrun application template")
        except Exception as e:
            # Clean up the package file
            if self._package_path and os.path.exists(self._package_path):
                os.remove(self._package_path)
            raise FlameError(
                FlameErrorCode.INTERNAL,
                f"Failed to get flmrun application template: {str(e)}"
            )
        
        # Step 4: Register the new application
        try:
            # Use /opt/{name} as working directory for the application
            working_directory = f"/opt/{self._name}"
            
            app_attrs = ApplicationAttributes(
                shim=flmrun_app.shim,
                image=flmrun_app.image,
                command=flmrun_app.command,
                description=f"Runner application: {self._name}",
                labels=flmrun_app.labels,
                arguments=flmrun_app.arguments,
                environments=flmrun_app.environments,
                working_directory=working_directory,
                max_instances=flmrun_app.max_instances,
                delay_release=flmrun_app.delay_release,
                schema=flmrun_app.schema,
                url=storage_url,
            )
            
            register_application(self._name, app_attrs)
            self._app_registered = True
            logger.info(f"Registered application '{self._name}' with working directory: {working_directory}")
        except Exception as e:
            # Clean up storage and package file
            self._cleanup_storage()
            if self._package_path and os.path.exists(self._package_path):
                os.remove(self._package_path)
            raise FlameError(
                FlameErrorCode.INTERNAL,
                f"Failed to register application: {str(e)}"
            )
        
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        """Exit the context manager and clean up resources.
        
        Steps:
        1. Close all RunnerService instances
        2. Unregister the application
        3. Delete the package from storage
        
        Args:
            exc_type: Exception type if an exception occurred
            exc_val: Exception value if an exception occurred
            exc_tb: Exception traceback if an exception occurred
        """
        logger.info(f"Exiting Runner context for '{self._name}'")
        
        # Step 1: Close all services
        for service in self._services:
            try:
                service.close()
            except Exception as e:
                logger.error(f"Error closing service: {e}", exc_info=True)
        
        # Step 2: Unregister the application
        if self._app_registered:
            try:
                unregister_application(self._name)
                logger.info(f"Unregistered application '{self._name}'")
            except Exception as e:
                logger.error(f"Error unregistering application: {e}", exc_info=True)
        
        # Step 3: Delete the package from storage
        self._cleanup_storage()
        
        # Clean up local package file
        if self._package_path and os.path.exists(self._package_path):
            try:
                os.remove(self._package_path)
                logger.info(f"Removed local package: {self._package_path}")
            except Exception as e:
                logger.error(f"Error removing local package: {e}", exc_info=True)
    
    def service(self, execution_object: Any, kind: Optional[RunnerServiceKind] = None) -> RunnerService:
        """Create a RunnerService for the given execution object.
        
        If execution_object is a class, it will be instantiated using its default
        constructor. The resulting RunnerService exposes all callable methods.
        
        Args:
            execution_object: A function, class, or class instance to expose as a service
            kind: The runner service kind. If None, defaults based on execution_object
            
        Returns:
            A RunnerService instance
        """
        
        # If it's a class, instantiate it
        if inspect.isclass(execution_object):
            logger.debug(f"Instantiating class {execution_object.__name__}")
            execution_object = execution_object()
        
        # Create the RunnerService
        runner_service = RunnerService(self._name, execution_object, kind=kind)
        self._services.append(runner_service)
        
        logger.info(f"Created service for execution object in Runner '{self._name}'")
        return runner_service
    
    def _create_package(self) -> str:
        """Create a .tar.gz package of the current working directory.
        
        Applies exclusion patterns from FlameContext.package.excludes.
        
        Returns:
            Path to the created package file
            
        Raises:
            FlameError: If package creation fails
        """
        cwd = os.getcwd()
        package_filename = f"{self._name}.tar.gz"
        package_path = os.path.join(cwd, package_filename)
        
        # Get exclusion patterns
        excludes = self._context.package.excludes if self._context.package else []
        
        logger.debug(f"Creating package with excludes: {excludes}")
        
        try:
            with tarfile.open(package_path, "w:gz") as tar:
                # Add files while respecting exclusions
                for item in os.listdir(cwd):
                    # Skip the package file itself
                    if item == package_filename:
                        continue
                    
                    # Check if item matches any exclusion pattern
                    if self._should_exclude(item, excludes):
                        logger.debug(f"Excluding: {item}")
                        continue
                    
                    item_path = os.path.join(cwd, item)
                    tar.add(item_path, arcname=item, recursive=True, 
                           filter=lambda tarinfo: None if self._should_exclude(tarinfo.name, excludes) else tarinfo)
            
            logger.info(f"Created package: {package_path}")
            return package_path
        
        except Exception as e:
            raise FlameError(
                FlameErrorCode.INTERNAL,
                f"Failed to create package: {str(e)}"
            )
    
    def _should_exclude(self, name: str, patterns: List[str]) -> bool:
        """Check if a file/directory name should be excluded.
        
        Args:
            name: The file or directory name
            patterns: List of exclusion patterns (supports wildcards)
            
        Returns:
            True if the name should be excluded
        """
        import fnmatch
        
        for pattern in patterns:
            if fnmatch.fnmatch(name, pattern) or fnmatch.fnmatch(os.path.basename(name), pattern):
                return True
        return False
    
    def _upload_package(self) -> str:
        """Upload the package to the storage location.
        
        Returns:
            The full URL to the uploaded package
            
        Raises:
            FlameError: If upload fails
        """
        if not self._package_path:
            raise FlameError(
                FlameErrorCode.INVALID_STATE,
                "Package path is not set"
            )
        
        storage_base = self._context.package.storage
        
        # Parse the storage URL
        parsed_url = urlparse(storage_base)
        
        if parsed_url.scheme != "file":
            raise FlameError(
                FlameErrorCode.INVALID_CONFIG,
                f"Unsupported storage scheme: {parsed_url.scheme}. Only file:// is supported."
            )
        
        # Get the storage directory path
        storage_dir = parsed_url.path
        
        # Ensure the storage directory exists
        if not os.path.exists(storage_dir):
            raise FlameError(
                FlameErrorCode.INVALID_CONFIG,
                f"Storage directory does not exist: {storage_dir}"
            )
        
        # Copy the package to storage
        dest_path = os.path.join(storage_dir, os.path.basename(self._package_path))
        
        # Check if package already exists
        if os.path.exists(dest_path):
            logger.info(f"Package already exists at {dest_path}, skipping upload")
        else:
            try:
                shutil.copy2(self._package_path, dest_path)
                logger.info(f"Copied package to {dest_path}")
            except Exception as e:
                raise FlameError(
                    FlameErrorCode.INTERNAL,
                    f"Failed to copy package to storage: {str(e)}"
                )
        
        # Return the full URL
        return f"file://{dest_path}"
    
    def _cleanup_storage(self) -> None:
        """Delete the package from storage."""
        if not self._package_path:
            return
        
        try:
            storage_base = self._context.package.storage
            parsed_url = urlparse(storage_base)
            
            if parsed_url.scheme == "file":
                storage_dir = parsed_url.path
                dest_path = os.path.join(storage_dir, os.path.basename(self._package_path))
                
                if os.path.exists(dest_path):
                    os.remove(dest_path)
                    logger.info(f"Removed package from storage: {dest_path}")
        
        except Exception as e:
            logger.error(f"Error cleaning up storage: {e}", exc_info=True)
