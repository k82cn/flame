# Flame API Reference

This document provides the API reference for Flame's gRPC services.

## Services Overview

Flame exposes three gRPC services:

| Service | Description | Proto File |
|---------|-------------|------------|
| [Frontend](frontend.md) | Client-facing API for sessions, tasks, and applications | `frontend.proto` |
| [Backend](backend.md) | Executor-facing API for node and executor management | `backend.proto` |
| [Instance](shim.md) | Application instance lifecycle management | `shim.proto` |

## Quick Links

- [Type Definitions](types.md) - Common message types used across all services
- [Frontend Service](frontend.md) - For client SDK developers
- [Backend Service](backend.md) - For executor/node developers
- [Instance Service](shim.md) - For application shim developers

## Package

All services and messages are defined in the `flame.v1` package.

```protobuf
package flame.v1;
```

## Authentication

Flame supports TLS for secure communication. Configure TLS certificates in your client configuration:

```yaml
contexts:
  - name: flame
    cluster:
      endpoint: "https://flame-session-manager:8080"
      tls:
        ca_file: "/etc/flame/certs/ca.crt"
```

## Error Handling

gRPC status codes are used for error reporting:

| Code | Description |
|------|-------------|
| `OK` | Operation succeeded |
| `NOT_FOUND` | Resource not found |
| `ALREADY_EXISTS` | Resource already exists |
| `INVALID_ARGUMENT` | Invalid request parameters |
| `FAILED_PRECONDITION` | Operation not allowed in current state |
| `INTERNAL` | Internal server error |

## Related Documentation

- [Flame Architecture](../README.md)
- [Quick Start Guide](../tutorials/)
- [Python SDK](../../sdk/python/README.md)
- [Rust SDK](../../sdk/rust/README.md)
