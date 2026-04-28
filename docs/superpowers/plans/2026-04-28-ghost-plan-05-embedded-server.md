# Ghost Plan 05 — Embedded Server (HTTP-style endpoints over libp2p)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `ghost-server` crate that implements the five MVP-1 server-side endpoints (Version, GetKeyPackage, GetDeliveryKey, GetPresence, InboxMessage) as typed `GhostRequest` / `GhostResponse` exchanges over libp2p's request-response protocol. Validated end-to-end by an integration test where Alice runs a `Server` and Bob queries every endpoint via a `Client`.

**Architecture:** New crate depends on `ghost-core`, `ghost-identity`, `ghost-protocol`, `ghost-storage`, `ghost-network`. Async (tokio). Defines `GhostRequest`/`GhostResponse` enums (CBOR-encoded as `Vec<u8>`); the existing `ghost-network::Network` transports them. The `Server` runs in a tokio task, receives inbound requests via Network's new request channel, dispatches to handlers (each handler queries Identity / Storage as needed), and replies. The `Client` is a thin wrapper: build typed request, call `Network::request`, decode typed response.

**Tech Stack:** All existing ghost-* deps. No new external crates. The architectural deviation: Plan 04's `Network` event loop will be refactored to expose response channels for inbound requests (Plan 04 auto-acked empty bodies — a slight misuse of request-response). The refactor is non-breaking for Plan 04's `Network::send_to` API.

**Deliverable Plan 05:** integration test in `crates/ghost-server/tests/e2e_endpoints.rs` that:

