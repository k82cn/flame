# RFE350: FlameSessionContext for Custom Session IDs

## 1. Motivation

**Background:**

Currently, when users create services via `Runner.service()`, the session ID is automatically generated using `short_name(app)` which produces a random suffix (e.g., `myapp-XyZ123`). This automatic generation has limitations:

1. **Traceability**: Users cannot easily correlate sessions with their business logic or external systems because session IDs are random and meaningless.

2. **Deterministic Testing**: When running tests or reproducible workflows, having predictable session IDs helps with debugging and log analysis.

3. **External Integration**: When integrating Flame with external systems (e.g., monitoring, logging, orchestration), having user-defined session IDs enables better correlation between Flame sessions and external identifiers.

4. **Application Context**: Users may want to pass additional application-level metadata (like `application_name`) alongside their execution objects for better observability.

**Target:**

This design introduces `FlameSessionContext`, a lightweight context class that users can attach to their execution objects to:

1. **Custom Session IDs**: Allow users to specify their own session IDs for better traceability and correlation.

2. **Application Metadata**: Provide a structured way to pass application-level context alongside execution objects.

3. **Opt-in Design**: Maintain backward compatibility - users who don't need custom session IDs continue to get auto-generated ones.

## 2. Function Specification

### Configuration

**FlameSessionContext Class:**

A new dataclass in `flamepy.runner.types` that encapsulates session context:

```python
@dataclass
class FlameSessionContext:
    """Context for customizing session creation in Runner.service().
    
    Users can attach this context to their execution objects via the
    `_flame_session_context` attribute to customize session behavior.
    
    Attributes:
        session_id: Custom session ID for the service. If not provided,
                   a random session ID will be generated.
        application_name: Optional application name for metadata/logging.
                         Defaults to the Runner's name if not specified.
    """
    session_id: Optional[str] = None
    application_name: Optional[str] = None
```

**Supported Execution Object Types:**

The `_flame_session_context` attribute is supported on all execution object types that `Runner.service()` accepts:

| Execution Object Type | How to Attach Context |
|-----------------------|-----------------------|
| **Class** | Define as class attribute |
| **Instance (Object)** | Set as instance attribute |
| **Function** | Set as function attribute |

**Usage Patterns:**

**1. With a Class:**

```python
from flamepy.runner import Runner, FlameSessionContext

class MyService:
    _flame_session_context = FlameSessionContext(
        session_id="my-custom-session-001",
        application_name="recommendation-engine"
    )
    
    def process(self, data):
        return data * 2

with Runner("my-app") as runner:
    # Session will be created with ID "my-custom-session-001"
    service = runner.service(MyService)
    result = service.process(42)
```

**2. With an Instance (Object):**

```python
from flamepy.runner import Runner, FlameSessionContext

class Counter:
    def __init__(self, initial=0):
        self._count = initial
    
    def add(self, value):
        self._count += value
        return self._count

# Create instance and attach context
counter = Counter(10)
counter._flame_session_context = FlameSessionContext(
    session_id="counter-session-001",
    application_name="stateful-counter"
)

with Runner("counter-app") as runner:
    # Session will be created with ID "counter-session-001"
    service = runner.service(counter, stateful=True)
    service.add(5)
```

**3. With a Function:**

```python
from flamepy.runner import Runner, FlameSessionContext

def compute(x, y):
    return x + y

# Attach context to function
compute._flame_session_context = FlameSessionContext(
    session_id="compute-batch-001",
    application_name="batch-processor"
)

with Runner("compute-app") as runner:
    # Session will be created with ID "compute-batch-001"
    service = runner.service(compute)
    result = service(1, 2)
```

### API

**FlameSessionContext Structure:**

```python
@dataclass
class FlameSessionContext:
    """Context for customizing Flame session creation.
    
    Attributes:
        session_id: Custom session identifier. If None, auto-generated.
        application_name: Optional application name for metadata.
    """
    session_id: Optional[str] = None
    application_name: Optional[str] = None
    
    def __post_init__(self) -> None:
        """Validate FlameSessionContext fields."""
        if self.session_id is not None:
            if not isinstance(self.session_id, str):
                raise ValueError(f"session_id must be a string, got {type(self.session_id)}")
            if len(self.session_id) == 0:
                raise ValueError("session_id cannot be empty string")
            if len(self.session_id) > 128:
                raise ValueError(f"session_id too long ({len(self.session_id)} > 128)")
        
        if self.application_name is not None:
            if not isinstance(self.application_name, str):
                raise ValueError(f"application_name must be a string, got {type(self.application_name)}")
```

