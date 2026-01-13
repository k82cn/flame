# flmrun - Flame Runner Service

The Flame Runner Service (`flmrun`) is a common Python application that enables remote execution of customized Python applications. It allows users to deploy and execute arbitrary Python functions and objects without needing to build custom container images.

## Features

- Execute arbitrary Python functions and callable objects remotely
- Session-wide shared execution context
- Support for positional arguments, keyword arguments, and large object inputs
- Minimal deployment overhead

## Usage

The `flmrun` application is automatically registered when the Flame Session Manager starts. It can be used to execute Python functions remotely:

```python
import flamepy

# Define a simple function
def sum(a: int, b: int) -> int:
    return a + b

# Create a session with the function as the execution object
ctx = flamepy.RunnerContext(execution_object=sum)
ssn = flamepy.create_session("flmrun", ctx)

# Invoke the function remotely
req = flamepy.RunnerRequest(method=None, args=(1, 2))
result = ssn.invoke(req)

# Get the result
print(result.get())  # Output: 3
```

## Architecture

The service is implemented using the `flamepy.runpy` module, which provides the `FlameRunpyService` class. This class implements the standard `FlameService` interface with the following methods:

- `on_session_enter`: Initializes the session environment
- `on_task_invoke`: Executes the requested method on the execution object
- `on_session_leave`: Cleans up resources at session end

## See Also

- [RFE284 Design Document](../docs/designs/RFE284-flmrun.md)
