# flmrun E2E Tests

## Overview

The flmrun application now has comprehensive end-to-end tests in the e2e test suite.

## Test Location

**File:** `/e2e/tests/test_flmrun.py`

## Test Cases

### 1. Application Registration
- **test_flmrun_application_registered**: Verifies that flmrun is registered as a default application with correct configuration

### 2. Basic Functionality
- **test_flmrun_sum_function**: Tests simple function execution with positional arguments
- **test_flmrun_no_args**: Tests function execution with no arguments
- **test_flmrun_kwargs**: Tests function execution with keyword arguments

### 3. Class Methods
- **test_flmrun_class_method**: Tests invoking methods on class instances (Calculator with add, multiply, subtract)
- **test_flmrun_stateful_class**: Tests stateful classes with instance variables (Counter)

### 4. Advanced Features
- **test_flmrun_lambda_function**: Tests lambda function execution
- **test_flmrun_multiple_tasks**: Tests running multiple tasks in the same session
- **test_flmrun_complex_return_types**: Tests functions returning dict, list, and tuple types

### 5. Validation
- **test_flmrun_runner_request_validation**: Tests RunnerRequest input validation (only one of args/kwargs/input_object)

## Running the Tests

### Run All E2E Tests
```bash
make e2e-ci
```

### Run Only flmrun Tests
```bash
docker compose exec flame-console \
  uv run -n pytest -vv e2e/tests/test_flmrun.py
```

### Run a Specific Test
```bash
docker compose exec flame-console \
  uv run -n pytest -vv e2e/tests/test_flmrun.py::test_flmrun_sum_function
```

### Run with More Verbose Output
```bash
docker compose exec flame-console \
  uv run -n pytest -vv -s e2e/tests/test_flmrun.py
```

## Test Coverage

The e2e tests cover:

✅ Application registration and configuration  
✅ Simple function execution (args, kwargs, no args)  
✅ Class method invocation  
✅ Stateful object handling  
✅ Lambda functions  
✅ Multiple tasks per session  
✅ Complex return types (dict, list, tuple)  
✅ Input validation  

## Test Structure

Each test follows the pattern:

1. **Setup**: Create execution object (function, class, lambda)
2. **Session Creation**: Create session with RunnerContext
3. **Execution**: Invoke tasks with RunnerRequest
4. **Verification**: Assert results match expectations
5. **Cleanup**: Close session

Example:
```python
def test_flmrun_sum_function():
    # Setup
    def sum(a: int, b: int) -> int:
        return a + b
    
    # Session creation
    ctx = flamepy.RunnerContext(execution_object=sum)
    ssn = flamepy.create_session("flmrun", ctx)
    
    try:
        # Execution
        req = flamepy.RunnerRequest(method=None, args=(1, 2))
        result = ssn.invoke(req)
        
        # Verification
        assert result == 3
    finally:
        # Cleanup
        ssn.close()
```

## Integration with CI/CD

The tests are integrated with the CI/CD pipeline and will run automatically on:
- Pull requests
- Commits to main branch
- Manual workflow triggers

## Related Files

- **Test File**: `/e2e/tests/test_flmrun.py`
- **Application Test**: `/e2e/tests/test_application.py` (updated to include flmrun)
- **Implementation**: `/sdk/python/src/flamepy/runpy.py`
- **Types**: `/sdk/python/src/flamepy/types.py` (RunnerContext, RunnerRequest)

## Future Test Additions

Potential areas for additional tests:

1. **Large Object Handling**: Test `input_object` with cached large objects
2. **Error Handling**: Test error cases (missing methods, invalid input)
3. **Concurrent Execution**: Test parallel task execution
4. **Performance**: Test with large number of tasks
5. **Package Download**: Test wheel package installation (Phase 2)

## Notes

- The linter warnings about unresolved imports (pytest, flamepy) are expected in the e2e environment
- Tests use the actual Flame infrastructure (session manager, executor manager)
- Tests are isolated - each creates and cleans up its own session
