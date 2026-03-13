# Test Plan: WatchNode Streaming

**Issue:** [#370](https://github.com/xflops/flame/issues/370)
**HLD Reference:** [FS.md](./FS.md)
**Author:** QA
**Date:** 2025-01-15
**Status:** Draft

## 1. Scope

### In Scope
- `WatchNode` bidirectional gRPC streaming API
- Node registration via stream
- Heartbeat mechanism and acknowledgements
- Executor state updates (initial sync and incremental)
- Stream reconnection and recovery
- Action derivation from executor state (CREATE/UPDATE/DELETE)
- Backend event loop and filtering
- Executor Manager stream client integration

### Out of Scope
- Changes to scheduling logic
- Polling-based `SyncNode` API (existing, not modified)
- mTLS authentication setup (infrastructure concern)
- Horizontal scaling of backend services (deployment concern)

## 2. Test Strategy

### Test Types
- [x] Unit Testing
- [x] Integration Testing
- [x] Edge Case Testing
- [x] Performance Testing
- [ ] Security Testing (deferred - mTLS is infrastructure)

### Test Environment
- **Environment:** Docker Compose (dev) / Local development
- **Components:** flame-session-manager, flame-executor-manager
- **Dependencies:** gRPC (Tonic), tokio runtime, storage layer

## 3. Use Case Coverage

| Use Case | Test Scenarios | Priority |
|----------|----------------|----------|
| UC1: Node Registration & Initial Sync | 5 scenarios | High |
| UC2: Executor State Updates | 6 scenarios | High |
| UC3: Heartbeat Mechanism | 4 scenarios | High |
| UC4: Stream Reconnection | 4 scenarios | Medium |
| UC5: Action Derivation | 5 scenarios | High |

## 4. Test Scenarios

### UC1: Node Registration & Initial Sync

| ID | Scenario | Type | Priority |
|----|----------|------|----------|
| TC-001 | Successful node registration with valid NodeRegistration message | Functional | High |
| TC-002 | Initial sync returns all existing executors for the node | Functional | High |
| TC-003 | Initial sync returns empty list when no executors exist | Functional | High |
| TC-004 | Registration with invalid/malformed node data | Negative | Medium |
| TC-005 | Multiple nodes register concurrently | Functional | Medium |

**TC-001: Successful Node Registration**
- **Preconditions:** Backend service running, no existing registration for node
- **Steps:**
  1. Executor Manager opens WatchNode stream
  2. Send NodeRegistration with valid Node data (name, spec)
  3. Observe response
- **Expected:** Stream established, backend accepts registration, node marked as connected

**TC-002: Initial Sync with Existing Executors**
- **Preconditions:** Node N1 has executors E1, E2 assigned in storage
- **Steps:**
  1. Open WatchNode stream for N1
  2. Send NodeRegistration
  3. Receive initial executor list
- **Expected:** Receive Executor messages for E1 and E2 with current state

**TC-003: Initial Sync with No Executors**
- **Preconditions:** Node N1 has no executors assigned
- **Steps:**
  1. Open WatchNode stream for N1
  2. Send NodeRegistration
- **Expected:** No Executor messages received, stream remains open

**TC-004: Invalid Registration Data**
- **Preconditions:** Backend service running
- **Steps:**
  1. Open WatchNode stream
  2. Send NodeRegistration with empty node name
- **Expected:** Stream returns error or closes gracefully

**TC-005: Concurrent Node Registrations**
- **Preconditions:** Backend service running
- **Steps:**
  1. Open WatchNode streams for N1, N2, N3 simultaneously
  2. Send NodeRegistration for each
- **Expected:** All nodes registered successfully, each receives own executor list

---

### UC2: Executor State Updates

| ID | Scenario | Type | Priority |
|----|----------|------|----------|
| TC-006 | Receive executor assignment update (new executor) | Functional | High |
| TC-007 | Receive executor state change (binding → bound) | Functional | High |
| TC-008 | Receive executor release notification | Functional | High |
| TC-009 | Updates filtered to correct node only | Functional | High |
| TC-010 | Multiple rapid state changes delivered in order | Functional | Medium |
| TC-011 | Large batch of executor updates | Performance | Medium |

**TC-006: New Executor Assignment**
- **Preconditions:** Node N1 connected via WatchNode stream
- **Steps:**
  1. Scheduler assigns new Executor E1 to N1
  2. Storage layer emits event
  3. Observe stream
- **Expected:** Executor message for E1 received on N1's stream

**TC-007: Executor State Transition**
- **Preconditions:** Node N1 connected, Executor E1 in "Binding" state
- **Steps:**
  1. E1 transitions to "Bound" state
  2. Observe stream
- **Expected:** Executor message with updated state received

**TC-008: Executor Release Notification**
- **Preconditions:** Node N1 connected, Executor E1 active
- **Steps:**
  1. E1 transitions to "ExecutorReleasing" or "ExecutorReleased"
  2. Observe stream
- **Expected:** Executor message received, client derives DELETE action

**TC-009: Node-Specific Filtering**
- **Preconditions:** N1 and N2 both connected via WatchNode
- **Steps:**
  1. Assign Executor E1 to N1 only
  2. Observe both streams
- **Expected:** N1 receives E1 update, N2 receives nothing

**TC-010: Ordered State Changes**
- **Preconditions:** Node N1 connected
- **Steps:**
  1. Trigger rapid state changes: E1 Pending → Binding → Bound
  2. Observe stream
- **Expected:** Messages received in correct order

**TC-011: Batch Executor Updates**
- **Preconditions:** Node N1 connected
- **Steps:**
  1. Assign 100 executors to N1 simultaneously
  2. Measure delivery time and completeness
- **Expected:** All 100 executor messages received within acceptable latency

---

### UC3: Heartbeat Mechanism

| ID | Scenario | Type | Priority |
|----|----------|------|----------|
| TC-012 | Successful heartbeat with acknowledgement | Functional | High |
| TC-013 | Heartbeat updates node status in backend | Functional | High |
| TC-014 | Missing heartbeat triggers timeout (stream_timeout) | Negative | High |
| TC-015 | Heartbeat with node status changes | Functional | Medium |

**TC-012: Heartbeat Acknowledgement**
- **Preconditions:** Node N1 registered via WatchNode
- **Steps:**
  1. Send NodeHeartbeat with node_name and status
  2. Observe response
- **Expected:** Acknowledgement message with timestamp received

**TC-013: Node Status Update via Heartbeat**
- **Preconditions:** Node N1 registered
- **Steps:**
  1. Send NodeHeartbeat with status (e.g., resource utilization)
  2. Query backend for node status
- **Expected:** Backend reflects updated node status

**TC-014: Heartbeat Timeout**
- **Preconditions:** Node N1 registered, stream_timeout = 30s
- **Steps:**
  1. Stop sending heartbeats
  2. Wait > 30 seconds
- **Expected:** Backend closes stream, node marked as disconnected

**TC-015: Heartbeat with Status Changes**
- **Preconditions:** Node N1 registered
- **Steps:**
  1. Send heartbeat with status "Healthy"
  2. Send heartbeat with status "Degraded"
- **Expected:** Both acknowledged, backend tracks status history

---

### UC4: Stream Reconnection

| ID | Scenario | Type | Priority |
|----|----------|------|----------|
| TC-016 | Client reconnects after network interruption | Functional | High |
| TC-017 | Full state sync on reconnection | Functional | High |
| TC-018 | Exponential backoff on repeated failures | Functional | Medium |
| TC-019 | Graceful server shutdown with stream drain | Functional | Medium |

**TC-016: Reconnection After Disconnect**
- **Preconditions:** Node N1 connected
- **Steps:**
  1. Simulate network partition (kill stream)
  2. Client detects disconnect
  3. Client reconnects
- **Expected:** New stream established, node re-registered

**TC-017: State Sync on Reconnection**
- **Preconditions:** N1 disconnected, executor changes occurred during disconnect
- **Steps:**
  1. E1 assigned to N1 while disconnected
  2. N1 reconnects and re-registers
- **Expected:** N1 receives E1 in initial sync

**TC-018: Exponential Backoff**
- **Preconditions:** Backend unavailable
- **Steps:**
  1. Client attempts to connect
  2. Connection fails repeatedly
  3. Observe retry intervals
- **Expected:** Retry intervals increase exponentially (e.g., 1s, 2s, 4s, 8s...)

**TC-019: Graceful Server Shutdown**
- **Preconditions:** Multiple nodes connected
- **Steps:**
  1. Initiate backend graceful shutdown
  2. Observe stream behavior
- **Expected:** Streams drained gracefully, clients notified

---

### UC5: Action Derivation

| ID | Scenario | Type | Priority |
|----|----------|------|----------|
| TC-020 | Derive CREATE action for new executor | Functional | High |
| TC-021 | Derive UPDATE action for known executor | Functional | High |
| TC-022 | Derive DELETE action for ExecutorReleasing state | Functional | High |
| TC-023 | Derive DELETE action for ExecutorReleased state | Functional | High |
| TC-024 | Handle unknown executor state gracefully | Edge Case | Medium |

**TC-020: CREATE Action Derivation**
- **Preconditions:** Node N1 connected, no local knowledge of E1
- **Steps:**
  1. Receive Executor message for E1 (first time)
  2. Client derives action
- **Expected:** Action = CREATE, client starts executor

**TC-021: UPDATE Action Derivation**
- **Preconditions:** Node N1 connected, E1 already known locally
- **Steps:**
  1. Receive Executor message for E1 with state change
  2. Client derives action
- **Expected:** Action = UPDATE, client updates executor state

**TC-022: DELETE Action for ExecutorReleasing**
- **Preconditions:** Node N1 connected, E1 known locally
- **Steps:**
  1. Receive Executor message for E1 with state = ExecutorReleasing
  2. Client derives action
- **Expected:** Action = DELETE, client initiates executor shutdown

**TC-023: DELETE Action for ExecutorReleased**
- **Preconditions:** Node N1 connected, E1 known locally
- **Steps:**
  1. Receive Executor message for E1 with state = ExecutorReleased
  2. Client derives action
- **Expected:** Action = DELETE, client removes executor

**TC-024: Unknown Executor State**
- **Preconditions:** Node N1 connected
- **Steps:**
  1. Receive Executor message with unexpected/unknown state
- **Expected:** Client handles gracefully (log warning, no crash)

---

## 5. Edge Cases & Negative Tests

| ID | Scenario | Type | Priority |
|----|----------|------|----------|
| TC-025 | Send heartbeat before registration | Negative | Medium |
| TC-026 | Send multiple registrations on same stream | Negative | Medium |
| TC-027 | Empty WatchNodeRequest message | Negative | Low |
| TC-028 | Very long node name (boundary) | Edge Case | Low |
| TC-029 | Stream with no activity (idle timeout) | Edge Case | Medium |
| TC-030 | Backend restart while streams active | Edge Case | High |

**TC-025: Heartbeat Before Registration**
- **Steps:** Open stream, send NodeHeartbeat without prior NodeRegistration
- **Expected:** Error response or stream closed

**TC-026: Duplicate Registration**
- **Steps:** Send NodeRegistration twice on same stream
- **Expected:** Second registration ignored or error returned

**TC-027: Empty Request**
- **Steps:** Send WatchNodeRequest with neither registration nor heartbeat set
- **Expected:** Error response, stream remains stable

**TC-028: Long Node Name**
- **Steps:** Register with node name of 1000+ characters
- **Expected:** Validation error or truncation with warning

**TC-029: Idle Stream**
- **Steps:** Register node, send no heartbeats, wait for stream_timeout
- **Expected:** Stream closed after timeout

**TC-030: Backend Restart**
- **Steps:** 
  1. Connect multiple nodes
  2. Restart backend service
  3. Observe client behavior
- **Expected:** Clients detect disconnect, reconnect with backoff

---

## 6. Performance Tests

| ID | Scenario | Metric | Target |
|----|----------|--------|--------|
| TC-031 | Latency: Executor update delivery | Time from storage event to client receipt | < 100ms |
| TC-032 | Throughput: Concurrent streams | Max active streams | 1000+ |
| TC-033 | Throughput: Events per second | Events/sec across all streams | 10,000+ |
| TC-034 | Memory: Per-stream overhead | Memory per active stream | < 1MB |
| TC-035 | Heartbeat latency | Round-trip time for heartbeat/ack | < 50ms |

**TC-031: Update Delivery Latency**
- **Setup:** Single node connected
- **Steps:**
  1. Trigger executor state change with timestamp
  2. Measure time until client receives update
- **Expected:** P99 latency < 100ms

**TC-032: Concurrent Stream Capacity**
- **Setup:** Backend with default configuration
- **Steps:**
  1. Open streams incrementally (100, 500, 1000, 2000)
  2. Monitor backend resource usage
  3. Verify all streams functional
- **Expected:** 1000+ concurrent streams without degradation

**TC-033: Event Throughput**
- **Setup:** 100 nodes connected
- **Steps:**
  1. Generate 100 executor changes per second
  2. Measure delivery rate across all streams
- **Expected:** All events delivered, no backpressure

**TC-034: Memory Overhead**
- **Setup:** Baseline memory measurement
- **Steps:**
  1. Open 100 streams
  2. Measure memory increase
  3. Calculate per-stream overhead
- **Expected:** < 1MB per stream

**TC-035: Heartbeat Latency**
- **Setup:** Node connected via WatchNode
- **Steps:**
  1. Send 1000 heartbeats
  2. Measure round-trip time for each
- **Expected:** P99 < 50ms

---

## 7. Test Data Requirements

| Data Type | Description | Source |
|-----------|-------------|--------|
| Node configurations | Valid node specs with various resource profiles | Test fixtures |
| Executor states | All possible executor state values | Enum from types.proto |
| Heartbeat payloads | NodeStatus with various resource metrics | Test fixtures |
| Large executor lists | 100+ executors for batch testing | Generated |

## 8. Dependencies & Risks

### Dependencies
- gRPC/Tonic framework functional
- Storage layer event emission working
- tokio runtime stable
- Docker Compose environment available

### Risks
| Risk | Impact | Mitigation |
|------|--------|------------|
| Stream instability under load | High | Performance testing early |
| Event ordering issues | Medium | Add sequence numbers if needed |
| Memory leaks in long-running streams | High | Monitor memory in perf tests |
| Flaky tests due to timing | Medium | Use deterministic waits, not sleeps |

## 9. Exit Criteria

- [ ] All high-priority test cases executed (TC-001 through TC-023)
- [ ] No critical or high severity defects open
- [ ] All use cases from HLD validated
- [ ] Performance targets met (TC-031 through TC-035)
- [ ] Edge cases documented and handled (TC-025 through TC-030)
- [ ] Test report submitted

## 10. Observability Verification

Verify the following metrics are emitted (per HLD):
- [ ] `watch_node_active_streams` - gauge of active connections
- [ ] `watch_node_events_sent` - counter of events pushed
- [ ] `watch_node_heartbeat_latency` - histogram of heartbeat RTT

Verify logging:
- [ ] Stream connect events logged
- [ ] Stream disconnect events logged
- [ ] Error conditions logged with context