**Detection Convention:**

- The special attribute `_flame_session_context` is used to attach context to execution objects
- This attribute must be an instance of `FlameSessionContext`
- Works with all execution object types: classes, instances (objects), and functions
- If the attribute exists but is not a `FlameSessionContext`, a warning is logged and it's ignored

### CLI

No CLI changes required. Existing `flmctl` commands will display custom session IDs the same way they display auto-generated ones.

### Other Interfaces

**SDK Interface Changes:**

- `RunnerService.__init__()`: Updated to check for `_flame_session_context` attribute
- `Runner.service()`: No signature change, behavior change only
- `RunnerContext`: No changes needed (session_id is not stored in RunnerContext)

### Scope

**In Scope:**
- Add `FlameSessionContext` dataclass to `flamepy.runner.types`
- Update `RunnerService.__init__()` to detect and use `_flame_session_context.session_id`
- Update `Runner.service()` to pass through detected context
- Export `FlameSessionContext` from `flamepy.runner` module
- Validation of session_id format (length, type)

**Out of Scope:**
- Session ID uniqueness enforcement (session manager already handles this)
- Session ID format restrictions beyond basic validation
- Passing FlameSessionContext through RPC (only session_id is used)
- Storing application_name in session metadata (future enhancement)

**Limitations:**
- Custom session IDs must still be unique - if a session with the same ID exists, session creation will fail
- `application_name` is currently for local logging/debugging only, not persisted
- Cannot change session ID after service creation

### Feature Interaction

**Related Features:**
- **Runner.service()**: The primary integration point for FlameSessionContext
- **RunnerService**: Creates sessions using the custom session_id if provided
- **Session Manager**: Receives custom session_id via `create_session()` (existing parameter)

**Updates Required:**

1. **types.py** (`sdk/python/src/flamepy/runner/types.py`):
   - Add `FlameSessionContext` dataclass
   - Export in `__all__`

2. **runner.py** (`sdk/python/src/flamepy/runner/runner.py`):
   - Update `RunnerService.__init__()` to:
     - Check if `execution_object` has `_flame_session_context` attribute
     - Validate that it's a `FlameSessionContext` instance
     - Use `_flame_session_context.session_id` instead of `short_name(app)` if provided

3. **__init__.py** (`sdk/python/src/flamepy/runner/__init__.py`):
   - Export `FlameSessionContext`

**Integration Points:**
- **Python SDK -> Session Manager**: Custom session_id passed via existing `create_session(session_id=...)` parameter
- **RunnerContext**: No changes needed - session_id is used during session creation, not serialized

**Compatibility:**
- Fully backward compatible - existing code without `_flame_session_context` continues to work
- Auto-generated session IDs remain the default behavior

**Breaking Changes:**
- None

## 3. Implementation Detail

### Architecture

```
+--------------------------------------------------------------+
|                  User Code                                    |
|  class MyService:                                             |
|      _flame_session_context = FlameSessionContext(...)        |
|      def process(self, data): ...                             |
+----------------------------+---------------------------------+
                             |
                             v runner.service(MyService)
+--------------------------------------------------------------+
|              Runner.service() (runner.py)                     |
|  - Receives execution_object                                  |
|  - Creates RunnerService(app, execution_object, ...)          |
+----------------------------+---------------------------------+
                             |
                             v
+--------------------------------------------------------------+
|            RunnerService.__init__() (runner.py)               |
|  - Check hasattr(execution_object, '_flame_session_context')  |
|  - Validate FlameSessionContext instance                      |
|  - Extract session_id (or use short_name(app) if None)        |
|  - Call create_session(session_id=custom_or_generated)        |
+----------------------------+---------------------------------+
                             |
                             v create_session(session_id=...)
+--------------------------------------------------------------+
|              Session Manager (Rust)                           |
|  - Receives session_id from SDK                               |
|  - Creates session with provided ID                           |
|  - No changes needed - already accepts custom session_id      |
+--------------------------------------------------------------+
```

### Components

**1. FlameSessionContext (Python SDK)**
- **Location**: `sdk/python/src/flamepy/runner/types.py`
- **Responsibilities**:
  - Store user-provided session configuration
  - Validate configuration fields
  - Provide structured context for session customization