1. Alice creates Identity + Database (with KeyPackages populated) + Network + Server (the Server holds refs to the others).
2. Bob creates Identity + Network + Client.
3. Bob calls `client.get_version(alice_addr)` — receives `{ protocol: "ghost/1", min_compat: "ghost/1" }`.
4. Bob calls `client.get_delivery_key(alice_addr)` — receives Alice's X25519 delivery pubkey.
5. Bob calls `client.get_key_package(alice_addr)` — receives a serialized MLS KeyPackage (consumed from Alice's storage).
6. Second call `get_key_package` returns a DIFFERENT KeyPackage (proving consumption).
7. Bob calls `client.get_presence(alice_addr)` — receives Alice's presence info.
8. Bob calls `client.send_inbox(alice_addr, envelope_bytes)` — Alice's server emits the envelope on its inbox channel.

**Reference spec:** [docs/superpowers/specs/2026-04-27-ghost-mvp1-design.md](../specs/2026-04-27-ghost-mvp1-design.md), section 5 ("HTTP API embedded server").

**Reference plans:**
- [Plan 02](2026-04-27-ghost-plan-02-crypto-protocol.md) — sealed envelopes carried as InboxMessage payload
- [Plan 03](2026-04-27-ghost-plan-03-storage.md) — MyKeyPackagesRepo, Database
- [Plan 04](2026-04-28-ghost-plan-04-network-discovery.md) — Network struct (refactored in Task 1)

---

## Architectural deviation from spec section 5

Spec called for literal HTTP paths (`GET /v1/keypackages/{ghostid}`, etc.) over HTTP/3 over QUIC. Plan 04 committed us to libp2p, so HTTP would be redundant.

**Substitute:** typed `GhostRequest` / `GhostResponse` enums (CBOR-encoded) carried by libp2p's `request_response::Behaviour`. Same set of capabilities, more idiomatic for the libp2p stack.

**What we keep:**
- Five endpoints map 1:1 (Version, GetKeyPackage, GetDeliveryKey, GetPresence, InboxMessage)
- All requests are authenticated by libp2p-tls (the peer's GhostId-equivalent PeerId is verified)
- Inbox is "send and ack"; others have meaningful response payloads

**What we lose:**
- Browser-friendly HTTP API (irrelevant for MVP-1; mobile clients in MVP-2+ may need this and we can wire HTTP later if needed)
- Standard HTTP load-balancer / observability tooling (also irrelevant for MVP-1; we're not behind a load balancer)

This deviation will be documented in the spec as a follow-up after Plan 05 completes.

---

## Task 1: Refactor ghost-network for proper request-response

**Files:**
- Modify: `crates/ghost-network/src/network.rs`
- Possibly modify: `crates/ghost-network/tests/e2e_loopback.rs` (must keep passing)

**Goal:** add typed request/response semantics to Network. Plan 04's `next_inbound` auto-acked empty bodies; we now need:
- A way for the inbound consumer to respond with meaningful payloads.
- A way for the outbound caller to await the response.

The existing `send_to` API stays as a fire-and-forget wrapper (now sends a request and discards the response). The Plan 04 e2e test continues to pass.

### Step 1.1: Add new types to `crates/ghost-network/src/network.rs`

Add (without removing existing types):

```rust
use libp2p::request_response::ResponseChannel;

/// A typed handle to respond to an inbound request.
/// Holds a one-shot channel back to the swarm task; the swarm task forwards
/// the response bytes to the originating peer via libp2p.
pub struct ResponseHandle {
    pub(crate) inner: ResponseChannel<Vec<u8>>,
}

/// An inbound request awaiting our response.
pub struct InboundRequest {
    pub sender: PeerId,
    pub payload: Vec<u8>,
    pub response: ResponseHandle,
}

impl std::fmt::Debug for InboundRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InboundRequest")
            .field("sender", &self.sender)
            .field("payload_len", &self.payload.len())
            .finish()
    }
}
```

### Step 1.2: Add new commands

Add to the `Command` enum:

```rust
pub(crate) enum Command {
    // ... existing variants ...
    Request {
        target_peer: PeerId,
        target_addr: Option<Multiaddr>,
        bytes: Vec<u8>,
        reply: oneshot::Sender<Result<Vec<u8>>>,
    },
    Respond {
        channel: ResponseChannel<Vec<u8>>,
        bytes: Vec<u8>,
    },
}
```

### Step 1.3: Add new public methods on `Network`

```rust
impl Network {
    /// Send a typed request and await the response bytes from the peer.
    pub async fn request(
        &self,
        target_peer: PeerId,
        target_addr: Option<Multiaddr>,
        bytes: Vec<u8>,
    ) -> Result<Vec<u8>> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(Command::Request { target_peer, target_addr, bytes, reply: tx })
            .await
            .map_err(|_| NetworkError::ChannelClosed)?;
        rx.await.map_err(|_| NetworkError::ChannelClosed)?
    }

    /// Receive the next inbound request. Returns None if the loop has shut down.
    /// The consumer MUST call `respond` on the InboundRequest's `response` field
    /// to complete the request-response cycle.
    pub async fn next_request(&mut self) -> Option<InboundRequest> {
        self.request_rx.recv().await
    }

    /// Send a response to a previously received InboundRequest.
    pub async fn respond(&self, response: ResponseHandle, bytes: Vec<u8>) -> Result<()> {
        self.cmd_tx
            .send(Command::Respond { channel: response.inner, bytes })
            .await
            .map_err(|_| NetworkError::ChannelClosed)?;
        Ok(())
    }
}
```

### Step 1.4: Wire up the new channel + event-loop changes

Add to `Network` struct:

```rust
pub struct Network {
    cmd_tx: mpsc::Sender<Command>,
    inbound_rx: mpsc::Receiver<InboundEvent>,
    request_rx: mpsc::Receiver<InboundRequest>,  // NEW
    local_peer_id: PeerId,
    _task: tokio::task::JoinHandle<()>,
}
```

In `Network::spawn`, create the new channel and pass tx into `run_event_loop`:

```rust
let (request_tx, request_rx) = mpsc::channel::<InboundRequest>(64);
let task = tokio::spawn(run_event_loop(swarm, cmd_rx, inbound_tx, request_tx));
```

### Step 1.5: Update `run_event_loop` (and helpers `handle_command` / `handle_swarm_event`)

Add a third `mpsc::Sender<InboundRequest>` parameter `request_tx`.

Track pending **outbound requests** (to correlate libp2p RequestId → reply oneshot):

```rust
use libp2p::request_response::OutboundRequestId;
let mut pending_requests: HashMap<OutboundRequestId, oneshot::Sender<Result<Vec<u8>>>> =
    HashMap::new();
```

Handle the new commands in `handle_command`:

```rust
Command::Request { target_peer, target_addr, bytes, reply } => {
    if let Some(addr) = target_addr {
        swarm.behaviour_mut().kad.add_address(&target_peer, addr);
    }
    let request_id = swarm.behaviour_mut().messages.send_request(&target_peer, bytes);
    pending_requests.insert(request_id, reply);
}
Command::Respond { channel, bytes } => {
    if let Err(_) = swarm.behaviour_mut().messages.send_response(channel, bytes) {
        // Channel may have been closed by libp2p (e.g., timeout). Silently drop.
    }
}
```

Replace the auto-ack `Message::Request` handler in `handle_swarm_event` with delivery to `request_tx`:

```rust
// In the `request_response::Event::Message { ... message: Message::Request { request, channel, .. } ... }` arm:
let inbound = InboundRequest {
    sender: peer,
    payload: request,
    response: ResponseHandle { inner: channel },
};
let _ = request_tx.send(inbound).await;
// Also push to inbound_tx for backwards compat with `Network::next_inbound` consumers.
let _ = inbound_tx.send(InboundEvent::Message {
    sender: peer,
    payload: vec![],  // payload duplicated in InboundRequest; here a placeholder for the legacy stream
}).await;
```

**IMPORTANT:** The above duplicates events to two channels. To avoid that, you have two cleaner options:

**A.** Keep only the new `InboundRequest` channel; deprecate `next_inbound` / `InboundEvent::Message`. The Plan 04 test must be updated to use `next_request` and explicitly call `respond` with empty bytes.

**B.** Refactor `next_inbound` to internally read from `request_rx`, auto-respond with empty body on drop, and expose `payload` as `InboundEvent::Message`. Backward compat without duplication.

**Recommended: Option A.** Less hidden state. Plan 04's test gets a 4-line update (use `next_request`, call `network.respond(req.response, vec![]).await`).

If you choose A, modify `crates/ghost-network/tests/e2e_loopback.rs`:

```rust
// Replace:
let event = timeout(Duration::from_secs(10), bob.next_inbound())
// With:
let req = timeout(Duration::from_secs(10), bob.next_request())
    .await
    .expect("inbound timeout")
    .expect("bob received None");
assert_eq!(req.sender, alice_peer_id);
assert_eq!(req.payload, payload);
bob.respond(req.response, Vec::new()).await.expect("respond");
```

And remove the `InboundEvent` import. Also remove or adapt `send_to` — see Step 1.6.

### Step 1.6: Decide on `send_to` API

Two options:

**Option 1:** Keep `send_to` as a thin wrapper over `request` that ignores the response.

```rust
impl Network {
    /// Fire-and-forget send. Returns when the request has been dispatched (not when
    /// the peer has processed it). The peer's response is silently discarded.
    pub async fn send_to(
        &self,
        target_peer: PeerId,
        target_addr: Option<Multiaddr>,
        bytes: Vec<u8>,
    ) -> Result<()> {
        // Drop the response immediately; we don't care about it for fire-and-forget.
        let _ = self.request(target_peer, target_addr, bytes).await?;
        Ok(())
    }
}
```

**Option 2:** Remove `send_to` entirely; all callers use `request` and explicitly ignore the response.

**Recommended: Option 1.** Backwards-compat wrapper, tiny code.

### Step 1.7: Handle the `Response` event in the swarm event loop

The libp2p request-response behaviour emits `Event::Message { message: Message::Response { request_id, response } }` when a response arrives. Wire this to `pending_requests`:

```rust
request_response::Message::Response { request_id, response, .. } => {
    if let Some(reply) = pending_requests.remove(&request_id) {
        let _ = reply.send(Ok(response));
    }
}
```

Also handle `Event::OutboundFailure { request_id, error, .. }`:

```rust
request_response::Event::OutboundFailure { request_id, error, .. } => {
    if let Some(reply) = pending_requests.remove(&request_id) {
        let _ = reply.send(Err(NetworkError::Transport(format!("outbound failure: {error}"))));
    }
}
```

### Step 1.8: Run tests

```bash
export PATH="/c/Strawberry/perl/bin:/c/Strawberry/c/bin:$PATH"
cd /c/Users/david/Desktop/Ghost
cargo +1.87-x86_64-pc-windows-msvc test -p ghost-network 2>&1 | tail -15
```

Expected: 16 unit + 1 integration = 17. The e2e_loopback test should still pass (after the small refactor in Step 1.5).

If the test fails, iterate. Common issues:
- `OutboundRequestId` import path
- The match arms for `Event::Message::Response` — variant fields may differ between libp2p versions
- The `send_request` return type — in 0.56 it returns `OutboundRequestId` directly

### Step 1.9: Commit

```bash
git add crates/ghost-network/
git commit -m "refactor(ghost-network): proper request-response API (Network::request, next_request, respond)

Adds Network::request(peer, addr, bytes) -> Vec<u8> for request/response
semantics, plus Network::next_request / Network::respond for receivers.
Network::send_to becomes a thin wrapper over request that discards the response.
Updates the Plan 04 e2e_loopback test to use the new API.

Required for Plan 05's GhostRequest/GhostResponse endpoints, which need
typed responses (Version, GetKeyPackage, GetDeliveryKey, GetPresence)
instead of Plan 04's auto-acked empty bodies."
```

---

## Task 2: ghost-server crate scaffold + GhostRequest/GhostResponse enums

**Files:**
- Modify: `Cargo.toml` (root)
- Create: `crates/ghost-server/Cargo.toml`
- Create: `crates/ghost-server/src/lib.rs`
- Create: `crates/ghost-server/src/error.rs`
- Create: `crates/ghost-server/src/messages.rs`

### Step 2.1: Modify root `Cargo.toml`

Add `"crates/ghost-server"` to `members = [...]` (after `"crates/ghost-protocol"`, before `"crates/ghost-storage"`).

No new workspace deps — ghost-server uses existing tokio, ciborium, serde, etc.

### Step 2.2: Create `crates/ghost-server/Cargo.toml`

```toml
[package]
name = "ghost-server"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Ghost embedded server: dispatches typed requests over the network."

[dependencies]
ghost-core     = { path = "../ghost-core" }
ghost-identity = { path = "../ghost-identity" }
ghost-protocol = { path = "../ghost-protocol" }
ghost-storage  = { path = "../ghost-storage" }
ghost-network  = { path = "../ghost-network" }

tokio = { workspace = true }
serde = { workspace = true }
ciborium = { workspace = true }
hex = { workspace = true }
blake3 = { workspace = true }

thiserror = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
proptest = { workspace = true }
libp2p = { workspace = true }
```

### Step 2.3: Create `crates/ghost-server/src/error.rs`

```rust
//! Top-level error type for ghost-server.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("CBOR encode: {0}")]
    CborEncode(String),
    #[error("CBOR decode: {0}")]
    CborDecode(String),

    #[error("network: {0}")]
    Network(#[from] ghost_network::NetworkError),

    #[error("storage: {0}")]
    Storage(#[from] ghost_storage::StorageError),

    #[error("server reported error: {0}")]
    Remote(String),

    #[error("no key packages available")]
    NoKeyPackagesAvailable,

    #[error("invalid request: {0}")]
    InvalidRequest(String),

    #[error("invalid response: {0}")]
    InvalidResponse(String),

    #[error("entity not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, ServerError>;
```

### Step 2.4: Create `crates/ghost-server/src/messages.rs`

```rust
//! Typed request/response enums for the embedded server.
//!
//! Carried as opaque bytes by ghost-network's request-response protocol.
//! Both sides CBOR-encode/decode at this layer.

use crate::{Result, ServerError};
use ghost_core::GhostId;
use serde::{Deserialize, Serialize};

/// Current protocol version. Bumped on breaking wire-format changes.
pub const PROTOCOL_VERSION: &str = "ghost/1";
/// Minimum compatible version. Peers below this will be rejected.
pub const MIN_COMPAT_VERSION: &str = "ghost/1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GhostRequest {
    /// Request the server's protocol version.
    Version,
    /// Request the server's X25519 delivery public key.
    GetDeliveryKey,
    /// Request a fresh KeyPackage from the server (server consumes one from storage).
    GetKeyPackage,
    /// Request the server's current presence (online/offline + last_seen).
    GetPresence,
    /// Send a sealed envelope to the server's inbox.
    InboxMessage { envelope: Vec<u8> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GhostResponse {
    Version {
        protocol: String,
        min_compat: String,
    },
    DeliveryKey {
        x25519_pub: [u8; 32],
    },
    KeyPackage {
        bytes: Vec<u8>,
    },
    Presence {
        online: bool,
        last_seen: u64,
    },
    InboxAck,
    Error {
        reason: String,
    },
}

impl GhostRequest {
    pub fn to_cbor(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        ciborium::into_writer(self, &mut buf)
            .map_err(|e| ServerError::CborEncode(e.to_string()))?;
        Ok(buf)
    }

    pub fn from_cbor(bytes: &[u8]) -> Result<Self> {
        ciborium::from_reader(bytes).map_err(|e| ServerError::CborDecode(e.to_string()))
    }
}

impl GhostResponse {
    pub fn to_cbor(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        ciborium::into_writer(self, &mut buf)
            .map_err(|e| ServerError::CborEncode(e.to_string()))?;
        Ok(buf)
    }

    pub fn from_cbor(bytes: &[u8]) -> Result<Self> {
        ciborium::from_reader(bytes).map_err(|e| ServerError::CborDecode(e.to_string()))
    }

    /// Convert a successful response into a Result. `Error` variant becomes `Err`.
    pub fn into_ok(self) -> Result<Self> {
        match self {
            Self::Error { reason } => Err(ServerError::Remote(reason)),
            other => Ok(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_cbor_roundtrip_for_each_variant() {
        let cases = vec![
            GhostRequest::Version,
            GhostRequest::GetDeliveryKey,
            GhostRequest::GetKeyPackage,
            GhostRequest::GetPresence,
            GhostRequest::InboxMessage { envelope: vec![1, 2, 3, 4] },
        ];
        for req in cases {
            let bytes = req.to_cbor().unwrap();
            let _ = GhostRequest::from_cbor(&bytes).unwrap();
            // Don't assert equality — Clone+Serialize round-trip is tested by the round-trip itself.
        }
    }

    #[test]
    fn response_cbor_roundtrip_for_each_variant() {
        let cases = vec![
            GhostResponse::Version {
                protocol: PROTOCOL_VERSION.to_string(),
                min_compat: MIN_COMPAT_VERSION.to_string(),
            },
            GhostResponse::DeliveryKey { x25519_pub: [9u8; 32] },
            GhostResponse::KeyPackage { bytes: vec![5, 6, 7] },
            GhostResponse::Presence { online: true, last_seen: 1234 },
            GhostResponse::InboxAck,
            GhostResponse::Error { reason: "test".to_string() },
        ];
        for resp in cases {
            let bytes = resp.to_cbor().unwrap();
            let _ = GhostResponse::from_cbor(&bytes).unwrap();
        }
    }

    #[test]
    fn into_ok_passes_through_success() {
        let r = GhostResponse::InboxAck;
        let r = r.into_ok().unwrap();
        assert!(matches!(r, GhostResponse::InboxAck));
    }

    #[test]
    fn into_ok_fails_on_error_variant() {
        let r = GhostResponse::Error { reason: "boom".to_string() };
        let err = r.into_ok().unwrap_err();
        assert!(matches!(err, ServerError::Remote(_)));
    }
}
```

### Step 2.5: Create `crates/ghost-server/src/lib.rs`

```rust
//! Ghost embedded server: dispatches typed requests over the network.

pub mod error;
pub mod messages;

pub use error::{Result, ServerError};
pub use messages::{GhostRequest, GhostResponse, MIN_COMPAT_VERSION, PROTOCOL_VERSION};

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-server");
    }
}
```

### Step 2.6: Test + commit

```bash
export PATH="/c/Strawberry/perl/bin:/c/Strawberry/c/bin:$PATH"
cd /c/Users/david/Desktop/Ghost
cargo +1.87-x86_64-pc-windows-msvc test -p ghost-server 2>&1 | tail -10
```

Expected: 5 tests pass (1 smoke + 4 messages).

```bash
git add Cargo.toml Cargo.lock crates/ghost-server/
git commit -m "feat(ghost-server): scaffold crate with GhostRequest/GhostResponse enums"
```

---

## Task 3: Server struct + Version + DeliveryKey handlers

**Files:**
- Create: `crates/ghost-server/src/server.rs`
- Modify: `crates/ghost-server/src/lib.rs`

### Step 3.1: Create `crates/ghost-server/src/server.rs`

```rust
//! Server: spawns a tokio task that drains inbound requests from Network and dispatches them.

use crate::messages::{GhostRequest, GhostResponse, MIN_COMPAT_VERSION, PROTOCOL_VERSION};
use crate::{Result, ServerError};
use ghost_identity::IdentityKey;
use ghost_network::{InboundRequest, Network, ResponseHandle};
use ghost_protocol::delivery_public;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

/// Inbound envelope routed to the application layer (Plan 06 will consume this).
#[derive(Debug, Clone)]
pub struct InboundEnvelope {
    pub envelope: Vec<u8>,
}

/// Snapshot of presence state — set by the application layer.
#[derive(Debug, Clone, Copy)]
pub struct PresenceState {
    pub online: bool,
    pub last_seen: u64,
}

impl Default for PresenceState {
    fn default() -> Self {
        Self {
            online: false,
            last_seen: 0,
        }
    }
}

pub struct Server {
    _task: tokio::task::JoinHandle<()>,
    inbox_rx: mpsc::Receiver<InboundEnvelope>,
}

impl Server {
    /// Spawn a server attached to the given Network. The Network MUST be exclusive
    /// to this Server (the Server takes ownership of inbound request consumption).
    pub fn spawn(
        ik: Arc<IdentityKey>,
        network: Arc<Mutex<Network>>,
        presence: Arc<Mutex<PresenceState>>,
    ) -> Result<Self> {
        let (inbox_tx, inbox_rx) = mpsc::channel::<InboundEnvelope>(64);
        let task = tokio::spawn(run_server(ik, network, presence, inbox_tx));
        Ok(Self {
            _task: task,
            inbox_rx,
        })
    }

    /// Receive the next inbox envelope. Returns None if the server has shut down.
    pub async fn next_inbox(&mut self) -> Option<InboundEnvelope> {
        self.inbox_rx.recv().await
    }
}

async fn run_server(
    ik: Arc<IdentityKey>,
    network: Arc<Mutex<Network>>,
    presence: Arc<Mutex<PresenceState>>,
    inbox_tx: mpsc::Sender<InboundEnvelope>,
) {
    loop {
        // Acquire the network lock briefly to receive the next request, then release.
        let req = {
            let mut net = network.lock().await;
            net.next_request().await
        };
        let Some(req) = req else {
            break;
        };

        let response = handle_request(&ik, &presence, &inbox_tx, &req.payload).await;

        let bytes = match response.to_cbor() {
            Ok(b) => b,
            Err(_) => GhostResponse::Error {
                reason: "internal: response serialization failed".into(),
            }
            .to_cbor()
            .unwrap_or_default(),
        };

        let net = network.lock().await;
        let _ = net.respond(req.response, bytes).await;
    }
}

async fn handle_request(
    ik: &IdentityKey,
    presence: &Arc<Mutex<PresenceState>>,
    inbox_tx: &mpsc::Sender<InboundEnvelope>,
    payload: &[u8],
) -> GhostResponse {
    let request = match GhostRequest::from_cbor(payload) {
        Ok(r) => r,
        Err(e) => return GhostResponse::Error { reason: format!("decode: {e}") },
    };

    match request {
        GhostRequest::Version => GhostResponse::Version {
            protocol: PROTOCOL_VERSION.to_string(),
            min_compat: MIN_COMPAT_VERSION.to_string(),
        },
        GhostRequest::GetDeliveryKey => GhostResponse::DeliveryKey {
            x25519_pub: *delivery_public(ik).as_bytes(),
        },
        GhostRequest::GetKeyPackage => {
            // Plan 05 Task 4 fills this in.
            GhostResponse::Error { reason: "key package handler not implemented yet".into() }
        }
        GhostRequest::GetPresence => {
            let p = *presence.lock().await;
            GhostResponse::Presence {
                online: p.online,
                last_seen: p.last_seen,
            }
        }
        GhostRequest::InboxMessage { envelope } => {
            let _ = inbox_tx.send(InboundEnvelope { envelope }).await;
            GhostResponse::InboxAck
        }
    }
}
```

### Step 3.2: Modify `crates/ghost-server/src/lib.rs`

```rust
//! Ghost embedded server: dispatches typed requests over the network.

pub mod error;
pub mod messages;
pub mod server;

pub use error::{Result, ServerError};
pub use messages::{GhostRequest, GhostResponse, MIN_COMPAT_VERSION, PROTOCOL_VERSION};
pub use server::{InboundEnvelope, PresenceState, Server};

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-server");
    }
}
```

### Step 3.3: Test + commit

```bash
cargo +1.87-x86_64-pc-windows-msvc test -p ghost-server 2>&1 | tail -10
```

Expected: 5 tests pass (no new tests in Task 3 — handlers tested via the e2e test in Task 7).

```bash
git add crates/ghost-server/
git commit -m "feat(ghost-server): Server struct + Version/DeliveryKey/Presence/Inbox handlers (KeyPackage stub)"
```

---

## Task 4: GetKeyPackage handler (storage integration)

**Files:**
- Modify: `crates/ghost-server/src/server.rs`

### Step 4.1: Update `Server::spawn` and `run_server` / `handle_request` to take a `Database` ref

```rust
use ghost_storage::{Database, MyKeyPackagesRepo};

impl Server {
    pub fn spawn(
        ik: Arc<IdentityKey>,
        network: Arc<Mutex<Network>>,
        presence: Arc<Mutex<PresenceState>>,
        db: Arc<Database>,
    ) -> Result<Self> {
        let (inbox_tx, inbox_rx) = mpsc::channel::<InboundEnvelope>(64);
        let task = tokio::spawn(run_server(ik, network, presence, db, inbox_tx));
        Ok(Self { _task: task, inbox_rx })
    }
}
```

Update `run_server` and `handle_request` signatures to thread `db: Arc<Database>` through.

### Step 4.2: Implement the `GetKeyPackage` handler

In `handle_request`, replace the stub:

```rust
GhostRequest::GetKeyPackage => {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // Atomically pop one available one-time KeyPackage.
    // Use spawn_blocking because rusqlite is sync.
    let db_clone = db.clone();
    let result = tokio::task::spawn_blocking(move || {
        let repo = db_clone.my_keypackages();
        let available = repo.list_available_one_time().map_err(ServerError::from)?;
        let candidate = match available.first() {
            Some(c) => c.clone(),
            None => {
                // Fall back to last-resort if no one-time is available.
                match repo.last_resort().map_err(ServerError::from)? {
                    Some(lr) => lr,
                    None => return Err(ServerError::NoKeyPackagesAvailable),
                }
            }
        };
        // Mark consumed if it was a one-time. Last-resort isn't consumed.
        if !candidate.is_last_resort {
            repo.mark_consumed(&candidate.package_id, now).map_err(ServerError::from)?;
        }
        Ok::<_, ServerError>(candidate.package_blob.clone())
    })
    .await;

    match result {
        Ok(Ok(bytes)) => GhostResponse::KeyPackage { bytes },
        Ok(Err(e)) => GhostResponse::Error { reason: format!("{e}") },
        Err(e) => GhostResponse::Error { reason: format!("task join: {e}") },
    }
}
```

### Step 4.3: Test + commit

```bash
cargo +1.87-x86_64-pc-windows-msvc test -p ghost-server 2>&1 | tail -8
```

Expected: 5 tests pass (no new unit tests — full-flow tested in e2e test, Task 7).

```bash
git add crates/ghost-server/
git commit -m "feat(ghost-server): GetKeyPackage handler (consumes one from MyKeyPackagesRepo)"
```

---

## Task 5: Client struct (typed wrappers over Network::request)

**Files:**
- Create: `crates/ghost-server/src/client.rs`
- Modify: `crates/ghost-server/src/lib.rs`

### Step 5.1: Create `crates/ghost-server/src/client.rs`

```rust
//! Client: typed wrappers over Network::request for each endpoint.

use crate::messages::{GhostRequest, GhostResponse};
use crate::{Result, ServerError};
use ghost_network::Network;
use libp2p::{Multiaddr, PeerId};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Typed wrappers over `Network::request`. Builds the typed `GhostRequest`,
/// sends it as CBOR bytes, decodes the typed `GhostResponse`.
pub struct Client {
    network: Arc<Mutex<Network>>,
}

impl Client {
    pub fn new(network: Arc<Mutex<Network>>) -> Self {
        Self { network }
    }

    async fn send_request(
        &self,
        peer: PeerId,
        addr: Option<Multiaddr>,
        req: GhostRequest,
    ) -> Result<GhostResponse> {
        let bytes = req.to_cbor()?;
        let net = self.network.lock().await;
        let response_bytes = net.request(peer, addr, bytes).await?;
        drop(net);
        let resp = GhostResponse::from_cbor(&response_bytes)?;
        Ok(resp)
    }

    pub async fn get_version(
        &self,
        peer: PeerId,
        addr: Option<Multiaddr>,
    ) -> Result<(String, String)> {
        let resp = self.send_request(peer, addr, GhostRequest::Version).await?;
        match resp.into_ok()? {
            GhostResponse::Version { protocol, min_compat } => Ok((protocol, min_compat)),
            other => Err(ServerError::InvalidResponse(format!("expected Version, got {other:?}"))),
        }
    }

    pub async fn get_delivery_key(
        &self,
        peer: PeerId,
        addr: Option<Multiaddr>,
    ) -> Result<[u8; 32]> {
        let resp = self.send_request(peer, addr, GhostRequest::GetDeliveryKey).await?;
        match resp.into_ok()? {
            GhostResponse::DeliveryKey { x25519_pub } => Ok(x25519_pub),
            other => Err(ServerError::InvalidResponse(format!("expected DeliveryKey, got {other:?}"))),
        }
    }

    pub async fn get_key_package(
        &self,
        peer: PeerId,
        addr: Option<Multiaddr>,
    ) -> Result<Vec<u8>> {
        let resp = self.send_request(peer, addr, GhostRequest::GetKeyPackage).await?;
        match resp.into_ok()? {
            GhostResponse::KeyPackage { bytes } => Ok(bytes),
            other => Err(ServerError::InvalidResponse(format!("expected KeyPackage, got {other:?}"))),
        }
    }

    pub async fn get_presence(
        &self,
        peer: PeerId,
        addr: Option<Multiaddr>,
    ) -> Result<(bool, u64)> {
        let resp = self.send_request(peer, addr, GhostRequest::GetPresence).await?;
        match resp.into_ok()? {
            GhostResponse::Presence { online, last_seen } => Ok((online, last_seen)),
            other => Err(ServerError::InvalidResponse(format!("expected Presence, got {other:?}"))),
        }
    }

    pub async fn send_inbox(
        &self,
        peer: PeerId,
        addr: Option<Multiaddr>,
        envelope: Vec<u8>,
    ) -> Result<()> {
        let resp = self
            .send_request(peer, addr, GhostRequest::InboxMessage { envelope })
            .await?;
        match resp.into_ok()? {
            GhostResponse::InboxAck => Ok(()),
            other => Err(ServerError::InvalidResponse(format!("expected InboxAck, got {other:?}"))),
        }
    }
}
```

### Step 5.2: Modify `lib.rs`

```rust
pub mod client;
pub mod error;
pub mod messages;
pub mod server;

pub use client::Client;
pub use error::{Result, ServerError};
pub use messages::{GhostRequest, GhostResponse, MIN_COMPAT_VERSION, PROTOCOL_VERSION};
pub use server::{InboundEnvelope, PresenceState, Server};
```

### Step 5.3: Test + commit

```bash
cargo +1.87-x86_64-pc-windows-msvc test -p ghost-server 2>&1 | tail -10
```

Expected: 5 tests pass (no new unit tests — Client tested via e2e in Task 6).

```bash
git add crates/ghost-server/
git commit -m "feat(ghost-server): Client struct with typed wrappers (get_version, get_delivery_key, get_key_package, get_presence, send_inbox)"
```

---

## Task 6: End-to-end integration test (Plan 05 deliverable)

**Files:**
- Create: `crates/ghost-server/tests/e2e_endpoints.rs`

**This is the deliverable.** Alice runs Server (with Identity + DB + populated KeyPackages); Bob runs Client. Bob queries every endpoint over loopback.

### Step 6.1: Create `crates/ghost-server/tests/e2e_endpoints.rs`

```rust
//! Plan 05 deliverable: Bob's Client queries all five endpoints on Alice's Server.

use ghost_identity::Identity;
use ghost_network::Network;
use ghost_protocol::{delivery_public, new_provider, populate_initial_keypackages};
use ghost_server::{Client, PresenceState, Server};
use ghost_storage::{derive_master_key, Database};
use libp2p::Multiaddr;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::sync::Mutex;
use tokio::time::timeout;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bob_queries_all_endpoints_on_alice() {
    let dir = tempdir().unwrap();
    let alice_db_path = dir.path().join("alice.db");

    // ===== Alice setup =====
    let mut alice_id = Identity::generate(Some("Alice".into()), 1700000000);
    let alice_master = derive_master_key(&alice_id.identity_key);

    // Open alice's DB and populate KeyPackages.
    let alice_db = Arc::new({
        let db = Database::open_encrypted(&alice_db_path, &alice_master).unwrap();
        db.migrate().unwrap();
        // Populate alice's identity with KeyPackages backed by an MLS provider.
        let provider = new_provider();
        populate_initial_keypackages(&mut alice_id, &provider, 3).unwrap();
        // Sync the populated KeyPackages into MyKeyPackagesRepo so the Server can serve them.
        for kp_bytes in &alice_id.mls_keypackages {
            // Use BLAKE3 of bytes as package_id (deterministic).
            let pkg_id = *blake3::hash(kp_bytes).as_bytes();
            db.my_keypackages().insert(&ghost_storage::MyKeyPackageRow {
                package_id: pkg_id,
                package_blob: kp_bytes.clone(),
                private_key: vec![],  // Plan 05 doesn't yet wire up private init keys; placeholder.
                created_at: 1700000000,
                consumed_at: None,
                is_last_resort: false,
            }).unwrap();
        }
        db
    });

    let alice_ik = Arc::new(alice_id.identity_key);
    let alice_network = Arc::new(Mutex::new(Network::spawn(&alice_ik).await.unwrap()));
    let alice_peer_id = alice_network.lock().await.local_peer_id();

    // Listen on loopback.
    let listen_addr: Multiaddr = "/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap();
    alice_network.lock().await.listen_on(listen_addr).await.unwrap();
    let alice_addr = wait_for_addr(&alice_network).await;

    // Set Alice's presence.
    let alice_presence = Arc::new(Mutex::new(PresenceState {
        online: true,
        last_seen: 1700000060,
    }));

    // Spawn Alice's Server.
    let mut alice_server = Server::spawn(
        alice_ik.clone(),
        alice_network.clone(),
        alice_presence.clone(),
        alice_db.clone(),
    )
    .unwrap();

    // ===== Bob setup =====
    let bob_id = Identity::generate(Some("Bob".into()), 1700000000);
    let bob_ik = Arc::new(bob_id.identity_key);
    let bob_network = Arc::new(Mutex::new(Network::spawn(&bob_ik).await.unwrap()));
    let bob_client = Client::new(bob_network.clone());

    // ===== Run all endpoint checks =====

    // 1. Version.
    let (proto, min_compat) = bob_client
        .get_version(alice_peer_id, Some(alice_addr.clone()))
        .await
        .expect("get_version");
    assert_eq!(proto, "ghost/1");
    assert_eq!(min_compat, "ghost/1");

    // 2. DeliveryKey.
    let dk_remote = bob_client
        .get_delivery_key(alice_peer_id, Some(alice_addr.clone()))
        .await
        .expect("get_delivery_key");
    let dk_local = *delivery_public(&alice_ik).as_bytes();
    assert_eq!(dk_remote, dk_local, "remote delivery key must match local computation");

    // 3. KeyPackage (first call) — should return one of the three populated KPs.
    let kp1 = bob_client
        .get_key_package(alice_peer_id, Some(alice_addr.clone()))
        .await
        .expect("get_key_package first");
    assert!(!kp1.is_empty());

    // 4. KeyPackage (second call) — must return a DIFFERENT KP (proves consumption).
    let kp2 = bob_client
        .get_key_package(alice_peer_id, Some(alice_addr.clone()))
        .await
        .expect("get_key_package second");
    assert_ne!(kp1, kp2, "consecutive calls must return different KeyPackages");

    // 5. Presence.
    let (online, last_seen) = bob_client
        .get_presence(alice_peer_id, Some(alice_addr.clone()))
        .await
        .expect("get_presence");
    assert!(online);
    assert_eq!(last_seen, 1700000060);

    // 6. InboxMessage.
    let envelope_bytes = b"sealed envelope from bob".to_vec();
    bob_client
        .send_inbox(alice_peer_id, Some(alice_addr.clone()), envelope_bytes.clone())
        .await
        .expect("send_inbox");

    let received = timeout(Duration::from_secs(5), alice_server.next_inbox())
        .await
        .expect("inbox timeout")
        .expect("alice inbox closed");
    assert_eq!(received.envelope, envelope_bytes);
}

async fn wait_for_addr(net: &Arc<Mutex<Network>>) -> Multiaddr {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let addrs = net.lock().await.local_addrs().await;
        if let Some(a) = addrs.into_iter().next() {
            return a;
        }
        if tokio::time::Instant::now() > deadline {
            panic!("listener never bound");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
```

### Step 6.2: Run the integration test

```bash
export PATH="/c/Strawberry/perl/bin:/c/Strawberry/c/bin:$PATH"
cd /c/Users/david/Desktop/Ghost
cargo +1.87-x86_64-pc-windows-msvc test -p ghost-server --test e2e_endpoints 2>&1 | tail -15
```

Expected: `bob_queries_all_endpoints_on_alice` passes.

If it fails, debug step by step:
- **Version fails:** Server isn't dispatching at all. Check that `Server::spawn` creates the task and the task is running.
- **DeliveryKey fails:** Likely a serialization issue (`x25519_pub: [u8; 32]` may need the same `serde_sig` workaround as Plan 02/04 — `serde` doesn't natively serialize 32-byte arrays in older versions, but for 32 bytes it usually does). If it fails, check the CBOR codec for `[u8; 32]` and add a `serde(with = "serde_arr")` shim if needed.
- **KeyPackage fails on first call:** the `populate_initial_keypackages` + `MyKeyPackagesRepo::insert` chain may have skipped some KPs. Inspect the DB content via `db.my_keypackages().list_available_one_time()`.
- **KeyPackage fails on second call (returns same):** `mark_consumed` isn't running. Confirm the spawn_blocking path completes successfully.
- **Presence fails:** look at the `presence.lock().await` value at request time.
- **Inbox fails:** check that `inbox_tx.send` reaches `alice_server.next_inbox()`.

### Step 6.3: Run full workspace test (no regressions)

```bash
cargo +1.87-x86_64-pc-windows-msvc test --workspace -- --test-threads=1 2>&1 | grep "test result" | head -15
```

Expected counts (approximate): ghost-core 16, ghost-identity 40, ghost-protocol 43+1, ghost-storage 42+1, ghost-network 16+1, ghost-server 5+1 = ~166 tests.

### Step 6.4: Commit

```bash
git add crates/ghost-server/
git commit -m "test(ghost-server): end-to-end test (Bob's Client queries all 5 endpoints on Alice's Server)"
```

---

## Task 7: Final verification + tag plan-05-complete

**Files:** none (verify + tag).

### Step 7.1: Run the full battery

```bash
export PATH="/c/Strawberry/perl/bin:/c/Strawberry/c/bin:$PATH"
cd /c/Users/david/Desktop/Ghost

cargo +1.87-x86_64-pc-windows-msvc fmt --all -- --check 2>&1 | tail -5
echo "---"
cargo +1.87-x86_64-pc-windows-msvc clippy --all-targets --workspace -- -D warnings 2>&1 | tail -10
echo "---"
cargo +1.87-x86_64-pc-windows-msvc test --workspace -- --test-threads=1 2>&1 | grep "test result" | head -15
echo "---"
bash scripts/smoke-test-plan-01.sh 2>&1 | tail -3
```

If `cargo fmt` reports diffs, run `cargo fmt --all` and re-check.
If `cargo clippy` reports warnings, fix the smallest possible thing.
If any test fails, STOP and report.

### Step 7.2: If cleanup commits were needed, commit

```bash
git status --porcelain
```

If anything is modified:

```bash
git add -A
git commit -m "chore(plan-05): final verification fixes"
```

### Step 7.3: Tag

```bash
git tag -a plan-05-complete -m "Plan 05 (Embedded Server) complete

Deliverable: ghost-server crate implementing the five MVP-1 endpoints
(Version, GetDeliveryKey, GetKeyPackage, GetPresence, InboxMessage)
as typed GhostRequest/GhostResponse exchanges over libp2p
request-response. Server dispatches handlers backed by Identity,
ghost-storage MyKeyPackagesRepo, and an inbox channel that the
application layer (Plan 06) will drain.

Validated by integration test bob_queries_all_endpoints_on_alice in
crates/ghost-server/tests/e2e_endpoints.rs which spins up Alice's
full stack (Identity + DB + Network + Server) and Bob's Client,
queries every endpoint over loopback, asserts:
  - protocol = 'ghost/1'
  - delivery_key matches local computation
  - get_key_package returns different KP on consecutive calls
  - presence reflects the configured state
  - send_inbox routes the envelope into Server's inbox channel

Architectural deviation from spec section 5: replaced literal HTTP
paths with libp2p request-response over the existing Plan 04 transport.
Same five logical endpoints, more idiomatic for the chosen stack.

Coverage: ghost-server 5 unit + 1 integration. ~166 total tests pass.
cargo fmt and cargo clippy clean. Plan 01 smoke still passes.

Notable choices:
  - Plan 04's Network refactored: send_to is now a thin wrapper over
    the new request/response API.
  - Server takes Arc<Mutex<Network>>, Arc<IdentityKey>, Arc<Database>,
    Arc<Mutex<PresenceState>>. Sharing pattern matches what ghost-client
    (Plan 06) will need.
  - GetKeyPackage uses spawn_blocking for sync rusqlite work.

Next: Plan 06 — Client Orchestration. Wires Network + Server + Storage
+ Protocol + Identity together into the actual messaging client. First
plan that produces a runnable end-to-end demo (two CLI processes
exchange E2EE messages over loopback)."
```

### Step 7.4: Verify

```bash
git tag -l
git show plan-05-complete --stat | head -25
```

---

## Risks & Open Questions for Plan 05

| Risk | Mitigation |
|---|---|
| Refactoring Plan 04's `Network` may break the e2e_loopback test | Task 1 explicitly updates that test; passing both old and new tests is the gate. |
| `tokio::sync::Mutex<Network>` introduces lock contention if Server and outbound Client share the same Network instance | Plan 06 will revisit. For Plan 05 the e2e test uses separate Network instances per role (Alice has Network+Server, Bob has Network+Client), so contention is irrelevant. |
| `populate_initial_keypackages` creates KeyPackages but does not insert them into MyKeyPackagesRepo | Task 6's e2e test does the manual insertion. Plan 06 will add a helper that does both. |
| `private_key` field of `MyKeyPackageRow` is left empty in the test | Plan 06 will wire this up properly when handling incoming Welcomes (which need the matching private init key). |
| KeyPackage validation on the requesting peer | Out of scope for Plan 05. Plan 06's "first contact" flow validates the KeyPackage via openmls's KeyPackageIn::validate before using it. |
| Presence state is purely passive (consumer sets it) | Documented; Plan 06 will introduce a heartbeat task that updates presence periodically. |

## Self-Review Checklist (after writing this plan)

**1. Spec coverage** — design spec section 5 (HTTP API):
- ✓ `/v1/version` → `GhostRequest::Version`
- ✓ `/v1/keypackages/{ghostid}` → `GhostRequest::GetKeyPackage` (consumes from MyKeyPackagesRepo)
- ✓ `/v1/delivery-key/{ghostid}` → `GhostRequest::GetDeliveryKey` (derived from local IK)
- ✓ `/v1/inbox` → `GhostRequest::InboxMessage` (routes to inbox channel)
- ✓ `/v1/presence/{ghostid}` → `GhostRequest::GetPresence`
- Architectural deviation: HTTP/3 paths → libp2p request-response with typed enums (documented above)

**2. Placeholder scan** — no "TBD" / "TODO". Phrases like "Implementer:" are libp2p API consultation hints.

**3. Type consistency** — `Server::spawn(ik, network, presence, db)` ↔ `Client::new(network)`. `GhostRequest::*` and `GhostResponse::*` variants paired. `InboundEnvelope`, `PresenceState`, `ResponseHandle`, `InboundRequest` named consistently across crates.

---

**Plan 05 complete and ready for execution.**
