# Flame Python SDK

Python SDK for the Flame, a distributed system for Agentic AI.

## Installation

```bash
pip install flamepy
```

## Quick Start

```python
import asyncio
import flamepy

async def main():
    # Create a session with the application, e.g. Agent
    session = await flamepy.create_session("flmping")
    
    # Create and run a task
    resp = await session.invoke(b"task input data")

    # Handle the output of task
    print(resp.output)

    # Close session
    await session.close()

if __name__ == "__main__":
    asyncio.run(main())
```

## API Reference

### Session

Represents a computing session with application, e.g. Agent, Tools.

```python
# Create a session
session = await flamepy.create_session("my-app")

# Close a session
await session.close()
```

### Task

Represents individual computing tasks within a session.

```python
# Create a task
task = await session.invoke(b"input data")

# Get task status
task = await session.get_task(task.id)

# Watch task progress
async for update in session.watch_task(task.id):
    print(f"Task state: {update.state}")
    if update.is_completed():
        break
```

## Error Handling

The SDK provides custom exception types for different error scenarios:

```python
from flamepy import FlameError, FlameErrorCode

try:
    session = await flamepy.create_session("flmping")
except FlameError as e:
    if e.code == FlameErrorCode.INVALID_CONFIG:
        print("Configuration error:", e.message)
    elif e.code == FlameErrorCode.INVALID_STATE:
        print("State error:", e.message)
```

## Development

To set up the development environment:

```bash
# Clone the repository
git clone https://github.com/xflops/flame.git
cd flame/sdk/python

# Install in development mode
pip install -e .[dev]

# Run tests
pytest

# Format code
black flamepy/
isort flamepy/

# Type checking
mypy flamepy/
``` 