# RFE309: Agent client

## Motivation

Currently, users build AI Agents by directly re-using the core APIs. However, this approach can be confusing and inconvenient for end users. In this feature, a dedicated Agent client will be introduced to streamline and simplify the process of building Agent clients.

## Function Specification

A new class, called `Agent`, will be added to the `flamepy.agent` module to provide a streamlined interface for building and interacting with agents.

### Initialization

There are two ways to manage the lifecycle of an Agent: using the constructor directly or as a context manager.

- **Direct Instantiation:**  
  When you create an agent using the constructor, you are responsible for explicitly closing it after use. This ensures that all resources are properly released.

  ```python
  agent = Agent("test", ctx)
  # ... use the agent ...
  agent.close()
  ```

- **Context Manager:**  
  By using the agent as a context manager, resource management is handled automatically. The agent is closed when execution leaves the context block.

  ```python
  with Agent("test", ctx) as agent:
      # ... use the agent ...
  ```

### Invoking the Agent

Use the `invoke()` method to interact with the agent. The request and response objects should exactly match the agent's entrypoint signature defined on the service side.

```python
resp = agent.invoke(req)
```

### Accessing the Agent Context

The current agent context—such as a running conversation or environment state—can be retrieved via the `context()` method.

```python
ctx = agent.context()
```

## Implementation detail

1. The `Agent` class is essentially a thin wrapper around `core.Session`, where the Agent's name corresponds to the application's name.
2. The agent's `context()` method returns the session's shared (common) data, accessed directly from `core.Session`.
3. The agent's `invoke()` method delegates its call to `core.Session.invoke`, passing through the invocation to the underlying session mechanism.

## Use Cases

1. Update e2e and integration test accordingly
2. Update example accordingly