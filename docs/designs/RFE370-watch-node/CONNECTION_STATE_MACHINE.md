# NodeConnection State Machine

This document describes the connection state machine implementation for managing the lifecycle of node connections in the session manager.

## Overview

The `NodeConnection` represents a single connection to one executor manager node. Each node has exactly one connection managed by the `ConnectionManager` in the controller. The connection lifecycle is managed through the **State Pattern**, following the same design as `controller/nodes/` and `controller/executors/`.

## Architecture

```
model/connection/
└── mod.rs              # Types: NodeConnection, ConnectionState,
                        #        NodeConnectionPtr, NodeConnectionSender,
                        #        NodeConnectionReceiver, ConnectionCallbacks

controller/connections/
├── mod.rs              # ConnectionStates trait, from() factory
├── manager.rs          # ConnectionManager
├── connected.rs        # ConnectedState
├── draining.rs         # DrainingState
└── closed.rs           # ClosedState
```

### Design Patterns

1. **State Pattern** — Each connection state is a separate struct implementing the `ConnectionStates` trait
2. **Factory Pattern** — `from(conn_ptr)` creates the appropriate state handler based on current state
3. **Callback Pattern** — `ConnectionCallbacks` trait decouples state transitions from controller actions
4. **Channel Pattern** — `NodeConnectionSender` and `NodeConnectionReceiver` provide async-safe communication

## State Machine

```
                   ┌─────────────┐
                   │   (new)     │
                   └──────┬──────┘
                          │ connect()
                          ▼
             ┌────────────────────────┐
             │      Connected         │◄───────┐
             │  - can send/recv       │        │
             │  - can drain           │        │
             └───────────┬────────────┘        │
                         │ drain()             │
                         ▼                     │
             ┌────────────────────────┐        │
             │      Draining          │        │
             │  - drain timer running │────────┘ connect() [reconnect]
             │  - waiting for close   │
             └───────────┬────────────┘
                         │ close()
                         ▼
             ┌────────────────────────┐
             │       Closed           │
             │  - permanently closed  │
             │  - cannot recover      │
             └────────────────────────┘
             (removed from registry)
```

## States

### Connected

The node is connected and operational. Created directly when `ConnectionManager::connect()` is called.

**Valid operations:**
- `drain()` → transitions to Draining, starts drain timer
- `notify_executor()` → sends executor updates to the node via sender
- `connect()` → idempotent, returns current state (no error)

**Invalid operations:**
- `close()` → must drain first (error)

### Draining

The node has disconnected and the drain timer is running. This gives the node a chance to reconnect before being fully shut down.

**Valid operations:**
- `connect()` → reconnect before timeout, cancels drain timer, transitions to Connected
- `close()` → drain timer expired, transitions to Closed

**Invalid operations:**
- `drain()` → already draining (error)
- `notify_executor()` → no active connection (error)

### Closed

The connection has been permanently closed after the drain timeout expired. The connection is removed from the registry and cannot be recovered.

**Invalid operations:**
- `connect()` → permanently closed (error)
- `drain()` → already closed (error)
- `close()` → already closed (error)
- `notify_executor()` → no active connection (error)

## Components

### NodeConnection (model/connection/mod.rs)

Represents the connection data with internal async queue:

```rust
pub struct NodeConnection {
    pub node_name: String,
    queue: AsyncQueue<Executor>,  // Internal, not exposed
    pub state: ConnectionState,
    pub drain_cancel: Option<CancellationToken>,
}

impl NodeConnection {
    pub fn new(node_name: String) -> Self;
    pub fn sender(&self) -> NodeConnectionSender;
    pub fn receiver(&self) -> NodeConnectionReceiver;
    pub fn is_connected(&self) -> bool;
    pub fn state(&self) -> &ConnectionState;
}
```

### NodeConnectionSender / NodeConnectionReceiver

Cloneable handles for async-safe communication across await points:

```rust
#[derive(Clone)]
pub struct NodeConnectionSender {
    queue: AsyncQueue<Executor>,
    node_name: String,
}

impl NodeConnectionSender {
    pub async fn send(&self, executor: Executor) -> Result<(), FlameError>;
}

#[derive(Clone)]
pub struct NodeConnectionReceiver {
    queue: AsyncQueue<Executor>,
}

impl NodeConnectionReceiver {
    pub async fn recv(&self) -> Option<Executor>;
}
```

### ConnectionStates Trait (controller/connections/mod.rs)

