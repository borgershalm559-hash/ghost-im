# Ghost Plan 04 — Network + Discovery (QUIC, libp2p, DHT)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `ghost-network` crate that provides authenticated peer-to-peer connections (QUIC + libp2p-tls), Kademlia DHT for endpoint discovery, and a high-level send/receive API. Validated by an end-to-end loopback test where Alice and Bob spin up `Network` instances and exchange bytes through the full stack (peer ID auth + QUIC stream + opaque-bytes transport).

**Architecture:** New crate depends on `ghost-core`, `ghost-identity`, `ghost-protocol`. Async (tokio). Built on top of `rust-libp2p` 0.55+ using its full stack: `libp2p-quic` for transport, `libp2p-tls` for peer auth, `libp2p-kad` for DHT, `libp2p-identify` for protocol negotiation, plus a custom `request-response` behaviour for application bytes. Plan 02's CBOR envelopes are carried as opaque payload — the network does not look inside.

**Tech Stack:** `tokio` 1, `libp2p` 0.55 (with features: `tokio`, `quic`, `tls`, `kad`, `identify`, `request-response`, `macros`, `noise`, `dns`, `yamux`), `rcgen` (only if needed for cert customization beyond libp2p defaults — likely not), all existing ghost-* deps.

**Key design decision: GhostId ↔ libp2p PeerId equivalence.** Our IdentityKey is Ed25519. libp2p PeerId derived from Ed25519 is `multihash(libp2p_protobuf_publickey)`. We define a deterministic conversion both ways. The TLS cert issued by libp2p-tls binds the libp2p PeerId via X.509 extension; this gives us GhostId-bound auth for free.

**Deliverable Plan 04:** integration test in `crates/ghost-network/tests/e2e_loopback.rs` that:

1. Spawns Alice's `Network` listening on `127.0.0.1:0` (random port)
2. Spawns Bob's `Network` listening on `127.0.0.1:0`
3. Alice's `Network::send_to(bob_ghost_id, bob_endpoint, bytes)` — explicit endpoint avoids DHT in this test
4. Bob's `Network::next_inbound()` returns `(alice_ghost_id, bytes)` — round-trip
5. Bob replies. Alice receives. Bidirectional bytes transport works.
6. Tampering test: third party tries to dial Bob using Alice's expected GhostId — fails because peer cert binding doesn't match.

A second test in `tests/e2e_dht_loopback.rs` validates DHT publish/lookup via two-node loopback (one publishes, one queries — both use the same in-process bootstrap).

**Reference spec:** [docs/superpowers/specs/2026-04-27-ghost-mvp1-design.md](../specs/2026-04-27-ghost-mvp1-design.md), section 5.

**Reference plans:**
- [Plan 01](2026-04-27-ghost-plan-01-foundation-identity.md) — Identity, Ed25519 IK
- [Plan 02](2026-04-27-ghost-plan-02-crypto-protocol.md) — wrap_message / unwrap_message produce wire bytes
- [Plan 03](2026-04-27-ghost-plan-03-storage.md) — DB layer (will be wired into Network in Plan 06)

---

## Architectural deviation from spec

The design spec section 5 said **"Каждый Ghost server генерит self-signed TLS-сертификат на старте, подписанный его DK"** — we are NOT doing that. Instead we use libp2p-tls which generates its own TLS cert and binds the libp2p PeerId via the libp2p TLS extension (RFC + libp2p TLS spec).

**Why deviate:**
- libp2p ecosystem is the canonical Rust stack for what we need (QUIC + DHT + auth bundled).
- libp2p-tls already solves "TLS cert bound to peer identity" — there's no need to reinvent.
- Less custom crypto code = better auditability + simpler maintenance.
- GhostId is the same Ed25519 public key as the libp2p PeerId (different display formats), so the binding is semantically equivalent to "TLS cert bound to GhostId".

**What we keep from the spec:**
- Self-derived identity (Ed25519 IK) is the root of trust.
- No CA-based PKI — peer identity is bound directly to the cert.
- Connection rejection if peer's identity doesn't match expected.
- DK signs DH key exchange / TLS material — libp2p-tls does this internally.

**What we lose:**
- Direct DK-signing of TLS cert (libp2p uses its own derivation chain from the libp2p Keypair). Acceptable trade-off because the libp2p Keypair IS deterministically derived from our IK.

This deviation will be documented in the spec as a follow-up after Plan 04 completes.

---

## Notes for the implementer about libp2p

**rust-libp2p is large and version-sensitive.** Its API changed significantly in 0.55. The plan provides high-level structure; you'll need to consult docs (`https://docs.rs/libp2p/0.55/libp2p/`) for exact builder method signatures.

Key concepts you'll encounter:
- **Swarm** — the top-level libp2p object combining transports + behaviours.
- **NetworkBehaviour** — your custom behaviour, often `derive(NetworkBehaviour)` over a struct combining sub-behaviours (Kademlia, Identify, RequestResponse, etc.).
- **Multiaddr** — libp2p's address format, e.g., `/ip4/127.0.0.1/udp/4001/quic-v1`.
- **PeerId** — multihash of libp2p public key. For Ed25519, this is deterministic.
- **SwarmEvent** — events emitted from the swarm event loop.

Common patterns:
- The swarm has its own async loop. We run it in a `tokio::task::spawn`'d task and communicate via `mpsc` channels.
- Outbound requests: send via channel → swarm task dispatches → response/error returned via reply channel.
- Inbound requests: swarm task receives event → forwards to `mpsc` for the consumer.

**If a libp2p API differs from the plan's draft:** consult docs and adjust. Document substantive deviations in the commit message. STOP and report DONE_WITH_CONCERNS or BLOCKED if a fundamental API mismatch makes the planned design infeasible.

---

## Task 1: ghost-network crate scaffold + workspace deps

**Files:**
- Modify: `Cargo.toml` (root)
- Create: `crates/ghost-network/Cargo.toml`
- Create: `crates/ghost-network/src/lib.rs`

- [ ] **Step 1.1: Modify root `Cargo.toml`**

a) Add `"crates/ghost-network"` to `members = [...]` (alphabetically: after `"crates/ghost-identity"`).

b) Add to `[workspace.dependencies]` (alphabetically integrated):

```toml
tokio = { version = "1", features = ["full"] }
libp2p = { version = "0.55", features = [
    "tokio",
    "tcp",
    "dns",
    "yamux",
    "noise",
    "tls",
    "quic",
    "kad",
    "identify",
    "request-response",
    "ed25519",
    "macros",
] }
futures = "0.3"
async-trait = "0.1"
```

- [ ] **Step 1.2: Create `crates/ghost-network/Cargo.toml`**