**2. RunnerService (Python SDK)**
- **Location**: `sdk/python/src/flamepy/runner/runner.py`
- **Responsibilities**:
  - Detect `_flame_session_context` attribute on execution objects
  - Extract and validate session_id from context
  - Use custom session_id or fall back to auto-generation

### Data Structures

**FlameSessionContext:**

```python
@dataclass
class FlameSessionContext:
    """Context for customizing Flame session creation.
    
    Attributes:
        session_id: Custom session identifier. If provided, this ID will be
                   used when creating the session. Must be unique across
                   all active sessions. If None (default), a random ID
                   will be generated using short_name(app).
        application_name: Optional application name for logging and debugging.
                         Currently used for local context only.
    """
    session_id: Optional[str] = None
    application_name: Optional[str] = None
    
    def __post_init__(self) -> None:
        """Validate FlameSessionContext fields."""
        if self.session_id is not None:
            if not isinstance(self.session_id, str):
                raise ValueError(f"session_id must be a string, got {type(self.session_id)}")
            if len(self.session_id) == 0:
                raise ValueError("session_id cannot be empty string")
            if len(self.session_id) > 128:
                raise ValueError(f"session_id too long ({len(self.session_id)} chars, max 128)")
        
        if self.application_name is not None and not isinstance(self.application_name, str):
            raise ValueError(f"application_name must be a string, got {type(self.application_name)}")
```

### Algorithms

**Algorithm 1: Session ID Extraction in RunnerService**

```python
def __init__(self, app: str, execution_object: Any, stateful: bool = False, autoscale: bool = True):
    self._app = app
    self._execution_object = execution_object
    self._function_wrapper = None

    # NEW: Extract custom session_id from FlameSessionContext if present
    custom_session_id = None
    if hasattr(execution_object, '_flame_session_context'):
        ctx = getattr(execution_object, '_flame_session_context')
        if isinstance(ctx, FlameSessionContext):
            custom_session_id = ctx.session_id
            if ctx.application_name:
                logger.debug(f"FlameSessionContext application_name: {ctx.application_name}")
        else:
            logger.warning(
                f"_flame_session_context attribute found but is not FlameSessionContext "
                f"(got {type(ctx).__name__}), ignoring"
            )
    
    # Determine session_id: use custom if provided, otherwise generate
    session_id = custom_session_id if custom_session_id else short_name(app)
    
    # Create RunnerContext (unchanged)
    runner_context = RunnerContext(execution_object=execution_object, stateful=stateful, autoscale=autoscale)
    serialized_ctx = cloudpickle.dumps(runner_context, protocol=cloudpickle.DEFAULT_PROTOCOL)
    object_ref = put_object(session_id, serialized_ctx)
    common_data_bytes = object_ref.encode()
    
    # Create session with determined session_id
    self._session = create_session(
        application=app,
        common_data=common_data_bytes,
        session_id=session_id,  # Custom or generated
        min_instances=runner_context.min_instances,
        max_instances=runner_context.max_instances
    )

    logger.debug(
        f"Created RunnerService for app '{app}' with session '{self._session.id}' "
        f"(stateful={stateful}, autoscale={autoscale}, custom_session_id={custom_session_id is not None})"
    )

    self._generate_wrappers()
```

### System Considerations

**Performance:**
- Negligible overhead - single `hasattr()` and `isinstance()` check per service creation
- No runtime impact on task execution

**Scalability:**
- No scalability impact - session_id determination is local operation

**Reliability:**
- Session ID uniqueness is enforced by session manager (existing behavior)
- Duplicate session ID errors propagate to user with clear error message

**Resource Usage:**
- No additional memory usage beyond FlameSessionContext instance (small dataclass)

**Security:**
- No security implications - session_id is user-provided metadata only

**Observability:**
- Custom session IDs improve log correlation and debugging
- `application_name` provides additional context in logs

**Operational:**
- No deployment changes required
- Backward compatible with existing services

**Dependencies:**
- No new external dependencies
- Uses existing `short_name()` for fallback

## 4. Use Cases

### Basic Use Case: Custom Session ID

**Description:** User wants to use a meaningful session ID for better log correlation.

**Step-by-step workflow:**
1. User defines their service class with `_flame_session_context` attribute
2. User calls `runner.service(MyService)`
3. `RunnerService` detects the attribute and extracts `session_id`
4. Session is created with the custom ID
5. User can filter logs by their custom session ID

**Code Example:**