Defines operations available for each state:

```rust
#[async_trait]
pub trait ConnectionStates: Send + Sync + 'static {
    async fn connect(&self) -> Result<ConnectionState, FlameError>;
    async fn drain(&self) -> Result<(), FlameError>;
    async fn close(&self) -> Result<(), FlameError>;
    async fn notify_executor(&self, executor: &Executor) -> Result<(), FlameError>;
    fn state(&self) -> ConnectionState;
}
```

### ConnectionCallbacks Trait (model/connection/mod.rs)

Callbacks invoked during state transitions:

```rust
#[async_trait]
pub trait ConnectionCallbacks: Send + Sync + 'static {
    /// Called when a node connects (new or reconnect).
    /// Note: Node registration in storage is done separately in controller.register_node().
    async fn on_connected(&self, node_name: &str) -> Result<(), FlameError>;
    async fn on_draining(&self, node_name: &str) -> Result<(), FlameError>;
    async fn on_closed(&self, node_name: &str) -> Result<(), FlameError>;
}
```

### ConnectionManager (controller/connections/manager.rs)

Manages all node connections:

```rust
pub struct ConnectionManager<C: ConnectionCallbacks> {
    connections: MutexPtr<HashMap<String, NodeConnectionPtr>>,
    drain_timeout: Duration,
    callbacks: Arc<C>,
}
```

**Key methods:**
- `connect()` — Register a new connection or reconnect an existing one, returns `(NodeConnectionSender, NodeConnectionReceiver)`
- `get_channel()` — Get sender/receiver handles for an existing connection
- `drain()` — Start draining a connection (begins drain timer)
- `notify_executor()` — Send executor state change to a node

## Connection Lifecycle

### 1. New Connection (register_node)

```
Client                    ConnectionManager              Controller
  │                              │                           │
  │──── RegisterNode ───────────>│                           │
  │                              │                           │── storage.register_node()
  │                              │<── connect() ─────────────│
  │                              │   (creates Connected)     │
  │                              │── on_connected() ────────>│── node → Ready
  │                              │   returns (sender, recv)  │
  │                              │                           │── send initial executors
  │<──── OK ─────────────────────│<─────────────────────────│
```

### 2. Watch Stream (watch_node)

```
Client                    ConnectionManager              Controller
  │                              │                           │
  │──── WatchNode ──────────────>│                           │
  │      (heartbeat)             │                           │
  │                              │<── get_channel() ────────│
  │                              │   returns (sender, recv)  │
  │                              │                           │
  │<──── Executor ──────────────│<── recv.recv() ───────────│
  │<──── Executor ──────────────│<── recv.recv() ───────────│
  │                              │                           │
  │      (stream loop)           │                           │
```

### 3. Disconnection with Reconnect

```
Client                    ConnectionManager              Controller
  │                              │                           │
  │──── stream closes ─────────>│                           │
  │                              │── drain() ──────────────>│
  │                              │   (starts timer)         │── on_draining()
  │                              │                           │── node → Unknown
  │                              │                           │
  │──── RegisterNode ───────────>│                           │
  │      (reconnect)             │<── connect() ─────────────│
  │                              │   (cancels timer)        │── on_connected()
  │                              │                           │── node → Ready
```

### 4. Disconnection with Timeout

```
Client                    ConnectionManager              Controller
  │                              │                           │
  │──── stream closes ─────────>│                           │
  │                              │── drain() ──────────────>│
  │                              │   (starts timer)         │── on_draining()
  │                              │                           │── node → Unknown
  │                              │                           │
  │                              │   (timer expires)        │
  │                              │── close() ──────────────>│
  │                              │   (removes connection)   │── on_closed()
  │                              │                           │── node → NotReady
  │                              │                           │── cleanup executors
```

## Configuration

- `DEFAULT_DRAIN_TIMEOUT_SECS`: 30 seconds — Time to wait for reconnection before closing

## Integration with Node State Machine

The connection callbacks integrate with the existing node state machine:

| Connection Event | Node State Transition |
|-----------------|----------------------|
| `on_connected`  | → Ready              |
| `on_draining`   | → Unknown            |
| `on_closed`     | → NotReady + cleanup |

## Error Handling

- Invalid state transitions return `FlameError::InvalidState`
- Lock failures return `FlameError::Internal`
- Network failures return `FlameError::Network`
- Missing connections return `FlameError::NotFound`