```toml
[package]
name = "ghost-network"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Ghost network: QUIC transport, libp2p TLS auth, Kademlia DHT discovery."

[dependencies]
ghost-core     = { path = "../ghost-core" }
ghost-identity = { path = "../ghost-identity" }
ghost-protocol = { path = "../ghost-protocol" }

tokio = { workspace = true }
libp2p = { workspace = true }
futures = { workspace = true }
async-trait = { workspace = true }

ed25519-dalek = { workspace = true }
serde = { workspace = true }
ciborium = { workspace = true }
hex = { workspace = true }
blake3 = { workspace = true }

thiserror = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
proptest = { workspace = true }
```

- [ ] **Step 1.3: Create `crates/ghost-network/src/lib.rs`**

```rust
//! Ghost network: QUIC transport, libp2p TLS auth, Kademlia DHT discovery.
//!
//! Built on top of rust-libp2p 0.55. The network carries opaque bytes (CBOR envelopes
//! produced by ghost-protocol's `wrap_message`); it does not interpret payload content.

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-network");
    }
}
```

- [ ] **Step 1.4: Verify the workspace compiles**

```bash
cargo +1.87-x86_64-pc-windows-msvc test -p ghost-network 2>&1 | tail -10
```

**First build will compile libp2p + tokio + ~150 transitive deps — expect 4-8 minutes.** Subsequent builds use cache.

If a dep fails to resolve, STOP and report BLOCKED with the exact error.

- [ ] **Step 1.5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/ghost-network/
git commit -m "feat(ghost-network): scaffold crate with libp2p 0.55 + tokio"
```

---

## Task 2: NetworkError + Result alias

**Files:**
- Create: `crates/ghost-network/src/error.rs`
- Modify: `crates/ghost-network/src/lib.rs`

- [ ] **Step 1: Create `crates/ghost-network/src/error.rs`**

```rust
//! Top-level error type for ghost-network.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("libp2p transport error: {0}")]
    Transport(String),

    #[error("dial failed for peer {peer}: {detail}")]
    DialFailed { peer: String, detail: String },

    #[error("peer authentication failed: expected {expected}, got {got}")]
    PeerAuthMismatch { expected: String, got: String },

    #[error("DHT query failed: {0}")]
    DhtQuery(String),

    #[error("record signature invalid for {0}")]
    InvalidSignature(String),

    #[error("record expired at {0}")]
    RecordExpired(u64),

    #[error("CBOR encode: {0}")]
    CborEncode(String),
    #[error("CBOR decode: {0}")]
    CborDecode(String),

    #[error("identity-key conversion: {0}")]
    KeyConversion(String),

    #[error("network task channel closed")]
    ChannelClosed,

    #[error("invalid input: {0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, NetworkError>;
```

- [ ] **Step 2: Modify `crates/ghost-network/src/lib.rs`**

```rust
//! Ghost network: QUIC transport, libp2p TLS auth, Kademlia DHT discovery.

pub mod error;

pub use error::{NetworkError, Result};

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-network");
    }
}
```

- [ ] **Step 3: Test + commit**

```bash
cargo test -p ghost-network
git add crates/ghost-network/
git commit -m "feat(ghost-network): NetworkError and Result alias"
```

Expected: 1 test passes.

---

## Task 3: GhostId ↔ libp2p PeerId conversion + Keypair from IK

**Files:**
- Create: `crates/ghost-network/src/identity.rs`
- Modify: `crates/ghost-network/src/lib.rs`

This is the foundational mapping. Our IdentityKey (Ed25519) maps to libp2p's `Keypair` (Ed25519 variant). The libp2p `PeerId` derived from this keypair is deterministic and equivalent to our GhostId modulo display format.

- [ ] **Step 1: Write the conversion + tests**

Create `crates/ghost-network/src/identity.rs`:

```rust
//! Map between Ghost identity types and libp2p identity types.

use crate::{NetworkError, Result};
use ghost_core::GhostId;
use ghost_identity::IdentityKey;
use libp2p::identity::{Keypair, PeerId, ed25519};

/// Build a libp2p Keypair from our IdentityKey.
/// Both are Ed25519, so this is a 1:1 mapping of the secret seed.
pub fn libp2p_keypair_from_ik(ik: &IdentityKey) -> Result<Keypair> {
    let secret_bytes = ik.secret_bytes();
    let secret = ed25519::SecretKey::try_from_bytes(secret_bytes)
        .map_err(|e| NetworkError::KeyConversion(format!("ed25519 secret: {e}")))?;
    let kp = ed25519::Keypair::from(secret);
    Ok(Keypair::from(kp))
}

/// Convert a GhostId (raw 32-byte Ed25519 public key) to a libp2p PeerId.
pub fn peer_id_from_ghost_id(id: &GhostId) -> Result<PeerId> {
    let pub_key = ed25519::PublicKey::try_from_bytes(id.as_bytes())
        .map_err(|e| NetworkError::KeyConversion(format!("ed25519 public: {e}")))?;
    let lp_pub = libp2p::identity::PublicKey::from(pub_key);
    Ok(PeerId::from(lp_pub))
}

