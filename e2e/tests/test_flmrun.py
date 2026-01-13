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

import pytest
import flamepy
from e2e.helpers import (
    sum_func,
    multiply_func,
    greet_func,
    get_message_func,
    return_dict_func,
    return_list_func,
    return_tuple_func,
    square_func,
    Calculator,
    Counter,
)


# Application name for flmrun with e2e package installation
FLMRUN_E2E_APP = "flmrun-e2e"


@pytest.fixture(autouse=True)
def setup_flmrun_with_e2e():
    """
    Fixture to register a flmrun application with e2e package URL.
    
    This automatically registers a custom flmrun application that installs
    the e2e package when a session starts, making e2e modules available
    to the runner.
    """
    # Get the base flmrun application configuration
    flmrun = flamepy.get_application("flmrun")
    
    # Register a new application with e2e directory URL for package installation
    flamepy.register_application(
        FLMRUN_E2E_APP,
        flamepy.ApplicationAttributes(
            shim=flmrun.shim,
            url="file:///opt/e2e",  # e2e directory to be installed
            working_directory=flmrun.working_directory,
            command=flmrun.command,
            arguments=flmrun.arguments,
            description="Flmrun with e2e package installed",
        )
    )
    
    yield
    
    # Clean up: unregister the test application
    flamepy.unregister_application(FLMRUN_E2E_APP)


def test_flmrun_application_registered():
    """Test that flmrun is registered as a default application."""
    apps = flamepy.list_applications()
    app_names = [app.name for app in apps]
    assert FLMRUN_E2E_APP in app_names, f"{FLMRUN_E2E_APP} not found in applications: {app_names}"
    
    # Get the flmrun application and verify its configuration
    flmrun = flamepy.get_application(FLMRUN_E2E_APP)
    assert flmrun.name == FLMRUN_E2E_APP
    assert flmrun.shim == flamepy.Shim.Host
    assert flmrun.state == flamepy.ApplicationState.ENABLED
    assert flmrun.command == "/usr/bin/uv"
    assert flmrun.working_directory == "/usr/local/flame/work/flmrun"


def test_flmrun_sum_function():
    """Test Case 1: Run a simple sum function remotely."""
    # Create a session with RunnerContext and sum function
    ctx = flamepy.RunnerContext(execution_object=sum_func)
    ssn = flamepy.create_session(FLMRUN_E2E_APP, ctx)
    
    try:
        # Invoke the sum function remotely with positional arguments
        req = flamepy.RunnerRequest(method=None, args=(1, 2))
        result = ssn.invoke(req)
        
        # Verify the result
        assert result == 3, f"Expected 3, got {result}"
        
    finally:
        # Clean up
        ssn.close()


def test_flmrun_class_method():
    """Test Case 2: Run methods on a class instance."""
    # Create an instance of the calculator
    calc = Calculator()
    
    # Create a session with the calculator instance
    ctx = flamepy.RunnerContext(execution_object=calc)
    ssn = flamepy.create_session(FLMRUN_E2E_APP, ctx)
    
    try:
        # Test add method
        req = flamepy.RunnerRequest(method="add", args=(5, 3))
        result = ssn.invoke(req)
        assert result == 8, f"Expected 8, got {result}"
        
        # Test multiply method
        req = flamepy.RunnerRequest(method="multiply", args=(4, 7))
        result = ssn.invoke(req)
        assert result == 28, f"Expected 28, got {result}"
        
        # Test subtract method
        req = flamepy.RunnerRequest(method="subtract", args=(10, 3))
        result = ssn.invoke(req)
        assert result == 7, f"Expected 7, got {result}"
        
    finally:
        # Clean up
        ssn.close()


def test_flmrun_kwargs():
    """Test Case 3: Run a function with keyword arguments."""
    # Create a session with the function
    ctx = flamepy.RunnerContext(execution_object=greet_func)
    ssn = flamepy.create_session(FLMRUN_E2E_APP, ctx)
    
    try:
        # Test with keyword arguments
        req = flamepy.RunnerRequest(method=None, kwargs={"name": "World", "greeting": "Hi"})
        result = ssn.invoke(req)
        assert result == "Hi, World!", f"Expected 'Hi, World!', got {result}"
        
        # Test with partial keyword arguments (uses default)
        req = flamepy.RunnerRequest(method=None, kwargs={"name": "Python"})
        result = ssn.invoke(req)
        assert result == "Hello, Python!", f"Expected 'Hello, Python!', got {result}"
        
    finally:
        # Clean up
        ssn.close()


def test_flmrun_no_args():
    """Test Case 4: Run a function with no arguments."""
    # Create a session with the function
    ctx = flamepy.RunnerContext(execution_object=get_message_func)
    ssn = flamepy.create_session(FLMRUN_E2E_APP, ctx)
    
    try:
        # Invoke with no arguments (all fields None)
        req = flamepy.RunnerRequest(method=None)
        result = ssn.invoke(req)
        assert result == "Hello from flmrun!", f"Expected 'Hello from flmrun!', got {result}"
        
    finally:
        # Clean up
        ssn.close()


