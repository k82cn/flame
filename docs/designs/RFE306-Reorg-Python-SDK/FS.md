# RFE306: Re-org Python SDK to include RL, Agent and Core

## Motivation

Currently, all APIs reside within the `flamepy` module, but they serve distinct purposes, such as reinforcement learning (RL), agent frameworks, and core functionality. To improve clarity and usability, it is preferable to organize the SDK into separate modules according to their specific purposes.

## Function Specification

The `flamepy` package will be organized into four distinct modules, each reflecting a key area of functionality within the Flame ecosystem. This modular structure will clarify the purpose of each component and enhance both usability and maintainability.

### `core` module

Similar to Rust SDK, the `core` module is dedicated to the fundamental concepts within Flame, such as Applications, Sessions, and Tasks. This module also encompasses the infrastructure necessary to deploy services that implement `FlameService`. At present, these capabilities are primarily located in `client.py`, `service.py`, and `types.py`—alongside their associated generated proto files.

### `cache` module

By default, Flame persists all data—including task inputs, outputs, and common data—via the `flame-session-manager`. This approach is effective for small-scale data. However, for use cases involving large or frequent data transfers, such as training workloads, persisting all data centrally can become inefficient or impractical. To address this, Flame provides a dedicated object cache service (`flame-object-cache`) using Apache Arrow Flight protocol, enabling more efficient handling of large or transient data with persistent storage. The `cache` module includes the client implementation of the cache, included in `cache.py` right now.

**Note:** As of RFE318, the cache is now a standalone service rather than embedded in `flame-executor-manager`.

### `agent` module

The `agent` module is used to build an AI Agent by Flame. It shares the client with `core` right now, and the service is defined in `instance.py` right now.

### `rl` module

The `rl` module is used to build RL for AI Agent. Its client is defined in `runner.py`, and the service is defined in `runpy.py` right now.

## Implementation Details

1. Migrate the following files into the new `core` module to consolidate core functionality:
   - `client.py`
   - `service.py`
   - `types.py`
   - `frontend_pb2.py`, `frontend_pb2_grpc.py`
   - `shim_pb2.py`, `shim_pb2_grpc.py`
   - `types_pb2.py`, `types_pb2_grpc.py`

2. Move `instance.py` into the new `agent` module, as it provides the service foundation for agent development.

3. Relocate both `runner.py` (client) and `runpy.py` (service) into the `rl` module, organizing all reinforcement learning features together.

4. Move `cache.py` into the `cache` module, centralizing cache-related client functionality.

## Test Cases

1. Update e2e and integrate test accordingly
2. Update examples accordingly