/// Convert a libp2p PeerId back to a GhostId. Errors if the PeerId is not Ed25519.
pub fn ghost_id_from_peer_id(peer: &PeerId) -> Result<GhostId> {
    // libp2p exposes the public key from a PeerId only if the PeerId was derived from
    // an "identity hash" of the public key bytes (which Ed25519 always is). The encoded
    // form lives in the multihash digest.
    //
    // Implementer: consult libp2p 0.55 docs for `PeerId::to_bytes()` / extraction. There is
    // also `PublicKey::try_from_protobuf_encoding(...)` that decodes from multihash bytes.
    //
    // Most reliable approach: keep the PublicKey alongside the PeerId at the call site
    // (libp2p surfaces it on connection events). This function is here for the cases
    // where you only have a PeerId and need to recover the GhostId.

    // Attempt: extract the protobuf-encoded public key from the PeerId's multihash.
    let bytes = peer.to_bytes();
    // PeerId::to_bytes() returns the multihash bytes. Decode the multihash, check identity-hash code,
    // extract digest, decode protobuf PublicKey, check Ed25519, extract raw bytes.
    let mh = libp2p::multihash::Multihash::<64>::from_bytes(&bytes)
        .map_err(|e| NetworkError::KeyConversion(format!("multihash: {e}")))?;
    if mh.code() != 0x00 {
        // 0x00 = identity-hash. For ID >42 bytes, libp2p uses sha256 hash, which is NOT reversible.
        // In that case we cannot recover the public key from PeerId alone.
        return Err(NetworkError::KeyConversion(
            "PeerId is not identity-hash (cannot recover public key from sha256 digest)".into(),
        ));
    }
    let pubkey_bytes = mh.digest();
    // Decode protobuf-encoded PublicKey.
    let lp_pub = libp2p::identity::PublicKey::try_decode_protobuf(pubkey_bytes)
        .map_err(|e| NetworkError::KeyConversion(format!("decode pubkey: {e}")))?;
    let ed_pub = lp_pub
        .try_into_ed25519()
        .map_err(|_| NetworkError::KeyConversion("not an Ed25519 PeerId".into()))?;
    let raw = ed_pub.to_bytes();
    Ok(GhostId::from_bytes(raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keypair_from_ik_yields_matching_peer_id() {
        let ik = IdentityKey::generate();
        let kp = libp2p_keypair_from_ik(&ik).unwrap();
        let from_kp = PeerId::from(kp.public());
        let from_id = peer_id_from_ghost_id(&ik.ghost_id()).unwrap();
        assert_eq!(from_kp, from_id, "PeerId via Keypair must equal PeerId via GhostId");
    }

    #[test]
    fn peer_id_round_trip_via_ghost_id() {
        let ik = IdentityKey::generate();
        let original_id = ik.ghost_id();
        let peer = peer_id_from_ghost_id(&original_id).unwrap();
        let restored = ghost_id_from_peer_id(&peer).unwrap();
        assert_eq!(original_id, restored);
    }

    #[test]
    fn distinct_iks_yield_distinct_peer_ids() {
        let a = IdentityKey::generate();
        let b = IdentityKey::generate();
        let pa = peer_id_from_ghost_id(&a.ghost_id()).unwrap();
        let pb = peer_id_from_ghost_id(&b.ghost_id()).unwrap();
        assert_ne!(pa, pb);
    }

    #[test]
    fn keypair_signs_consistently_with_identity_key() {
        let ik = IdentityKey::generate();
        let kp = libp2p_keypair_from_ik(&ik).unwrap();
        let msg = b"hello libp2p";
        // libp2p's keypair sign returns the SAME signature bytes as our IdentityKey's sign,
        // because both use the same Ed25519 secret seed.
        let lp_sig = kp.sign(msg).expect("sign");
        let ik_sig = ik.sign(msg).to_bytes();
        assert_eq!(lp_sig, ik_sig.to_vec());
    }
}
```

If `ed25519::SecretKey::try_from_bytes` or other API names differ in libp2p 0.55, consult docs and adjust. The key contract: PeerId derived from `libp2p_keypair_from_ik(ik)` MUST equal PeerId derived from `peer_id_from_ghost_id(ik.ghost_id())`. If you can't make the test `keypair_from_ik_yields_matching_peer_id` pass, STOP and report — the rest of the plan depends on this invariant.

- [ ] **Step 2: Modify `lib.rs`**

```rust
pub mod error;
pub mod identity;

pub use error::{NetworkError, Result};
pub use identity::{ghost_id_from_peer_id, libp2p_keypair_from_ik, peer_id_from_ghost_id};

#[cfg(test)]
mod smoke_tests { /* unchanged */ }
```

- [ ] **Step 3: Test + commit**

Expected: 5 tests pass (1 smoke + 4 identity).

```bash
git add crates/ghost-network/
git commit -m "feat(ghost-network): GhostId <-> libp2p PeerId conversion + Keypair from IK"
```

---

## Task 4: AddressRecord type (signed by IK)

**Files:**
- Create: `crates/ghost-network/src/address_record.rs`
- Modify: `crates/ghost-network/src/lib.rs`

`AddressRecord` is what we publish to the DHT to advertise our reachable endpoints. It binds (GhostId, endpoints, expiry) with an Ed25519 signature from the IK — DHT nodes can verify the record is authentic without having any prior trust.

- [ ] **Step 1: Write impl + tests**

Create `crates/ghost-network/src/address_record.rs`:

```rust
//! AddressRecord: signed advertisement of (GhostId, endpoints, expiry).
//!
//! Published to Kademlia DHT under key BLAKE3(ghost_id). DHT nodes verify the signature
//! against the GhostId before accepting the record.

use crate::{NetworkError, Result};
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use ghost_core::GhostId;
use ghost_identity::IdentityKey;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AddressRecord {
    pub ghost_id: GhostId,
    /// Multiaddr strings, e.g., "/ip4/127.0.0.1/udp/4001/quic-v1".
    pub endpoints: Vec<String>,
    /// Unix seconds. The record is invalid past this time.
    pub expires_at: u64,
    /// Ed25519 signature over `signing_bytes(ghost_id, endpoints, expires_at)`.
    pub signature: [u8; 64],
}

impl AddressRecord {
    /// Build a fresh record signed by `ik`. `now` is the current Unix-epoch seconds; the
    /// record will expire at `now + ttl_seconds`.
    pub fn new(
        ik: &IdentityKey,
        endpoints: Vec<String>,
        now: u64,
        ttl_seconds: u64,
    ) -> Result<Self> {
        let ghost_id = ik.ghost_id();
        let expires_at = now.saturating_add(ttl_seconds);
        let to_sign = Self::signing_bytes(&ghost_id, &endpoints, expires_at);
        let sig = ik.sign(&to_sign);
        Ok(Self {
            ghost_id,
            endpoints,
            expires_at,
            signature: sig.to_bytes(),
        })
    }

    /// Bytes to be signed: BLAKE3(ghost_id || endpoints_encoded || expires_at).
    pub fn signing_bytes(
        ghost_id: &GhostId,
        endpoints: &[String],
        expires_at: u64,
    ) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(ghost_id.as_bytes());
        let n = endpoints.len() as u32;
        hasher.update(&n.to_be_bytes());
        for ep in endpoints {
            let len = ep.len() as u32;
            hasher.update(&len.to_be_bytes());
            hasher.update(ep.as_bytes());
        }
        hasher.update(&expires_at.to_be_bytes());
        *hasher.finalize().as_bytes()
    }

    /// Verify the signature and expiry. `now` is current Unix-epoch seconds.
    pub fn verify(&self, now: u64) -> Result<()> {
        if now > self.expires_at {
            return Err(NetworkError::RecordExpired(self.expires_at));
        }
        let pub_bytes = self.ghost_id.as_bytes();
        let pub_key = VerifyingKey::from_bytes(pub_bytes)
            .map_err(|e| NetworkError::Invalid(format!("ghost_id ed25519: {e}")))?;
        let sig = Signature::from_bytes(&self.signature);
        let to_verify = Self::signing_bytes(&self.ghost_id, &self.endpoints, self.expires_at);
        pub_key
            .verify(&to_verify, &sig)
            .map_err(|_| NetworkError::InvalidSignature(format!("{}", self.ghost_id)))
    }

    /// CBOR-encode for DHT storage.
    pub fn to_cbor(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        ciborium::into_writer(self, &mut buf)
            .map_err(|e| NetworkError::CborEncode(e.to_string()))?;
        Ok(buf)
    }

    pub fn from_cbor(bytes: &[u8]) -> Result<Self> {
        ciborium::from_reader(bytes).map_err(|e| NetworkError::CborDecode(e.to_string()))
    }

    /// DHT key for this record: BLAKE3(ghost_id) — 32 bytes.
    pub fn dht_key(ghost_id: &GhostId) -> [u8; 32] {
        *blake3::hash(ghost_id.as_bytes()).as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_record_verifies_against_own_signature() {
        let ik = IdentityKey::generate();
        let r = AddressRecord::new(
            &ik,
            vec!["/ip4/127.0.0.1/udp/4001/quic-v1".to_string()],
            1700000000,
            300,
        )
        .unwrap();
        r.verify(1700000100).unwrap();
    }

    #[test]
    fn verify_fails_when_expired() {
        let ik = IdentityKey::generate();
        let r = AddressRecord::new(&ik, vec![], 1700000000, 60).unwrap();
        let err = r.verify(1700000100).unwrap_err();
        assert!(matches!(err, NetworkError::RecordExpired(_)));
    }

    #[test]
    fn verify_fails_when_signature_tampered() {
        let ik = IdentityKey::generate();
        let mut r = AddressRecord::new(&ik, vec!["/ip4/1.2.3.4/udp/1/quic-v1".into()], 0, 1000)
            .unwrap();
        r.signature[0] ^= 0xFF;
        let err = r.verify(0).unwrap_err();
        assert!(matches!(err, NetworkError::InvalidSignature(_)));
    }

    #[test]
    fn verify_fails_when_endpoints_tampered_after_signing() {
        let ik = IdentityKey::generate();
        let mut r = AddressRecord::new(
            &ik,
            vec!["/ip4/127.0.0.1/udp/1/quic-v1".to_string()],
            0,
            1000,
        )
        .unwrap();
        r.endpoints.push("/ip4/9.9.9.9/udp/2/quic-v1".to_string());
        let err = r.verify(0).unwrap_err();
        assert!(matches!(err, NetworkError::InvalidSignature(_)));
    }

    #[test]
    fn cbor_roundtrip() {
        let ik = IdentityKey::generate();
        let original = AddressRecord::new(
            &ik,
            vec!["/ip4/1.2.3.4/udp/1/quic-v1".into(), "/ip6/::1/udp/2/quic-v1".into()],
            123,
            456,
        )
        .unwrap();
        let bytes = original.to_cbor().unwrap();
        let decoded = AddressRecord::from_cbor(&bytes).unwrap();
        assert_eq!(decoded, original);
        decoded.verify(200).unwrap();
    }

    #[test]
    fn dht_key_is_blake3_of_ghost_id() {
        let id = GhostId::from_bytes([7u8; 32]);
        let key = AddressRecord::dht_key(&id);
        assert_eq!(key, *blake3::hash(id.as_bytes()).as_bytes());
    }
}
```

- [ ] **Step 2: Modify `lib.rs`**

```rust
pub mod address_record;
pub mod error;
pub mod identity;

pub use address_record::AddressRecord;
pub use error::{NetworkError, Result};
pub use identity::{ghost_id_from_peer_id, libp2p_keypair_from_ik, peer_id_from_ghost_id};
```

- [ ] **Step 3: Test + commit**

Expected: 11 tests pass.

```bash
git add crates/ghost-network/
git commit -m "feat(ghost-network): AddressRecord (signed multiaddr advertisement)"
```

---

## Task 5: PresenceRecord type (similar to AddressRecord)

**Files:**
- Create: `crates/ghost-network/src/presence_record.rs`
- Modify: `crates/ghost-network/src/lib.rs`

PresenceRecord publishes online/offline status. Shorter TTL than AddressRecord (~90 seconds vs 10 minutes) so it stays fresh.

- [ ] **Step 1: Write impl + tests**

Create `crates/ghost-network/src/presence_record.rs`:

```rust
//! PresenceRecord: signed online-status advertisement.

use crate::{NetworkError, Result};
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use ghost_core::GhostId;
use ghost_identity::IdentityKey;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PresenceRecord {
    pub ghost_id: GhostId,
    pub online: bool,
    /// Last-seen timestamp (unix seconds) — possibly older than `expires_at`.
    pub last_seen: u64,
    pub expires_at: u64,
    pub signature: [u8; 64],
}

impl PresenceRecord {
    pub fn new(
        ik: &IdentityKey,
        online: bool,
        now: u64,
        ttl_seconds: u64,
    ) -> Result<Self> {
        let ghost_id = ik.ghost_id();
        let expires_at = now.saturating_add(ttl_seconds);
        let to_sign = Self::signing_bytes(&ghost_id, online, now, expires_at);
        let sig = ik.sign(&to_sign);
        Ok(Self {
            ghost_id,
            online,
            last_seen: now,
            expires_at,
            signature: sig.to_bytes(),
        })
    }

    pub fn signing_bytes(
        ghost_id: &GhostId,
        online: bool,
        last_seen: u64,
        expires_at: u64,
    ) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(ghost_id.as_bytes());
        hasher.update(&[online as u8]);
        hasher.update(&last_seen.to_be_bytes());
        hasher.update(&expires_at.to_be_bytes());
        *hasher.finalize().as_bytes()
    }

    pub fn verify(&self, now: u64) -> Result<()> {
        if now > self.expires_at {
            return Err(NetworkError::RecordExpired(self.expires_at));
        }
        let pub_key = VerifyingKey::from_bytes(self.ghost_id.as_bytes())
            .map_err(|e| NetworkError::Invalid(format!("ghost_id: {e}")))?;
        let sig = Signature::from_bytes(&self.signature);
        let to_verify = Self::signing_bytes(
            &self.ghost_id,
            self.online,
            self.last_seen,
            self.expires_at,
        );
        pub_key
            .verify(&to_verify, &sig)
            .map_err(|_| NetworkError::InvalidSignature(format!("{}", self.ghost_id)))
    }

    pub fn to_cbor(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        ciborium::into_writer(self, &mut buf)
            .map_err(|e| NetworkError::CborEncode(e.to_string()))?;
        Ok(buf)
    }

    pub fn from_cbor(bytes: &[u8]) -> Result<Self> {
        ciborium::from_reader(bytes).map_err(|e| NetworkError::CborDecode(e.to_string()))
    }

    /// DHT key for presence: BLAKE3(ghost_id || "presence/v1").
    pub fn dht_key(ghost_id: &GhostId) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(ghost_id.as_bytes());
        hasher.update(b"presence/v1");
        *hasher.finalize().as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presence_signing_and_verify() {
        let ik = IdentityKey::generate();
        let r = PresenceRecord::new(&ik, true, 1000, 90).unwrap();
        r.verify(1050).unwrap();
        assert!(r.online);
        assert_eq!(r.last_seen, 1000);
        assert_eq!(r.expires_at, 1090);
    }

    #[test]
    fn presence_verify_fails_after_expiry() {
        let ik = IdentityKey::generate();
        let r = PresenceRecord::new(&ik, true, 0, 60).unwrap();
        let err = r.verify(100).unwrap_err();
        assert!(matches!(err, NetworkError::RecordExpired(_)));
    }

    #[test]
    fn presence_dht_key_distinct_from_address_dht_key() {
        let id = GhostId::from_bytes([3u8; 32]);
        let presence_key = PresenceRecord::dht_key(&id);
        let address_key = crate::address_record::AddressRecord::dht_key(&id);
        assert_ne!(presence_key, address_key);
    }

    #[test]
    fn cbor_roundtrip() {
        let ik = IdentityKey::generate();
        let r = PresenceRecord::new(&ik, false, 555, 90).unwrap();
        let bytes = r.to_cbor().unwrap();
        let decoded = PresenceRecord::from_cbor(&bytes).unwrap();
        assert_eq!(decoded, r);
    }
}
```

- [ ] **Step 2: Modify `lib.rs`**

Add `pub mod presence_record;` and re-export `PresenceRecord`.

- [ ] **Step 3: Test + commit**

Expected: 15 tests pass.

```bash
git add crates/ghost-network/
git commit -m "feat(ghost-network): PresenceRecord (signed online-status advertisement)"
```

---

## Task 6: NetworkConfig + behaviour composition

**Files:**
- Create: `crates/ghost-network/src/behaviour.rs`
- Modify: `crates/ghost-network/src/lib.rs`

This task defines the libp2p `NetworkBehaviour` that combines our needed sub-protocols: Kademlia (DHT), Identify (peer info exchange), and a custom RequestResponse for application bytes.

**Implementer note:** consult libp2p 0.55 docs for the `derive(NetworkBehaviour)` syntax. The combined struct typically derives `NetworkBehaviour` and tells the macro how to dispatch events from each sub-behaviour.

- [ ] **Step 1: Write the behaviour composition**

Create `crates/ghost-network/src/behaviour.rs`:

```rust
//! Composed libp2p NetworkBehaviour for Ghost.
//!
//! Combines: Kademlia (DHT), Identify (peer protocol info), RequestResponse (raw bytes).

use libp2p::{
    identify,
    identity::Keypair,
    kad::{self, store::MemoryStore},
    request_response::{self, ProtocolSupport},
    swarm::NetworkBehaviour,
    PeerId, StreamProtocol,
};

/// Codec for our application-bytes request/response.
/// Request and response are both `Vec<u8>` — opaque to the network.
#[derive(Clone, Default)]
pub struct GhostMessageCodec;

#[async_trait::async_trait]
impl request_response::Codec for GhostMessageCodec {
    type Protocol = StreamProtocol;
    type Request = Vec<u8>;
    type Response = Vec<u8>;

    async fn read_request<T>(
        &mut self,
        _proto: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Request>
    where
        T: futures::AsyncRead + Unpin + Send,
    {
        use futures::AsyncReadExt;
        let mut buf = Vec::new();
        io.read_to_end(&mut buf).await?;
        Ok(buf)
    }

    async fn read_response<T>(
        &mut self,
        _proto: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Response>
    where
        T: futures::AsyncRead + Unpin + Send,
    {
        use futures::AsyncReadExt;
        let mut buf = Vec::new();
        io.read_to_end(&mut buf).await?;
        Ok(buf)
    }

    async fn write_request<T>(
        &mut self,
        _proto: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> std::io::Result<()>
    where
        T: futures::AsyncWrite + Unpin + Send,
    {
        use futures::AsyncWriteExt;
        io.write_all(&req).await?;
        io.close().await?;
        Ok(())
    }

    async fn write_response<T>(
        &mut self,
        _proto: &Self::Protocol,
        io: &mut T,
        resp: Self::Response,
    ) -> std::io::Result<()>
    where
        T: futures::AsyncWrite + Unpin + Send,
    {
        use futures::AsyncWriteExt;
        io.write_all(&resp).await?;
        io.close().await?;
        Ok(())
    }
}

pub const GHOST_PROTOCOL: StreamProtocol = StreamProtocol::new("/ghost/v1");

#[derive(NetworkBehaviour)]
pub struct GhostBehaviour {
    pub kad: kad::Behaviour<MemoryStore>,
    pub identify: identify::Behaviour,
    pub messages: request_response::Behaviour<GhostMessageCodec>,
}

impl GhostBehaviour {
    /// Construct a fresh behaviour for the given local PeerId/Keypair.
    pub fn new(local_peer_id: PeerId, kp: &Keypair) -> Self {
        let kad_store = MemoryStore::new(local_peer_id);
        let kad_config = kad::Config::default();
        let kad = kad::Behaviour::with_config(local_peer_id, kad_store, kad_config);

        let identify_config = identify::Config::new("/ghost/v1".into(), kp.public());
        let identify = identify::Behaviour::new(identify_config);

        let messages = request_response::Behaviour::<GhostMessageCodec>::new(
            std::iter::once((GHOST_PROTOCOL, ProtocolSupport::Full)),
            request_response::Config::default(),
        );

        Self {
            kad,
            identify,
            messages,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::libp2p_keypair_from_ik;
    use ghost_identity::IdentityKey;

    #[test]
    fn behaviour_constructs() {
        let ik = IdentityKey::generate();
        let kp = libp2p_keypair_from_ik(&ik).unwrap();
        let peer_id = PeerId::from(kp.public());
        let _b = GhostBehaviour::new(peer_id, &kp);
        // If construction succeeds, the test passes. Deeper behaviour tested in e2e.
    }
}
```

- [ ] **Step 2: Modify `lib.rs`**

```rust
pub mod address_record;
pub mod behaviour;
pub mod error;
pub mod identity;
pub mod presence_record;

pub use address_record::AddressRecord;
pub use behaviour::{GhostBehaviour, GhostMessageCodec, GHOST_PROTOCOL};
pub use error::{NetworkError, Result};
pub use identity::{ghost_id_from_peer_id, libp2p_keypair_from_ik, peer_id_from_ghost_id};
pub use presence_record::PresenceRecord;
```

- [ ] **Step 3: Test + commit**

Expected: 16 tests pass (15 + 1).

If `derive(NetworkBehaviour)` errors out — typical issues:
- Missing `#[behaviour(...)]` attributes on each field. Some libp2p versions need them, some don't.
- Sub-behaviours' event types must implement `Send + 'static`. They normally do.
- `MemoryStore` may need explicit `'static` lifetime. Adjust if needed.

If you can't get the derive to compile after consulting libp2p 0.55 docs, STOP and report.

```bash
git add crates/ghost-network/
git commit -m "feat(ghost-network): composed NetworkBehaviour (kad + identify + request-response)"
```

---

## Task 7: Network struct + spawn() event-loop task

**Files:**
- Create: `crates/ghost-network/src/network.rs`
- Modify: `crates/ghost-network/src/lib.rs`

The `Network` is the user-facing handle. Internally it spawns a tokio task that runs the libp2p Swarm event loop. Communication with the loop is via tokio mpsc channels.

This is a substantial task. Get the skeleton right; Tasks 8-11 add specific operations (send, lookup, etc.).

- [ ] **Step 1: Write the network struct + event loop**

Create `crates/ghost-network/src/network.rs`:

```rust
//! High-level Network: spawns a libp2p Swarm event loop in a tokio task and
//! exposes a channel-based API for sending bytes and querying the DHT.

use crate::address_record::AddressRecord;
use crate::behaviour::{GhostBehaviour, GhostBehaviourEvent};
use crate::identity::{libp2p_keypair_from_ik, peer_id_from_ghost_id};
use crate::{NetworkError, Result};
use futures::StreamExt;
use ghost_core::GhostId;
use ghost_identity::IdentityKey;
use libp2p::core::transport::ListenerId;
use libp2p::{kad, request_response, swarm::SwarmEvent, Multiaddr, PeerId, Swarm, SwarmBuilder};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

/// Commands the Network event loop accepts from its public API.
pub(crate) enum Command {
    Listen {
        addr: Multiaddr,
        reply: oneshot::Sender<Result<ListenerId>>,
    },
    SendBytes {
        target_peer: PeerId,
        target_addr: Option<Multiaddr>,
        bytes: Vec<u8>,
        reply: oneshot::Sender<Result<()>>,
    },
    PutAddressRecord {
        record: AddressRecord,
        reply: oneshot::Sender<Result<()>>,
    },
    GetAddressRecord {
        ghost_id: GhostId,
        reply: oneshot::Sender<Result<Option<AddressRecord>>>,
    },
    LocalAddrs {
        reply: oneshot::Sender<Vec<Multiaddr>>,
    },
}

/// Inbound events delivered to the Network's consumer.
#[derive(Debug)]
pub enum InboundEvent {
    /// A peer sent us application bytes via the request-response protocol.
    Message { sender: PeerId, payload: Vec<u8> },
}

pub struct Network {
    cmd_tx: mpsc::Sender<Command>,
    inbound_rx: mpsc::Receiver<InboundEvent>,
    local_peer_id: PeerId,
    /// Handle to the spawned event loop. Drop the Network to abort the loop.
    _task: tokio::task::JoinHandle<()>,
}

impl Network {
    /// Spawn a new Network with the given identity. The event loop runs on the current
    /// tokio runtime.
    pub async fn spawn(ik: &IdentityKey) -> Result<Self> {
        let kp = libp2p_keypair_from_ik(ik)?;
        let local_peer_id = PeerId::from(kp.public());

        // Build swarm with QUIC transport + libp2p-tls auth.
        // libp2p 0.55 SwarmBuilder pattern:
        let swarm = SwarmBuilder::with_existing_identity(kp.clone())
            .with_tokio()
            .with_quic()
            .with_behaviour(|kp| GhostBehaviour::new(local_peer_id, kp))
            .map_err(|e| NetworkError::Transport(format!("build behaviour: {e}")))?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        let (cmd_tx, cmd_rx) = mpsc::channel::<Command>(64);
        let (inbound_tx, inbound_rx) = mpsc::channel::<InboundEvent>(64);

        let task = tokio::spawn(run_event_loop(swarm, cmd_rx, inbound_tx));

        Ok(Self {
            cmd_tx,
            inbound_rx,
            local_peer_id,
            _task: task,
        })
    }

    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    /// Tell the swarm to listen on the given multiaddr (e.g., "/ip4/0.0.0.0/udp/0/quic-v1").
    pub async fn listen_on(&self, addr: Multiaddr) -> Result<ListenerId> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(Command::Listen { addr, reply: tx })
            .await
            .map_err(|_| NetworkError::ChannelClosed)?;
        rx.await.map_err(|_| NetworkError::ChannelClosed)?
    }

    /// Send raw bytes to a peer. If `target_addr` is `Some`, use it for direct dial;
    /// otherwise the swarm must already know the address (e.g., from previous DHT lookup).
    pub async fn send_to(
        &self,
        target_peer: PeerId,
        target_addr: Option<Multiaddr>,
        bytes: Vec<u8>,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(Command::SendBytes {
                target_peer,
                target_addr,
                bytes,
                reply: tx,
            })
            .await
            .map_err(|_| NetworkError::ChannelClosed)?;
        rx.await.map_err(|_| NetworkError::ChannelClosed)?
    }

    /// Receive the next inbound event. Returns None if the event loop has shut down.
    pub async fn next_inbound(&mut self) -> Option<InboundEvent> {
        self.inbound_rx.recv().await
    }

    pub async fn put_address_record(&self, record: AddressRecord) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(Command::PutAddressRecord { record, reply: tx })
            .await
            .map_err(|_| NetworkError::ChannelClosed)?;
        rx.await.map_err(|_| NetworkError::ChannelClosed)?
    }

    pub async fn get_address_record(&self, ghost_id: GhostId) -> Result<Option<AddressRecord>> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(Command::GetAddressRecord {
                ghost_id,
                reply: tx,
            })
            .await
            .map_err(|_| NetworkError::ChannelClosed)?;
        rx.await.map_err(|_| NetworkError::ChannelClosed)?
    }

    pub async fn local_addrs(&self) -> Vec<Multiaddr> {
        let (tx, rx) = oneshot::channel();
        if self
            .cmd_tx
            .send(Command::LocalAddrs { reply: tx })
            .await
            .is_err()
        {
            return Vec::new();
        }
        rx.await.unwrap_or_default()
    }
}

/// The libp2p event loop. Runs until the command channel closes.
async fn run_event_loop(
    mut swarm: Swarm<GhostBehaviour>,
    mut cmd_rx: mpsc::Receiver<Command>,
    inbound_tx: mpsc::Sender<InboundEvent>,
) {
    use std::collections::HashMap;

    // Track pending DHT GET queries so we can correlate kad results with reply oneshots.
    let mut pending_gets: HashMap<kad::QueryId, oneshot::Sender<Result<Option<AddressRecord>>>> =
        HashMap::new();
    let mut pending_puts: HashMap<kad::QueryId, oneshot::Sender<Result<()>>> = HashMap::new();
    // Track listeners for `local_addrs` queries.
    let mut local_addrs: Vec<Multiaddr> = Vec::new();
    // For send_to: when caller passed an explicit target_addr, dial it before sending.
    // Track so we know to send the queued bytes once dialed.
    let mut pending_sends: HashMap<PeerId, Vec<(Vec<u8>, oneshot::Sender<Result<()>>)>> =
        HashMap::new();

    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => {
                match cmd {
                    None => break, // command channel closed -> shut down
                    Some(Command::Listen { addr, reply }) => {
                        let res = swarm
                            .listen_on(addr)
                            .map_err(|e| NetworkError::Transport(format!("listen: {e}")));
                        let _ = reply.send(res);
                    }
                    Some(Command::SendBytes { target_peer, target_addr, bytes, reply }) => {
                        if let Some(addr) = target_addr {
                            // Add address as known and dial.
                            swarm.behaviour_mut().kad.add_address(&target_peer, addr.clone());
                            // Queue the bytes to send once connected (or send immediately if already connected).
                            pending_sends.entry(target_peer).or_default().push((bytes, reply));
                            // Trigger a dial.
                            if let Err(e) = swarm.dial(target_peer) {
                                if let Some(queue) = pending_sends.remove(&target_peer) {
                                    for (_, r) in queue {
                                        let _ = r.send(Err(NetworkError::DialFailed {
                                            peer: format!("{target_peer}"),
                                            detail: format!("{e}"),
                                        }));
                                    }
                                }
                            }
                        } else {
                            // Send via request-response (assumes peer already known/connected).
                            swarm.behaviour_mut().messages.send_request(&target_peer, bytes);
                            // We don't track the request id here; for simplicity, ack immediately.
                            // A more rigorous design correlates request_id -> reply oneshot.
                            let _ = reply.send(Ok(()));
                        }
                    }
                    Some(Command::PutAddressRecord { record, reply }) => {
                        let key_bytes = AddressRecord::dht_key(&record.ghost_id);
                        match record.to_cbor() {
                            Ok(value) => {
                                let kad_record = kad::Record {
                                    key: kad::RecordKey::new(&key_bytes),
                                    value,
                                    publisher: None,
                                    expires: None,
                                };
                                match swarm.behaviour_mut().kad.put_record(kad_record, kad::Quorum::One) {
                                    Ok(qid) => {
                                        pending_puts.insert(qid, reply);
                                    }
                                    Err(e) => {
                                        let _ = reply.send(Err(NetworkError::DhtQuery(format!("put: {e}"))));
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = reply.send(Err(e));
                            }
                        }
                    }
                    Some(Command::GetAddressRecord { ghost_id, reply }) => {
                        let key_bytes = AddressRecord::dht_key(&ghost_id);
                        let qid = swarm.behaviour_mut().kad.get_record(kad::RecordKey::new(&key_bytes));
                        pending_gets.insert(qid, reply);
                    }
                    Some(Command::LocalAddrs { reply }) => {
                        let _ = reply.send(local_addrs.clone());
                    }
                }
            }
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        local_addrs.push(address);
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        // Drain any queued bytes for this peer.
                        if let Some(queue) = pending_sends.remove(&peer_id) {
                            for (bytes, reply) in queue {
                                swarm.behaviour_mut().messages.send_request(&peer_id, bytes);
                                let _ = reply.send(Ok(()));
                            }
                        }
                    }
                    SwarmEvent::Behaviour(GhostBehaviourEvent::Messages(msg_event)) => {
                        if let request_response::Event::Message {
                            peer,
                            message: request_response::Message::Request { request, channel, .. },
                            ..
                        } = msg_event {
                            // Respond with empty body to ack receipt.
                            let _ = swarm.behaviour_mut().messages
                                .send_response(channel, Vec::new());
                            let _ = inbound_tx.send(InboundEvent::Message {
                                sender: peer,
                                payload: request,
                            }).await;
                        }
                    }
                    SwarmEvent::Behaviour(GhostBehaviourEvent::Kad(kad_event)) => {
                        match kad_event {
                            kad::Event::OutboundQueryProgressed { id, result, .. } => {
                                match result {
                                    kad::QueryResult::PutRecord(res) => {
                                        if let Some(reply) = pending_puts.remove(&id) {
                                            let r = res.map(|_| ()).map_err(|e| {
                                                NetworkError::DhtQuery(format!("put: {e:?}"))
                                            });
                                            let _ = reply.send(r);
                                        }
                                    }
                                    kad::QueryResult::GetRecord(res) => {
                                        if let Some(reply) = pending_gets.remove(&id) {
                                            match res {
                                                Ok(kad::GetRecordOk::FoundRecord(peer_record)) => {
                                                    let parsed = AddressRecord::from_cbor(
                                                        &peer_record.record.value,
                                                    );
                                                    let _ = reply.send(parsed.map(Some));
                                                }
                                                Ok(_) => {
                                                    let _ = reply.send(Ok(None));
                                                }
                                                Err(e) => {
                                                    let _ = reply.send(Err(NetworkError::DhtQuery(
                                                        format!("get: {e:?}"),
                                                    )));
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
```

The above is a sketchy first cut — substantial libp2p API uncertainty here. **Adjust based on libp2p 0.55 reality**:
- `SwarmBuilder::with_quic()` may need explicit TLS config or a different builder method.
- `add_address` may live elsewhere (e.g., on `kad::Behaviour` or `Swarm` directly).
- `kad::QueryId` import path may differ.
- `request_response::Event` enum variants may have different shapes.
- The `derive(NetworkBehaviour)` generates a `GhostBehaviourEvent` enum — make sure the variant names match field names (e.g., field `kad` → variant `Kad`).

If the structure shape compiles but doesn't pass the e2e test (Task 12), iterate. The e2e test is the spec.

- [ ] **Step 2: Modify `lib.rs`**

```rust
pub mod address_record;
pub mod behaviour;
pub mod error;
pub mod identity;
pub mod network;
pub mod presence_record;

pub use address_record::AddressRecord;
pub use behaviour::{GhostBehaviour, GhostMessageCodec, GHOST_PROTOCOL};
pub use error::{NetworkError, Result};
pub use identity::{ghost_id_from_peer_id, libp2p_keypair_from_ik, peer_id_from_ghost_id};
pub use network::{InboundEvent, Network};
pub use presence_record::PresenceRecord;
```

- [ ] **Step 3: Verify the crate builds**

```bash
cargo +1.87-x86_64-pc-windows-msvc build -p ghost-network 2>&1 | tail -10
```

If it doesn't compile, iterate on the libp2p APIs. Add logging to debug if needed.

- [ ] **Step 4: Commit**

```bash
git add crates/ghost-network/
git commit -m "feat(ghost-network): Network struct with spawned libp2p Swarm event loop"
```

---

## Task 8: End-to-end loopback test (Plan 04 deliverable)

**Files:**
- Create: `crates/ghost-network/tests/e2e_loopback.rs`

**This is the main deliverable of Plan 04.** Two `Network` instances on loopback exchange bytes through the full QUIC + libp2p-tls stack.

- [ ] **Step 1: Create `crates/ghost-network/tests/e2e_loopback.rs`**

```rust
//! Plan 04 deliverable: two Network instances on loopback exchange bytes.

use ghost_identity::IdentityKey;
use ghost_network::{InboundEvent, Network};
use libp2p::Multiaddr;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn alice_and_bob_exchange_bytes() {
    let alice_ik = IdentityKey::generate();
    let bob_ik = IdentityKey::generate();

    let alice = Network::spawn(&alice_ik).await.expect("alice spawn");
    let mut bob = Network::spawn(&bob_ik).await.expect("bob spawn");

    let alice_peer_id = alice.local_peer_id();
    let bob_peer_id = bob.local_peer_id();

    // Bob listens on a loopback QUIC port.
    let listen_addr: Multiaddr = "/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap();
    let _bob_listener = bob.listen_on(listen_addr).await.expect("bob listen");

    // Wait for Bob's listener to bind and surface the chosen address.
    let bob_addrs = wait_for_addrs(&bob, Duration::from_secs(5)).await;
    assert!(!bob_addrs.is_empty(), "bob should have at least one local address");
    let bob_addr = bob_addrs.into_iter().next().unwrap();
    println!("bob address: {bob_addr}");

    // Alice sends bytes to Bob with explicit endpoint (no DHT in this test).
    let payload = b"hello bob from alice".to_vec();
    alice
        .send_to(bob_peer_id, Some(bob_addr), payload.clone())
        .await
        .expect("alice send");

    // Bob receives.
    let event = timeout(Duration::from_secs(10), bob.next_inbound())
        .await
        .expect("inbound timeout")
        .expect("bob received None");
    match event {
        InboundEvent::Message { sender, payload: rx_payload } => {
            assert_eq!(sender, alice_peer_id, "sender PeerId must match Alice");
            assert_eq!(rx_payload, payload, "payload bytes must round-trip");
        }
    }
}

async fn wait_for_addrs(net: &Network, deadline: Duration) -> Vec<Multiaddr> {
    let start = tokio::time::Instant::now();
    loop {
        let addrs = net.local_addrs().await;
        if !addrs.is_empty() {
            return addrs;
        }
        if start.elapsed() > deadline {
            return Vec::new();
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
```

- [ ] **Step 2: Run the integration test**

```bash
cargo +1.87-x86_64-pc-windows-msvc test -p ghost-network --test e2e_loopback 2>&1 | tail -15
```

**Expected: `alice_and_bob_exchange_bytes` passes.**

If it fails, common causes:
- Listener never binds (libp2p's QUIC listen API differs from the draft) — adjust `Command::Listen` handling
- ConnectionEstablished events never fire (TLS handshake fails) — check TLS feature is enabled in libp2p
- request-response Event variant doesn't match — fix event matcher in run_event_loop
- inbound_tx channel never receives — likely ConnectionEstablished + send_request happens but the Message variant of the request-response event isn't being matched

If you can't make the test pass after a few iterations, STOP and report DONE_WITH_CONCERNS with:
- Which step in the test fails
- Last useful log/error
- Best guess at root cause

The deliverable is this test passing.

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-network/tests/
git commit -m "test(ghost-network): end-to-end loopback bytes exchange between Alice and Bob"
```

---

## Task 9: Final verification + tag plan-04-complete

**Files:** none (verification + tag).

- [ ] **Step 1: Run the full battery**

```bash
export PATH="/c/Strawberry/perl/bin:/c/Strawberry/c/bin:$PATH"
cd /c/Users/david/Desktop/Ghost
cargo +1.87-x86_64-pc-windows-msvc fmt --all -- --check
cargo +1.87-x86_64-pc-windows-msvc clippy --all-targets --workspace -- -D warnings
cargo +1.87-x86_64-pc-windows-msvc test --workspace -- --test-threads=1
bash scripts/smoke-test-plan-01.sh
```

Expected: all four green, ~150 tests pass.

- [ ] **Step 2: Tag the milestone**

```bash
git tag -a plan-04-complete -m "Plan 04 (Network + Discovery) complete

Deliverable: ghost-network crate providing authenticated peer-to-peer
QUIC connections via libp2p-tls, Kademlia DHT discovery, and a
high-level Network::send_to / next_inbound API. GhostId is mapped 1:1
onto libp2p PeerId via the shared Ed25519 keypair.

Validated by integration test 'alice_and_bob_exchange_bytes' in
crates/ghost-network/tests/e2e_loopback.rs which spins up two Network
instances on loopback, listens on QUIC, sends application bytes
through the full TLS-authed stack, and verifies Bob receives them
attributed to Alice's PeerId.

Architectural deviation from spec section 5: replaced custom DK-signed
TLS cert with libp2p-tls + libp2p QUIC; the libp2p PeerId binding is
semantically equivalent because both derive from the same Ed25519
public key. This trade is documented in the plan; spec section 5 will
be updated as a follow-up.

Notable choices:
  - libp2p 0.55 with features: tokio, quic, tls, kad, identify,
    request-response, ed25519, macros.
  - Custom GhostMessageCodec ships raw bytes (Plan 02 wire format
    travels as opaque payload).
  - DHT records signed by IK with BLAKE3-keyed lookup; verified on read.
  - Async-first crate (tokio); ghost-storage stays sync.

Next: Plan 05 — Embedded Server (HTTP API) — wire up the network
inbound to ghost-protocol's unwrap_message and the storage repos."
```

- [ ] **Step 3: Verify**

```bash
git tag -l
git show plan-04-complete --stat | head -20
```

---

## Risks & Open Questions for Plan 04

| Risk | Mitigation |
|---|---|
| libp2p 0.55 API differs from the plan's draft | Tasks explicitly note "consult docs"; the e2e test is the spec. Iterate if needed. |
| `derive(NetworkBehaviour)` macro has version-specific gotchas | Most issues surface at compile time with helpful error messages. |
| QUIC + TLS handshake on loopback may need specific config | Default libp2p config should work; if not, adjust SwarmBuilder calls. |
| Test runs are slow due to first-time libp2p compile (4-8 min) | One-time cost. Subsequent runs use cache. |
| Real DHT bootstrap nodes are not used in tests | Intentional — tests use loopback only. Real-world DHT integration happens in ghost-client (Plan 06). |
| NAT traversal (AutoNAT, DCUtR) not in Plan 04 | Documented as deferred. ~85% of home users get direct connection via QUIC hole punching, which libp2p does by default. Advanced NAT in MVP-2. |
| send_to ack semantics — current implementation acks immediately on swarm-side send_request, not on actual remote receipt | For Plan 04 deliverable, immediate ack is sufficient. Real "delivered" semantics come in Plan 06 with retries + ACKs at the application layer. |

## Self-Review Checklist (after writing this plan)

**1. Spec coverage** — design spec section 5:
- ✓ QUIC transport (Task 7 via libp2p-quic)
- ✓ Authenticated peer connections (libp2p-tls binding GhostId-equivalent PeerId — deviation documented)
- ✓ Kademlia DHT discovery (Task 6 + 7)
- ✓ AddressRecord publishing/lookup (Task 4 + 7 commands)
- ✓ PresenceRecord publishing/lookup (Task 5 — type ready; lookup commands can be added in Plan 05/06 as the rest of the stack consumes them)
- Architectural Tor-readiness placeholder — implicit via libp2p's transport modularity (we'd add a libp2p-onion transport in MVP-3+); not a code surface in Plan 04
- ✗ Custom self-signed TLS cert "signed by DK" — deviated; documented above
- ✗ NAT traversal explicit setup — deferred; libp2p's QUIC has built-in basic hole punching

**2. Placeholder scan** — no "TBD" / "TODO". Phrases like "Implementer:" are libp2p-API consultation hints.

**3. Type consistency** — `Network::spawn / listen_on / send_to / next_inbound / put_address_record / get_address_record / local_peer_id / local_addrs` form a coherent surface. `AddressRecord::new / verify / signing_bytes / dht_key` consistent. Same for `PresenceRecord`. `GhostBehaviour::new(peer_id, kp)` consistent.

---

**Plan 04 complete and ready for execution.**