```python
from flamepy.runner import Runner, FlameSessionContext

class RecommendationService:
    # Attach custom session context
    _flame_session_context = FlameSessionContext(
        session_id="reco-batch-2024-02-08",
        application_name="recommendation-engine"
    )
    
    def recommend(self, user_id: int) -> list:
        # Business logic here
        return [1, 2, 3]

with Runner("reco-app") as runner:
    service = runner.service(RecommendationService)
    # Session ID will be "reco-batch-2024-02-08"
    results = service.recommend(42)
    print(results.get())
```

**Expected outcome:**
- Session created with ID "reco-batch-2024-02-08"
- Logs can be filtered using this predictable session ID
- `flmctl list -s` shows the custom session ID

### Advanced Use Case: Dynamic Session ID

**Description:** User generates session IDs programmatically based on workflow context.

**Step-by-step workflow:**

```python
from flamepy.runner import Runner, FlameSessionContext
import uuid

def create_service_with_trace(trace_id: str):
    """Factory function to create service with trace-correlated session."""
    
    class TrackedService:
        _flame_session_context = FlameSessionContext(
            session_id=f"trace-{trace_id}",
            application_name="tracked-processor"
        )
        
        def process(self, data):
            return data * 2
    
    return TrackedService

# In main workflow
trace_id = str(uuid.uuid4())[:8]
ServiceClass = create_service_with_trace(trace_id)

with Runner("tracked-app") as runner:
    service = runner.service(ServiceClass)
    # Session ID will be "trace-{trace_id}"
    result = service.process(21)
```

### Use Case: Backward Compatibility (No Context)

**Description:** Existing code without `_flame_session_context` continues to work.

```python
from flamepy.runner import Runner

class LegacyService:
    def compute(self, x):
        return x * 2

with Runner("legacy-app") as runner:
    # No _flame_session_context - auto-generated session ID
    service = runner.service(LegacyService)
    # Session ID will be auto-generated like "legacy-app-XyZ123"
    result = service.compute(5)
```

**Expected outcome:**
- Session ID auto-generated as before
- No changes required to existing code

### Use Case: Instance (Object) with Custom Session ID

**Description:** User creates an instance with initial state and attaches custom session context.

```python
from flamepy.runner import Runner, FlameSessionContext

class ModelServer:
    def __init__(self, model_path: str):
        self.model_path = model_path
        # Load model in constructor
        self.model = self._load_model(model_path)
    
    def _load_model(self, path):
        # Model loading logic
        return {"path": path}
    
    def predict(self, input_data):
        return {"prediction": 42, "model": self.model_path}

# Create instance with specific model
server = ModelServer("/models/v2.0")

# Attach session context to the instance
server._flame_session_context = FlameSessionContext(
    session_id="model-v2-inference",
    application_name="ml-inference"
)

with Runner("ml-app") as runner:
    # Session will use the pre-configured instance
    service = runner.service(server, stateful=True)
    result = service.predict([1, 2, 3])
    print(result.get())
```

**Expected outcome:**
- Session created with ID "model-v2-inference"
- Instance state (loaded model) is preserved
- Useful for stateful services with custom initialization

### Use Case: Function with Custom Session ID

**Description:** User attaches session context to a standalone function.

```python
from flamepy.runner import Runner, FlameSessionContext

def batch_process(items: list) -> list:
    """Process a batch of items."""
    return [item * 2 for item in items]

# Attach context to function
batch_process._flame_session_context = FlameSessionContext(
    session_id="batch-job-2024-02-08",
    application_name="batch-processor"
)

with Runner("batch-app") as runner:
    service = runner.service(batch_process)
    # Session ID will be "batch-job-2024-02-08"
    result = service([1, 2, 3, 4, 5])
    print(result.get())  # [2, 4, 6, 8, 10]
```

**Expected outcome:**
- Session created with ID "batch-job-2024-02-08"
- Function is executed remotely with custom session tracking

## 5. References

### Related Documents
- [RFE280 - Simplify the Python API of Flame](../FRE280-runner/RFE280-runner.md)
- [RFE323 - Enhanced Runner Service Configuration](../RFE323-runner-v2/FS.md)

### Implementation References
- `sdk/python/src/flamepy/runner/runner.py` - RunnerService implementation
- `sdk/python/src/flamepy/runner/types.py` - RunnerContext and RunnerRequest types
- `sdk/python/src/flamepy/core/client.py` - create_session() function
- `sdk/python/src/flamepy/core/types.py` - short_name() helper