def test_flmrun_multiple_tasks():
    """Test Case 5: Run multiple tasks in the same session."""
    # Create a session with the function
    ctx = flamepy.RunnerContext(execution_object=multiply_func)
    ssn = flamepy.create_session(FLMRUN_E2E_APP, ctx)
    
    try:
        # Run multiple tasks with different inputs
        test_cases = [
            ((2, 3), 6),
            ((5, 4), 20),
            ((10, 10), 100),
            ((7, 8), 56),
        ]
        
        for args, expected in test_cases:
            req = flamepy.RunnerRequest(method=None, args=args)
            result = ssn.invoke(req)
            assert result == expected, f"multiply{args} expected {expected}, got {result}"
        
    finally:
        # Clean up
        ssn.close()


def test_flmrun_stateful_class():
    """Test Case 6: Run a stateful class with instance variables."""
    # Create an instance of the counter
    counter = Counter()
    
    # Create a session with the counter instance
    ctx = flamepy.RunnerContext(execution_object=counter)
    ssn = flamepy.create_session(FLMRUN_E2E_APP, ctx)
    
    try:
        # Test increment
        req = flamepy.RunnerRequest(method="increment")
        result = ssn.invoke(req)
        assert result == 1, f"Expected 1, got {result}"
        
        # Test increment again
        req = flamepy.RunnerRequest(method="increment")
        result = ssn.invoke(req)
        assert result == 2, f"Expected 2, got {result}"
        
        # Test add
        req = flamepy.RunnerRequest(method="add", args=(5,))
        result = ssn.invoke(req)
        assert result == 7, f"Expected 7, got {result}"
        
        # Test get_count
        req = flamepy.RunnerRequest(method="get_count")
        result = ssn.invoke(req)
        assert result == 7, f"Expected 7, got {result}"
        
    finally:
        # Clean up
        ssn.close()


def test_flmrun_lambda_function():
    """Test Case 7: Run a lambda function (using module-level function)."""
    # Use module-level function instead of lambda (lambdas can't be pickled)
    ctx = flamepy.RunnerContext(execution_object=square_func)
    ssn = flamepy.create_session(FLMRUN_E2E_APP, ctx)
    
    try:
        # Test with different values
        for x in [2, 5, 10, 15]:
            req = flamepy.RunnerRequest(method=None, args=(x,))
            result = ssn.invoke(req)
            expected = x * x
            assert result == expected, f"Expected {expected}, got {result}"
        
    finally:
        # Clean up
        ssn.close()


def test_flmrun_complex_return_types():
    """Test Case 8: Test functions that return complex types."""
    # Test dict return
    ctx = flamepy.RunnerContext(execution_object=return_dict_func)
    ssn = flamepy.create_session(FLMRUN_E2E_APP, ctx)
    try:
        req = flamepy.RunnerRequest(method=None, args=("test", 42))
        result = ssn.invoke(req)
        assert result == {"test": 42}, f"Expected {{'test': 42}}, got {result}"
    finally:
        ssn.close()
    
    # Test list return
    ctx = flamepy.RunnerContext(execution_object=return_list_func)
    ssn = flamepy.create_session(FLMRUN_E2E_APP, ctx)
    try:
        req = flamepy.RunnerRequest(method=None, args=(5,))
        result = ssn.invoke(req)
        assert result == [0, 1, 2, 3, 4], f"Expected [0, 1, 2, 3, 4], got {result}"
    finally:
        ssn.close()
    
    # Test tuple return
    ctx = flamepy.RunnerContext(execution_object=return_tuple_func)
    ssn = flamepy.create_session(FLMRUN_E2E_APP, ctx)
    try:
        req = flamepy.RunnerRequest(method=None, args=(123, "test"))
        result = ssn.invoke(req)
        assert result == (123, "test"), f"Expected (123, 'test'), got {result}"
    finally:
        ssn.close()


def test_flmrun_runner_request_validation():
    """Test Case 9: Test RunnerRequest validation."""
    # Test that only one input type can be set
    with pytest.raises(ValueError, match="Only one of"):
        flamepy.RunnerRequest(method=None, args=(1, 2), kwargs={"a": 1})
    
    with pytest.raises(ValueError, match="Only one of"):
        flamepy.RunnerRequest(
            method=None, 
            args=(1, 2), 
            input_object=flamepy.ObjectExpr(source=flamepy.DataSource.LOCAL)
        )
    
    with pytest.raises(ValueError, match="Only one of"):
        flamepy.RunnerRequest(
            method=None, 
            kwargs={"a": 1}, 
            input_object=flamepy.ObjectExpr(source=flamepy.DataSource.LOCAL)
        )
    
    # Test that valid single input types work
    req = flamepy.RunnerRequest(method=None, args=(1, 2))
    assert req.args == (1, 2)
    assert req.kwargs is None
    assert req.input_object is None
    
    req = flamepy.RunnerRequest(method=None, kwargs={"a": 1})
    assert req.args is None
    assert req.kwargs == {"a": 1}
    assert req.input_object is None
    
    req = flamepy.RunnerRequest(method=None)
    assert req.args is None
    assert req.kwargs is None
    assert req.input_object is None
