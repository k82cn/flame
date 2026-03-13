# Changelog - RFE370: WatchNode Streaming Service

All notable changes to this feature will be documented in this file.

## [Unreleased]

### Added
- Initial HLD document (`FS.md`) for WatchNode streaming service

### Changed
- Removed separate `backend.proto.proposed` file; proto definitions merged into `FS.md`
- Updated `FS.md` to follow standard design template
- **Removed polling mode and configuration**: Streaming is now mandatory for all clients.
- **Clarified state-based design**: HLD now explicitly states that raw `Executor` objects are returned (no `ExecutorAction` enum). Client derives actions from executor state.

### Design Decisions
- **Bidirectional streaming**: Chose bidirectional over server-streaming to allow client heartbeats
- **Heartbeat mechanism**: 5-second interval with 15-second timeout for connection health
- **Mandatory streaming**: Removed polling fallback to simplify architecture and enforce low latency
- **Reconnection strategy**: Exponential backoff for stream recovery
- **State-based notifications**: Server sends raw `Executor` objects instead of action-wrapped messages. This is simpler and more flexible - the client derives actions (create/update/delete) by comparing received state with local cache.

## [0.1.0] - 2026-03-10

### Added
- Created RFE370-watch-node design directory
- Initial design document with:
  - Motivation and background analysis
  - gRPC streaming API specification
  - Architecture diagrams (Mermaid)
  - Sequence diagrams for key flows
  - Data structure definitions
  - Use cases covering normal and edge cases
