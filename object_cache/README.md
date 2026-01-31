# Flame Object Cache

Apache Arrow-based object cache service for Flame distributed system.

## Overview

The flame-object-cache is an embedded library that provides persistent object storage using Apache Arrow Flight protocol and Arrow IPC format for efficient serialization. It runs as a dedicated thread within the flame-executor-manager service.

## Features

- **Persistent Storage**: Objects are stored on disk using Arrow IPC format and survive server restarts
- **Arrow Flight Protocol**: High-performance gRPC-based protocol for data transfer
- **Key-based Organization**: Objects organized by session ID (`session_id/object_id`)
- **In-memory Index**: Fast O(1) lookups with disk-backed persistence
- **Zero-copy Operations**: Leverages Arrow's efficient columnar format

## Configuration

### Server Configuration (`flame-cluster.yaml`)

```yaml
cache:
  endpoint: "grpc://127.0.0.1:9090"
  network_interface: "eth0"
  storage: "/var/lib/flame/cache"  # Optional: disk storage path
```

### Client Configuration (`flame.yaml`)

```yaml
clusters:
  - name: flame
    endpoint: "http://flame-session-manager:8080"
    cache:
      endpoint: "grpc://flame-object-cache:9090"
      storage: "/tmp/flame_cache"  # Optional: local storage path
```

### Environment Variables

- `FLAME_CACHE_STORAGE`: Override cache storage path
- `FLAME_CACHE_ENDPOINT`: Override cache endpoint

## Usage

### Using the Cache

The cache server is automatically started when the executor-manager starts. No separate startup command is needed.

### Python SDK

```python
from flamepy import put_object, get_object, ObjectRef

# Put an object
ref = put_object("session123", my_data)
print(f"Stored at: {ref.endpoint}/{ref.key}")

# Get an object
data = get_object(ref)

# Update an object
new_ref = update_object(ref, new_data)
```

## Storage Structure

```
/var/lib/flame/cache/
└── session_id/
    ├── object1.arrow
    ├── object2.arrow
    └── object3.arrow
```

Each object is stored as an Arrow IPC file with schema: `{version: UInt64, data: Binary}`

## API

The cache server implements the Arrow Flight protocol:

- **do_put**: Upload an object (returns ObjectRef in BSON format)
- **do_get**: Retrieve an object by key
- **get_flight_info**: Get metadata about an object
- **list_flights**: List all cached objects
- **do_action**: Perform cache operations (PUT, UPDATE, DELETE)

## Building

The cache is built as part of the executor-manager:

```bash
# Build the cache library (part of executor-manager build)
cargo build --package object_cache --release

# Or build the full executor-manager
cargo build --package executor_manager --release
```

## Running with Docker Compose

```bash
# Start all services (cache runs embedded in executor-manager)
docker compose up -d

# View cache logs (part of executor-manager logs)
docker compose logs flame-executor-manager | grep cache

# Stop services
docker compose down
```

## Implementation Details

- **Language**: Rust
- **Protocol**: Arrow Flight (gRPC-based)
- **Storage Format**: Arrow IPC
- **Async Runtime**: Tokio
- **Arrow Version**: 53 (compatible with tonic 0.12)

## Architecture

The object cache runs as a dedicated thread within the executor-manager process, providing:

- **Simplified deployment**: No separate service to manage
- **Better locality**: Cache runs alongside executors on the same node
- **Shared configuration**: Uses the same config file as executor-manager
- **Resource efficiency**: Lower overhead than separate service

## Limitations

- Version is always 0 (no version conflict detection)
- No automatic cache cleanup or eviction
- Single-node cache (no distributed coordination)
- No authentication/authorization
- Objects are per-session (no cross-session sharing)

## See Also

- Design Document: `docs/designs/RFE318-cache/FS.md`
- Python SDK Cache Module: `sdk/python/src/flamepy/core/cache.py`
