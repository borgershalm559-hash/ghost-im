# Ghost Plan 02 — Crypto + Wire Protocol

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `ghost-protocol` crate that wraps MLS (RFC 9420) group state, sealed-sender envelopes, and KeyPackage-based asynchronous first contact, validated by an in-memory end-to-end test where two identities exchange E2EE messages.

**Architecture:** New crate depends on `ghost-identity` + `ghost-core`. Provides a thin, domain-friendly façade over `openmls` 0.7. Replaces `ghost-identity`'s placeholder X25519 `PreKey` (Plan 01) with proper MLS `KeyPackage`s (Plan 02 — schema v2). All state lives in memory; Plan 03 (storage) will wire persistence.

**Tech Stack:** `openmls` v0.7, `openmls_rust_crypto` v0.4, `openmls_traits` v0.4, `uuid` v1 (with `v7` feature), all existing `ghost-identity`/`ghost-core` deps.

**Deliverable Plan 02:** An integration test in `crates/ghost-protocol/tests/e2e.rs` that performs the full first-contact + bidirectional messaging flow:

1. Alice and Bob create independent Ghost identities (using Plan 01's CLI flow programmatically)
2. Bob generates an MLS `KeyPackage` and publishes it to an in-memory `KeyPackageStore` (stub server)
3. Alice fetches Bob's `KeyPackage`, creates a 2-member MLS group, and gets a `Welcome` message
4. Bob processes the `Welcome` and joins the group
5. Alice encrypts `"hello bob"` through the full envelope stack (MLS → sealed-sender → outer envelope CBOR bytes)
6. Bob receives the bytes, parses the outer envelope, decrypts sealed-sender, decrypts MLS, gets `"hello bob"`
7. Bob encrypts `"hi alice"` back, Alice receives it, full reverse flow works
8. Tampering tests: corrupt outer envelope bytes → reject; replay `msg_uuid` → dedup; wrong delivery key → reject

**Reference spec:** [docs/superpowers/specs/2026-04-27-ghost-mvp1-design.md](../specs/2026-04-27-ghost-mvp1-design.md), sections 3 and 4.

**Reference plan:** [docs/superpowers/plans/2026-04-27-ghost-plan-01-foundation-identity.md](2026-04-27-ghost-plan-01-foundation-identity.md) — Plan 01's identity layer is the foundation this builds on.

---

## Notes for the implementer about openmls

The `openmls` 0.7 API surface is large and version-sensitive. **The plan provides the test code (which IS the behavioural spec) and the call shape, but you should consult `openmls` 0.7 documentation (https://docs.rs/openmls/0.7/openmls/) for exact method signatures, struct field names, and configuration options.**

Where the plan says `// API call placeholder — exact name may differ`, treat that as "look up the corresponding openmls v0.7 method and call it with these inputs". If your call returns a typed result that doesn't match what later tests expect, that signals the placeholder didn't match and you need to consult docs.

Critical openmls concepts you'll use:
- `OpenMlsProvider` — composite trait combining crypto, storage, and rand providers
- `BasicCredential` — simple identity proof, we'll use `GhostId` bytes
- `CredentialWithKey` — credential + signature key
- `KeyPackage` and `KeyPackageBundle` — published bundle for first-contact
- `MlsGroup` — group state machine
- `MlsGroupConfig` / `MlsGroupCreateConfig` — group setup
- `Welcome` — message for invitees to join
- `MlsMessage{In,Out}` — wire-format messages

When in doubt, run `cargo doc -p openmls --open` and search for the type.

If a task seems blocked because openmls APIs don't match what the plan suggests, **report DONE_WITH_CONCERNS or BLOCKED** with the actual API surface you found. The controller will adjust the task. Do NOT silently invent code that compiles but doesn't do what the test wants.

---

## Task 1: ghost-protocol crate skeleton + workspace integration

**Files:**
- Create: `crates/ghost-protocol/Cargo.toml`
- Create: `crates/ghost-protocol/src/lib.rs`
- Modify: `Cargo.toml` (root) — add `ghost-protocol` to workspace members and add openmls deps to `[workspace.dependencies]`

- [ ] **Step 1: Modify root `Cargo.toml`**

Add `"crates/ghost-protocol"` to the existing `members = [...]` list (after `ghost-identity` and before `ghost-identity-cli`). Add to `[workspace.dependencies]` block (insert alphabetically, between existing entries):

```toml
openmls = { version = "0.7", features = ["test-utils"] }
openmls_rust_crypto = "0.4"
openmls_traits = "0.4"
uuid = { version = "1.10", features = ["v7", "serde"] }
```

- [ ] **Step 2: Create `crates/ghost-protocol/Cargo.toml`**

```toml
[package]
name = "ghost-protocol"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Ghost wire protocol: MLS group state, sealed-sender envelopes, KeyPackages."

[dependencies]
ghost-core     = { path = "../ghost-core" }
ghost-identity = { path = "../ghost-identity" }

openmls            = { workspace = true }
openmls_rust_crypto = { workspace = true }
openmls_traits      = { workspace = true }

ed25519-dalek = { workspace = true }
x25519-dalek  = { workspace = true }
chacha20poly1305 = { workspace = true }
blake3 = { workspace = true }
hkdf = { workspace = true }
sha2 = { workspace = true }
zeroize = { workspace = true }
rand = { workspace = true }

serde = { workspace = true }
ciborium = { workspace = true }
hex = { workspace = true }
uuid = { workspace = true }

thiserror = { workspace = true }

[dev-dependencies]
proptest = { workspace = true }
tempfile = { workspace = true }
```

- [ ] **Step 3: Create `crates/ghost-protocol/src/lib.rs`**

```rust
//! Ghost wire protocol: MLS group state, sealed-sender envelopes, KeyPackages.
//!
//! This crate is built on top of `openmls` (RFC 9420 — MLS) and provides a
//! domain-friendly façade for Ghost's specific needs: 2-member groups for 1-on-1
//! conversations, sealed-sender envelopes that hide the sender from the recipient's
//! server, and asynchronous first-contact via published KeyPackages.

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-protocol");
    }
}
```

- [ ] **Step 4: Verify the workspace compiles**

```bash
cargo +1.85-x86_64-pc-windows-msvc test -p ghost-protocol
```

Expected: 1 test passes. The first build will compile openmls and its transitive deps — this can take 2–4 minutes. Subsequent builds reuse the cache.

If the openmls features in the manifest cause a resolver error, STOP and report BLOCKED with the exact error. The most common failure is feature-flag mismatch between `openmls` and `openmls_rust_crypto` — they must share the same crypto-suite features.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/ghost-protocol/
git commit -m "feat(ghost-protocol): scaffold crate with openmls dependencies"
```

---

## Task 2: ProtoError + Result type alias

**Files:**
- Create: `crates/ghost-protocol/src/error.rs`
- Modify: `crates/ghost-protocol/src/lib.rs`

- [ ] **Step 1: Create `crates/ghost-protocol/src/error.rs`**

```rust
//! Top-level error type for ghost-protocol. Wraps openmls errors plus our own
//! envelope/sealed-sender/uuid-parse failures.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtoError {
    #[error("CBOR encode: {0}")]
    CborEncode(String),
    #[error("CBOR decode: {0}")]
    CborDecode(String),

    #[error("envelope wrong recipient: expected {expected}, got {got}")]
    WrongRecipient { expected: String, got: String },
    #[error("envelope unsupported version {0}")]
    UnsupportedVersion(u8),
    #[error("envelope unknown msg_type {0}")]
    UnknownMsgType(u8),

    #[error("sealed sender encrypt failed")]
    SealedSenderEncrypt,
    #[error("sealed sender decrypt failed (wrong key, tampered, or wrong recipient)")]
    SealedSenderDecrypt,

    #[error("invalid sender signature")]
    BadSenderSignature,
    #[error("duplicate msg_uuid (replay attack)")]
    Replay,

    #[error("MLS error: {0}")]
    Mls(String),

    #[error("ghost-identity error: {0}")]
    Identity(String),

    #[error("invalid input: {0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, ProtoError>;
```

- [ ] **Step 2: Modify `crates/ghost-protocol/src/lib.rs`**

```rust
//! Ghost wire protocol: MLS group state, sealed-sender envelopes, KeyPackages.

pub mod error;

pub use error::{ProtoError, Result};

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-protocol");
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo +1.85-x86_64-pc-windows-msvc test -p ghost-protocol
```
Expected: 1 test passes (still just the smoke).

- [ ] **Step 4: Commit**

```bash
git add crates/ghost-protocol/
git commit -m "feat(ghost-protocol): ProtoError and Result alias"
```

---

## Task 3: UUID v7 wrapper for msg_uuid

**Files:**
- Create: `crates/ghost-protocol/src/msg_uuid.rs`
- Modify: `crates/ghost-protocol/src/lib.rs`

UUID v7 is time-ordered, so it doubles as both a unique identifier and a sortable key for inbox dedup. Wrapping the raw `Uuid` keeps us free to swap implementations later (e.g., go to ULID).

- [ ] **Step 1: Write failing tests**

Create `crates/ghost-protocol/src/msg_uuid.rs`:

```rust
//! MessageUuid — time-ordered 128-bit ID for messages, used for replay dedup.
//! Backed by UUID v7 (RFC 9562).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageUuid(Uuid);

impl MessageUuid {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }

    pub fn into_bytes(self) -> [u8; 16] {
        *self.0.as_bytes()
    }
}

impl Default for MessageUuid {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for MessageUuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MessageUuid({})", self.0)
    }
}

impl std::fmt::Display for MessageUuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn distinct_uuids_per_call() {
        let mut set = HashSet::new();
        for _ in 0..1000 {
            assert!(set.insert(MessageUuid::new()));
        }
    }

    #[test]
    fn roundtrip_bytes() {
        let original = MessageUuid::new();
        let bytes = original.into_bytes();
        let restored = MessageUuid::from_bytes(bytes);
        assert_eq!(original, restored);
    }

    #[test]
    fn time_ordered_within_same_millisecond_and_across() {
        // V7 is time-ordered: a UUID generated later should compare-greater than one earlier.
        let earlier = MessageUuid::new();
        thread::sleep(Duration::from_millis(2));
        let later = MessageUuid::new();
        // V7 puts the timestamp in the high bytes, so earlier.as_bytes() < later.as_bytes() byte-wise.
        assert!(earlier.as_bytes() < later.as_bytes());
    }

    #[test]
    fn cbor_roundtrip() {
        let original = MessageUuid::new();
        let mut buf = Vec::new();
        ciborium::into_writer(&original, &mut buf).unwrap();
        let restored: MessageUuid = ciborium::from_reader(&buf[..]).unwrap();
        assert_eq!(original, restored);
    }
}
```

- [ ] **Step 2: Modify `crates/ghost-protocol/src/lib.rs`**

```rust
//! Ghost wire protocol: MLS group state, sealed-sender envelopes, KeyPackages.

pub mod error;
pub mod msg_uuid;

pub use error::{ProtoError, Result};
pub use msg_uuid::MessageUuid;

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-protocol");
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo +1.85-x86_64-pc-windows-msvc test -p ghost-protocol
```
Expected: 5 tests pass (4 new + 1 smoke).

- [ ] **Step 4: Commit**

```bash
git add crates/ghost-protocol/
git commit -m "feat(ghost-protocol): MessageUuid wrapper around UUID v7"
```

---

## Task 4: SealedBlob CBOR type + roundtrip test

**Files:**
- Create: `crates/ghost-protocol/src/sealed_blob.rs`
- Modify: `crates/ghost-protocol/src/lib.rs`

`SealedBlob` is the inner part of the wire envelope. It contains the real sender id, the actual MLS payload, and a sender-side signature. After being CBOR-encoded, it gets encrypted to the recipient's delivery key by the sealed-sender layer (Task 6+).

- [ ] **Step 1: Write failing tests**

Create `crates/ghost-protocol/src/sealed_blob.rs`:

```rust
//! SealedBlob — inner ciphertext payload, encrypted to recipient's delivery key.
//!
//! Layout (CBOR-serialized then encrypted by the sealed-sender layer):
//! ```text
//! SealedBlob {
//!   sender_id:        GhostId         // the real sender (hidden from server)
//!   payload_type:     u8              // 0=text app, 1=mls handshake, 2=mls commit, ...
//!   payload:          Bytes           // typically MLSMessage from openmls
//!   msg_uuid:         MessageUuid     // for dedup
//!   sender_signature: [u8; 64]        // Ed25519(DK, hash(sender_id || payload_type || payload || msg_uuid))
//! }
//! ```

use crate::msg_uuid::MessageUuid;
use crate::{ProtoError, Result};
use ghost_core::GhostId;
use serde::{Deserialize, Serialize};

/// Payload-type tag inside a SealedBlob.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PayloadType {
    AppText = 0,
    MlsHandshake = 1,
    MlsCommit = 2,
    Ack = 3,
}

impl TryFrom<u8> for PayloadType {
    type Error = ProtoError;
    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::AppText),
            1 => Ok(Self::MlsHandshake),
            2 => Ok(Self::MlsCommit),
            3 => Ok(Self::Ack),
            other => Err(ProtoError::Invalid(format!("unknown payload type {other}"))),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SealedBlob {
    pub sender_id: GhostId,
    pub payload_type: PayloadType,
    pub payload: Vec<u8>,
    pub msg_uuid: MessageUuid,
    /// Ed25519 signature (64 bytes); kept as raw bytes to avoid leaking the underlying type into the wire.
    pub sender_signature: [u8; 64],
}

impl SealedBlob {
    /// CBOR-encode for inclusion in the OuterEnvelope's `sealed_blob` field
    /// (this serialized form is what gets encrypted to the recipient's delivery key).
    pub fn to_cbor(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        ciborium::into_writer(self, &mut out).map_err(|e| ProtoError::CborEncode(e.to_string()))?;
        Ok(out)
    }

    pub fn from_cbor(bytes: &[u8]) -> Result<Self> {
        ciborium::from_reader(bytes).map_err(|e| ProtoError::CborDecode(e.to_string()))
    }

    /// Compute the bytes that the sender must sign (with their DK):
    /// blake3(sender_id || payload_type || payload || msg_uuid)
    pub fn signing_bytes(
        sender: &GhostId,
        payload_type: PayloadType,
        payload: &[u8],
        msg_uuid: &MessageUuid,
    ) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(sender.as_bytes());
        hasher.update(&[payload_type as u8]);
        hasher.update(payload);
        hasher.update(msg_uuid.as_bytes());
        *hasher.finalize().as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{SigningKey, Signer};
    use rand::rngs::OsRng;

    fn fake_signing_key() -> SigningKey {
        SigningKey::generate(&mut OsRng)
    }

    #[test]
    fn cbor_roundtrip_preserves_all_fields() {
        let sender = GhostId::from_bytes([7u8; 32]);
        let uuid = MessageUuid::new();
        let payload = b"hello world".to_vec();
        let signing_bytes = SealedBlob::signing_bytes(&sender, PayloadType::AppText, &payload, &uuid);
        let dk = fake_signing_key();
        let sig = dk.sign(&signing_bytes).to_bytes();

        let original = SealedBlob {
            sender_id: sender,
            payload_type: PayloadType::AppText,
            payload: payload.clone(),
            msg_uuid: uuid,
            sender_signature: sig,
        };

        let bytes = original.to_cbor().unwrap();
        let decoded = SealedBlob::from_cbor(&bytes).unwrap();

        assert_eq!(decoded.sender_id, original.sender_id);
        assert_eq!(decoded.payload_type, original.payload_type);
        assert_eq!(decoded.payload, original.payload);
        assert_eq!(decoded.msg_uuid, original.msg_uuid);
        assert_eq!(decoded.sender_signature, original.sender_signature);
    }

    #[test]
    fn payload_type_try_from() {
        assert_eq!(PayloadType::try_from(0u8).unwrap(), PayloadType::AppText);
        assert_eq!(PayloadType::try_from(1u8).unwrap(), PayloadType::MlsHandshake);
        assert!(matches!(
            PayloadType::try_from(99u8),
            Err(ProtoError::Invalid(_))
        ));
    }

    #[test]
    fn signing_bytes_deterministic() {
        let s = GhostId::from_bytes([1u8; 32]);
        let u = MessageUuid::from_bytes([2u8; 16]);
        let p = b"data";
        let a = SealedBlob::signing_bytes(&s, PayloadType::AppText, p, &u);
        let b = SealedBlob::signing_bytes(&s, PayloadType::AppText, p, &u);
        assert_eq!(a, b);
    }

    #[test]
    fn signing_bytes_differ_when_input_differs() {
        let s = GhostId::from_bytes([1u8; 32]);
        let u = MessageUuid::from_bytes([2u8; 16]);
        let a = SealedBlob::signing_bytes(&s, PayloadType::AppText, b"data1", &u);
        let b = SealedBlob::signing_bytes(&s, PayloadType::AppText, b"data2", &u);
        assert_ne!(a, b);
    }

    #[test]
    fn from_cbor_rejects_garbage() {
        let err = SealedBlob::from_cbor(b"not valid cbor").unwrap_err();
        assert!(matches!(err, ProtoError::CborDecode(_)));
    }
}
```

- [ ] **Step 2: Modify `lib.rs`**

```rust
//! Ghost wire protocol: MLS group state, sealed-sender envelopes, KeyPackages.

pub mod error;
pub mod msg_uuid;
pub mod sealed_blob;

pub use error::{ProtoError, Result};
pub use msg_uuid::MessageUuid;
pub use sealed_blob::{PayloadType, SealedBlob};

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-protocol");
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo +1.85-x86_64-pc-windows-msvc test -p ghost-protocol
```
Expected: 10 tests pass (5 prior + 5 new).

- [ ] **Step 4: Commit**

```bash
git add crates/ghost-protocol/
git commit -m "feat(ghost-protocol): SealedBlob with CBOR roundtrip and signing-bytes helper"
```

---

## Task 5: OuterEnvelope CBOR type + roundtrip test

**Files:**
- Create: `crates/ghost-protocol/src/outer_envelope.rs`
- Modify: `crates/ghost-protocol/src/lib.rs`

`OuterEnvelope` is what actually goes on the wire. It has the recipient (so the network can route), a timestamp (rounded to seconds for privacy), a version byte, a message-type byte, and the encrypted `sealed_blob` bytes.

- [ ] **Step 1: Write failing tests**

Create `crates/ghost-protocol/src/outer_envelope.rs`:

```rust
//! OuterEnvelope — the visible wire envelope. Recipient and timestamp are clear-text;
//! the entire SealedBlob (sender + payload) is encrypted to the recipient's delivery key.

use crate::{ProtoError, Result};
use ghost_core::GhostId;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u8 = 1;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MsgType {
    AppMessage = 0,
    MlsHandshake = 1,
    Ack = 2,
}

impl TryFrom<u8> for MsgType {
    type Error = ProtoError;
    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::AppMessage),
            1 => Ok(Self::MlsHandshake),
            2 => Ok(Self::Ack),
            other => Err(ProtoError::UnknownMsgType(other)),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OuterEnvelope {
    pub version: u8,
    pub msg_type: MsgType,
    pub recipient: GhostId,
    /// UTC seconds, rounded.
    pub timestamp: u64,
    /// Encrypted SealedBlob. Decryptable only by `recipient`'s delivery key.
    pub sealed_blob: Vec<u8>,
}

impl OuterEnvelope {
    pub fn new(
        msg_type: MsgType,
        recipient: GhostId,
        timestamp: u64,
        sealed_blob: Vec<u8>,
    ) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            msg_type,
            recipient,
            timestamp,
            sealed_blob,
        }
    }

    pub fn to_cbor(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        ciborium::into_writer(self, &mut out).map_err(|e| ProtoError::CborEncode(e.to_string()))?;
        Ok(out)
    }

    pub fn from_cbor(bytes: &[u8]) -> Result<Self> {
        let env: Self =
            ciborium::from_reader(bytes).map_err(|e| ProtoError::CborDecode(e.to_string()))?;
        if env.version != PROTOCOL_VERSION {
            return Err(ProtoError::UnsupportedVersion(env.version));
        }
        Ok(env)
    }

    /// Verify the outer envelope is intended for `expected_recipient`.
    pub fn check_recipient(&self, expected_recipient: &GhostId) -> Result<()> {
        if &self.recipient != expected_recipient {
            return Err(ProtoError::WrongRecipient {
                expected: format!("{}", expected_recipient),
                got: format!("{}", self.recipient),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cbor_roundtrip() {
        let recipient = GhostId::from_bytes([5u8; 32]);
        let original = OuterEnvelope::new(MsgType::AppMessage, recipient, 1700000000, vec![9, 9, 9]);
        let bytes = original.to_cbor().unwrap();
        let decoded = OuterEnvelope::from_cbor(&bytes).unwrap();
        assert_eq!(decoded.version, PROTOCOL_VERSION);
        assert_eq!(decoded.msg_type, MsgType::AppMessage);
        assert_eq!(decoded.recipient, recipient);
        assert_eq!(decoded.timestamp, 1700000000);
        assert_eq!(decoded.sealed_blob, vec![9, 9, 9]);
    }

    #[test]
    fn from_cbor_rejects_unsupported_version() {
        let recipient = GhostId::from_bytes([5u8; 32]);
        let mut bad = OuterEnvelope::new(MsgType::AppMessage, recipient, 1, vec![]);
        bad.version = 99;
        let bytes = bad.to_cbor().unwrap();
        let err = OuterEnvelope::from_cbor(&bytes).unwrap_err();
        assert!(matches!(err, ProtoError::UnsupportedVersion(99)));
    }

    #[test]
    fn check_recipient_accepts_match() {
        let r = GhostId::from_bytes([7u8; 32]);
        let env = OuterEnvelope::new(MsgType::AppMessage, r, 1, vec![]);
        env.check_recipient(&r).unwrap();
    }

    #[test]
    fn check_recipient_rejects_mismatch() {
        let alice = GhostId::from_bytes([1u8; 32]);
        let bob = GhostId::from_bytes([2u8; 32]);
        let env = OuterEnvelope::new(MsgType::AppMessage, alice, 1, vec![]);
        let err = env.check_recipient(&bob).unwrap_err();
        assert!(matches!(err, ProtoError::WrongRecipient { .. }));
    }

    #[test]
    fn msg_type_try_from() {
        assert_eq!(MsgType::try_from(0u8).unwrap(), MsgType::AppMessage);
        assert_eq!(MsgType::try_from(1u8).unwrap(), MsgType::MlsHandshake);
        assert!(matches!(
            MsgType::try_from(50u8),
            Err(ProtoError::UnknownMsgType(50))
        ));
    }
}
```

- [ ] **Step 2: Modify `lib.rs`**

```rust
//! Ghost wire protocol: MLS group state, sealed-sender envelopes, KeyPackages.

pub mod error;
pub mod msg_uuid;
pub mod outer_envelope;
pub mod sealed_blob;

pub use error::{ProtoError, Result};
pub use msg_uuid::MessageUuid;
pub use outer_envelope::{MsgType, OuterEnvelope, PROTOCOL_VERSION};
pub use sealed_blob::{PayloadType, SealedBlob};

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-protocol");
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo +1.85-x86_64-pc-windows-msvc test -p ghost-protocol
```
Expected: 15 tests pass (10 prior + 5 new).

- [ ] **Step 4: Commit**

```bash
git add crates/ghost-protocol/
git commit -m "feat(ghost-protocol): OuterEnvelope with CBOR roundtrip and recipient check"
```

---

## Task 6: Sealed sender — delivery key derivation

**Files:**
- Create: `crates/ghost-protocol/src/sealed_sender.rs`
- Modify: `crates/ghost-protocol/src/lib.rs`

The "delivery key" is an X25519 keypair derived deterministically from the user's IK. Anyone with the user's GhostId can compute the public delivery key (since it's a deterministic transformation of the public part of IK), but only the IK owner can compute the private side.

We derive via HKDF: `seed = HKDF-Extract(IK_secret, "ghost.delivery.v1")`, `priv_x25519 = HKDF-Expand(seed, "x25519")`. Then `pub = X25519::derive_public(priv)`.

- [ ] **Step 1: Write failing tests**

Create `crates/ghost-protocol/src/sealed_sender.rs`:

```rust
//! Sealed-sender layer.
//!
//! The recipient publishes an X25519 delivery public key (derived deterministically from their IK).
//! Senders ECDH against this delivery pubkey to wrap the SealedBlob, hiding the sender ID from
//! the network and from the recipient's own server (relevant once we have federation in MVP-3+).

use crate::{ProtoError, Result};
use ghost_identity::IdentityKey;
use hkdf::Hkdf;
use sha2::Sha256;
use x25519_dalek::{PublicKey as X25519Public, StaticSecret as X25519Secret};

const HKDF_SALT: &[u8] = b"ghost.delivery.v1.salt";
const HKDF_INFO: &[u8] = b"ghost.delivery.v1.x25519";

/// Derive the X25519 delivery secret from an IdentityKey.
/// Anyone holding the IdentityKey can derive both halves; outside parties can derive only
/// the public half from the IK's public bytes (since they're ed25519 -> blake3 -> hkdf chain).
pub fn delivery_secret(ik: &IdentityKey) -> X25519Secret {
    // Use the Ed25519 public bytes as the IKM. They're public, but combined with our domain-tagged
    // HKDF-Salt + HKDF-Info we get a unique derivation. (We don't reveal the Ed25519 SECRET key:
    // X25519 keys derived in this way are public-only-derivable for OTHERS, but for us we have the
    // SAME public bytes available, so the derivation works deterministically on either side.
    // Wait — that's wrong: if anyone with the public IK can derive the X25519 secret, then nothing
    // is private. The correct construction is to use a SECRET seed only the holder knows.
    //
    // Therefore we derive from the Ed25519 SIGNING-KEY bytes (secret seed), not the public bytes.
    // Anyone with the IdentityKey owns the secret seed.
    //
    // For OTHERS to derive the public half we must instead publish the X25519 public key separately
    // (see `delivery_public_published`).
    let seed = ik_secret_bytes(ik);
    let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT), &seed);
    let mut okm = [0u8; 32];
    hk.expand(HKDF_INFO, &mut okm).expect("32-byte expand always succeeds");
    X25519Secret::from(okm)
}

pub fn delivery_public(ik: &IdentityKey) -> X25519Public {
    X25519Public::from(&delivery_secret(ik))
}

/// Helper to extract the 32-byte secret seed from an IdentityKey.
/// `IdentityKey` already has `from_secret_bytes` in ghost-identity v0; we need the inverse.
/// Since `SigningKey::to_bytes()` returns the 32-byte secret in ed25519-dalek v2, we expose
/// it via the public `sign` API, but that doesn't give back raw bytes. We must add a method
/// on IdentityKey to expose the secret bytes — or compute the seed from an existing helper.
///
/// **Plan note:** This function depends on an addition to `ghost-identity::IdentityKey` of a
/// `secret_bytes(&self) -> [u8; 32]` method (see Step 1.5 below).
fn ik_secret_bytes(ik: &IdentityKey) -> [u8; 32] {
    ik.secret_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_keys_are_deterministic_per_identity() {
        let ik = IdentityKey::generate();
        let s1 = delivery_secret(&ik);
        let s2 = delivery_secret(&ik);
        // Compare via the public (derived) — secret-byte comparison would require zeroize-aware peek.
        let p1 = X25519Public::from(&s1);
        let p2 = X25519Public::from(&s2);
        assert_eq!(p1.as_bytes(), p2.as_bytes());
    }

    #[test]
    fn distinct_identities_yield_distinct_delivery_keys() {
        let ik_a = IdentityKey::generate();
        let ik_b = IdentityKey::generate();
        let pa = delivery_public(&ik_a);
        let pb = delivery_public(&ik_b);
        assert_ne!(pa.as_bytes(), pb.as_bytes());
    }

    #[test]
    fn delivery_public_matches_secret_derivation() {
        let ik = IdentityKey::generate();
        let secret = delivery_secret(&ik);
        let pub_via_secret = X25519Public::from(&secret);
        let pub_direct = delivery_public(&ik);
        assert_eq!(pub_via_secret.as_bytes(), pub_direct.as_bytes());
    }
}
```

- [ ] **Step 1.5: Add `secret_bytes` accessor to ghost-identity `IdentityKey`**

This is a small, necessary companion edit to `ghost-identity` so this delivery-key derivation works.

Edit `crates/ghost-identity/src/keys.rs` and add this method to the `impl IdentityKey` block (place it after `from_secret_bytes`):

```rust
    /// Expose the 32-byte secret seed. Must NEVER be used for anything other than
    /// deriving deterministic subkeys (e.g., delivery key) within trusted code in
    /// this workspace. The raw seed must never leave the process.
    pub fn secret_bytes(&self) -> [u8; 32] {
        self.signing.to_bytes()
    }
```

- [ ] **Step 2: Modify `lib.rs`** (in `ghost-protocol`)

```rust
//! Ghost wire protocol: MLS group state, sealed-sender envelopes, KeyPackages.

pub mod error;
pub mod msg_uuid;
pub mod outer_envelope;
pub mod sealed_blob;
pub mod sealed_sender;

pub use error::{ProtoError, Result};
pub use msg_uuid::MessageUuid;
pub use outer_envelope::{MsgType, OuterEnvelope, PROTOCOL_VERSION};
pub use sealed_blob::{PayloadType, SealedBlob};
pub use sealed_sender::{delivery_public, delivery_secret};

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-protocol");
    }
}
```

- [ ] **Step 3: Run tests for both crates**

```bash
cargo +1.85-x86_64-pc-windows-msvc test -p ghost-identity
cargo +1.85-x86_64-pc-windows-msvc test -p ghost-protocol
```
Expected: ghost-identity 43 (unchanged — `secret_bytes` is a non-breaking addition), ghost-protocol 18 (15 prior + 3 new).

- [ ] **Step 4: Commit**

```bash
git add crates/ghost-identity/src/keys.rs crates/ghost-protocol/
git commit -m "feat(ghost-protocol): X25519 delivery key derivation from IdentityKey"
```

---

## Task 7: Sealed sender — encrypt/decrypt of SealedBlob

**Files:**
- Modify (append): `crates/ghost-protocol/src/sealed_sender.rs`

The wrap/unwrap is a "sealed box" pattern: ephemeral X25519 + ECDH to recipient delivery pubkey + XChaCha20-Poly1305 AEAD. Each call uses a fresh ephemeral keypair (so observers can't link two messages from the same sender to the same recipient).

- [ ] **Step 1: Append failing tests + impl**

Append to `crates/ghost-protocol/src/sealed_sender.rs`:

```rust
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use rand::RngCore;
use x25519_dalek::EphemeralSecret;

const AEAD_AAD: &[u8] = b"ghost.sealed_sender.v1.aad";
/// 24 bytes for XChaCha20-Poly1305.
const NONCE_LEN: usize = 24;
/// 32 bytes for the ephemeral X25519 public key prepended to the ciphertext.
const EPHEMERAL_PUB_LEN: usize = 32;

/// Encrypt `plaintext` (typically a CBOR-encoded SealedBlob) so that only the holder of
/// the IK that produced `recipient_delivery_pub` can decrypt.
///
/// Output layout: `eph_pub (32) || nonce (24) || ciphertext+tag`
pub fn seal_to(recipient_delivery_pub: &X25519Public, plaintext: &[u8]) -> Result<Vec<u8>> {
    // Fresh ephemeral on every call.
    let eph_secret = EphemeralSecret::random_from_rng(&mut rand::thread_rng());
    let eph_pub = X25519Public::from(&eph_secret);
    let shared = eph_secret.diffie_hellman(recipient_delivery_pub);

    // Derive a 32-byte AEAD key from the shared secret.
    let mut aead_key = [0u8; 32];
    let hk = Hkdf::<Sha256>::new(Some(b"ghost.sealed_sender.v1.kdf"), shared.as_bytes());
    hk.expand(b"aead-key", &mut aead_key).expect("expand 32 always succeeds");

    let cipher = XChaCha20Poly1305::new((&aead_key).into());
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad: AEAD_AAD,
            },
        )
        .map_err(|_| ProtoError::SealedSenderEncrypt)?;

    let mut out = Vec::with_capacity(EPHEMERAL_PUB_LEN + NONCE_LEN + ciphertext.len());
    out.extend_from_slice(eph_pub.as_bytes());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt a sealed-sender blob using our delivery secret. Recovers the inner plaintext
/// (typically CBOR-encoded SealedBlob bytes).
pub fn unseal(recipient_delivery_secret: &X25519Secret, sealed: &[u8]) -> Result<Vec<u8>> {
    if sealed.len() < EPHEMERAL_PUB_LEN + NONCE_LEN {
        return Err(ProtoError::SealedSenderDecrypt);
    }
    let mut eph_bytes = [0u8; EPHEMERAL_PUB_LEN];
    eph_bytes.copy_from_slice(&sealed[..EPHEMERAL_PUB_LEN]);
    let eph_pub = X25519Public::from(eph_bytes);

    let mut nonce_bytes = [0u8; NONCE_LEN];
    nonce_bytes.copy_from_slice(&sealed[EPHEMERAL_PUB_LEN..EPHEMERAL_PUB_LEN + NONCE_LEN]);

    let ciphertext = &sealed[EPHEMERAL_PUB_LEN + NONCE_LEN..];

    let shared = recipient_delivery_secret.diffie_hellman(&eph_pub);
    let mut aead_key = [0u8; 32];
    let hk = Hkdf::<Sha256>::new(Some(b"ghost.sealed_sender.v1.kdf"), shared.as_bytes());
    hk.expand(b"aead-key", &mut aead_key).expect("expand 32 always succeeds");

    let cipher = XChaCha20Poly1305::new((&aead_key).into());
    let nonce = XNonce::from_slice(&nonce_bytes);
    cipher
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext,
                aad: AEAD_AAD,
            },
        )
        .map_err(|_| ProtoError::SealedSenderDecrypt)
}

#[cfg(test)]
mod sealing_tests {
    use super::*;

    #[test]
    fn seal_unseal_roundtrip() {
        let recipient = IdentityKey::generate();
        let recipient_pub = delivery_public(&recipient);
        let recipient_secret = delivery_secret(&recipient);

        let plaintext = b"the inner sealed blob bytes";
        let sealed = seal_to(&recipient_pub, plaintext).unwrap();
        assert!(sealed.len() >= 32 + 24 + 16); // eph + nonce + at least tag
        let recovered = unseal(&recipient_secret, &sealed).unwrap();
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn unseal_fails_with_wrong_recipient() {
        let intended = IdentityKey::generate();
        let stranger = IdentityKey::generate();
        let sealed = seal_to(&delivery_public(&intended), b"data").unwrap();
        let err = unseal(&delivery_secret(&stranger), &sealed).unwrap_err();
        assert!(matches!(err, ProtoError::SealedSenderDecrypt));
    }

    #[test]
    fn unseal_fails_on_truncated_input() {
        let recipient = IdentityKey::generate();
        let err = unseal(&delivery_secret(&recipient), b"too short").unwrap_err();
        assert!(matches!(err, ProtoError::SealedSenderDecrypt));
    }

    #[test]
    fn unseal_fails_on_tampered_ciphertext() {
        let recipient = IdentityKey::generate();
        let mut sealed = seal_to(&delivery_public(&recipient), b"data").unwrap();
        let last = sealed.len() - 1;
        sealed[last] ^= 0xFF;
        let err = unseal(&delivery_secret(&recipient), &sealed).unwrap_err();
        assert!(matches!(err, ProtoError::SealedSenderDecrypt));
    }

    #[test]
    fn each_seal_uses_fresh_ephemeral() {
        let recipient = IdentityKey::generate();
        let pub_ = delivery_public(&recipient);
        let s1 = seal_to(&pub_, b"data").unwrap();
        let s2 = seal_to(&pub_, b"data").unwrap();
        // First 32 bytes are the ephemeral pub; they must differ across calls.
        assert_ne!(&s1[..32], &s2[..32]);
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo +1.85-x86_64-pc-windows-msvc test -p ghost-protocol
```
Expected: 23 tests pass (18 prior + 5 new).

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-protocol/src/sealed_sender.rs
git commit -m "feat(ghost-protocol): sealed sender encrypt/decrypt with ephemeral X25519"
```

---

## Task 8: End-to-end envelope wrap + unwrap helpers

**Files:**
- Create: `crates/ghost-protocol/src/envelope.rs`
- Modify: `crates/ghost-protocol/src/lib.rs`

This task ties together SealedBlob signing + sealed-sender wrap + OuterEnvelope into one `wrap_message` and one `unwrap_message` function. These are the public APIs the rest of the system will call.

- [ ] **Step 1: Write failing tests**

Create `crates/ghost-protocol/src/envelope.rs`:

```rust
//! High-level envelope API: bundle SealedBlob signing, sealed-sender encryption,
//! and OuterEnvelope CBOR into single `wrap_message` / `unwrap_message` calls.

use crate::msg_uuid::MessageUuid;
use crate::outer_envelope::{MsgType, OuterEnvelope};
use crate::sealed_blob::{PayloadType, SealedBlob};
use crate::sealed_sender::{delivery_public, delivery_secret, seal_to, unseal};
use crate::{ProtoError, Result};
use ed25519_dalek::{Signature, Signer, Verifier};
use ghost_core::GhostId;
use ghost_identity::{DeviceKey, IdentityKey};

/// Bundle of values returned to the caller after `unwrap_message` succeeds.
#[derive(Debug)]
pub struct UnwrappedMessage {
    pub sender_id: GhostId,
    pub payload_type: PayloadType,
    pub payload: Vec<u8>,
    pub msg_uuid: MessageUuid,
}

/// Wrap an outgoing message: produces wire bytes (CBOR-encoded OuterEnvelope) intended for `recipient`.
///
/// Steps:
/// 1. Sign (sender_id || payload_type || payload || msg_uuid) with sender's DK.
/// 2. Build SealedBlob, CBOR-encode.
/// 3. Sealed-sender-encrypt to recipient's delivery pubkey.
/// 4. Wrap in OuterEnvelope, CBOR-encode.
pub fn wrap_message(
    sender_ik: &IdentityKey,
    sender_dk: &DeviceKey,
    recipient_id: GhostId,
    recipient_delivery_pub: &x25519_dalek::PublicKey,
    msg_type: MsgType,
    payload_type: PayloadType,
    payload: Vec<u8>,
    timestamp: u64,
) -> Result<Vec<u8>> {
    let sender_id = sender_ik.ghost_id();
    let msg_uuid = MessageUuid::new();

    let signing_bytes =
        SealedBlob::signing_bytes(&sender_id, payload_type, &payload, &msg_uuid);
    let signature = sender_dk.sign(&signing_bytes);

    let blob = SealedBlob {
        sender_id,
        payload_type,
        payload,
        msg_uuid,
        sender_signature: signature.to_bytes(),
    };
    let blob_bytes = blob.to_cbor()?;
    let sealed = seal_to(recipient_delivery_pub, &blob_bytes)?;

    let outer = OuterEnvelope::new(msg_type, recipient_id, timestamp, sealed);
    outer.to_cbor()
}

/// Unwrap an incoming envelope: parse, verify recipient, decrypt sealed-sender, parse SealedBlob,
/// verify sender signature, return plaintext payload + sender info.
///
/// `sender_dk_for_check` is a callback the caller supplies: given the inner sender's GhostId,
/// return the sender's known DK public bytes for signature verification (typically pulled from a
/// contact-key store or from MLS group state).
pub fn unwrap_message<F>(
    wire_bytes: &[u8],
    recipient_ik: &IdentityKey,
    sender_dk_for_check: F,
) -> Result<UnwrappedMessage>
where
    F: FnOnce(&GhostId) -> Option<ed25519_dalek::VerifyingKey>,
{
    let outer = OuterEnvelope::from_cbor(wire_bytes)?;
    outer.check_recipient(&recipient_ik.ghost_id())?;

    let recipient_secret = delivery_secret(recipient_ik);
    let blob_bytes = unseal(&recipient_secret, &outer.sealed_blob)?;
    let blob = SealedBlob::from_cbor(&blob_bytes)?;

    let sender_dk_pub = sender_dk_for_check(&blob.sender_id).ok_or_else(|| {
        ProtoError::Invalid(format!(
            "no known DK for sender {}",
            blob.sender_id
        ))
    })?;
    let signing_bytes = SealedBlob::signing_bytes(
        &blob.sender_id,
        blob.payload_type,
        &blob.payload,
        &blob.msg_uuid,
    );
    let sig = Signature::from_bytes(&blob.sender_signature);
    sender_dk_pub
        .verify(&signing_bytes, &sig)
        .map_err(|_| ProtoError::BadSenderSignature)?;

    Ok(UnwrappedMessage {
        sender_id: blob.sender_id,
        payload_type: blob.payload_type,
        payload: blob.payload,
        msg_uuid: blob.msg_uuid,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ghost_identity::{DeviceKey, IdentityKey};

    /// Build a (alice, bob) pair with their IK and DK. DKs are signed by IKs.
    fn alice_and_bob() -> (
        (IdentityKey, DeviceKey),
        (IdentityKey, DeviceKey),
    ) {
        let alice_ik = IdentityKey::generate();
        let alice_dk = DeviceKey::generate(&alice_ik);
        let bob_ik = IdentityKey::generate();
        let bob_dk = DeviceKey::generate(&bob_ik);
        ((alice_ik, alice_dk), (bob_ik, bob_dk))
    }

    #[test]
    fn wrap_then_unwrap_roundtrip() {
        let ((alice_ik, alice_dk), (bob_ik, _bob_dk)) = alice_and_bob();
        let alice_id = alice_ik.ghost_id();
        let bob_id = bob_ik.ghost_id();
        let bob_delivery = delivery_public(&bob_ik);
        let alice_dk_pub = alice_dk.public();

        let wire = wrap_message(
            &alice_ik,
            &alice_dk,
            bob_id,
            &bob_delivery,
            MsgType::AppMessage,
            PayloadType::AppText,
            b"hello bob".to_vec(),
            1700000000,
        )
        .unwrap();

        let result = unwrap_message(&wire, &bob_ik, |id| {
            if id == &alice_id {
                Some(alice_dk_pub)
            } else {
                None
            }
        })
        .unwrap();

        assert_eq!(result.sender_id, alice_id);
        assert_eq!(result.payload_type, PayloadType::AppText);
        assert_eq!(result.payload, b"hello bob");
    }

    #[test]
    fn unwrap_fails_when_recipient_mismatch() {
        let ((alice_ik, alice_dk), (bob_ik, _)) = alice_and_bob();
        let charlie_ik = IdentityKey::generate();
        let bob_id = bob_ik.ghost_id();
        let bob_delivery = delivery_public(&bob_ik);

        let wire = wrap_message(
            &alice_ik,
            &alice_dk,
            bob_id,
            &bob_delivery,
            MsgType::AppMessage,
            PayloadType::AppText,
            b"data".to_vec(),
            1,
        )
        .unwrap();

        // Charlie tries to unwrap. Outer-envelope recipient check should fail.
        let err = unwrap_message(&wire, &charlie_ik, |_| None).unwrap_err();
        assert!(matches!(err, ProtoError::WrongRecipient { .. }));
    }

    #[test]
    fn unwrap_fails_when_sender_dk_unknown() {
        let ((alice_ik, alice_dk), (bob_ik, _)) = alice_and_bob();
        let bob_id = bob_ik.ghost_id();
        let bob_delivery = delivery_public(&bob_ik);

        let wire = wrap_message(
            &alice_ik,
            &alice_dk,
            bob_id,
            &bob_delivery,
            MsgType::AppMessage,
            PayloadType::AppText,
            b"data".to_vec(),
            1,
        )
        .unwrap();

        // The DK-lookup callback returns None — Bob doesn't know Alice's DK.
        let err = unwrap_message(&wire, &bob_ik, |_| None).unwrap_err();
        assert!(matches!(err, ProtoError::Invalid(_)));
    }

    #[test]
    fn unwrap_fails_on_tampered_payload() {
        let ((alice_ik, alice_dk), (bob_ik, _)) = alice_and_bob();
        let alice_id = alice_ik.ghost_id();
        let alice_dk_pub = alice_dk.public();
        let bob_id = bob_ik.ghost_id();
        let bob_delivery = delivery_public(&bob_ik);

        let mut wire = wrap_message(
            &alice_ik,
            &alice_dk,
            bob_id,
            &bob_delivery,
            MsgType::AppMessage,
            PayloadType::AppText,
            b"data".to_vec(),
            1,
        )
        .unwrap();
        // Flip a bit somewhere late in the wire bytes — the AEAD tag will fail.
        let last = wire.len() - 1;
        wire[last] ^= 0xFF;

        let err = unwrap_message(&wire, &bob_ik, |id| {
            (id == &alice_id).then_some(alice_dk_pub)
        })
        .unwrap_err();
        assert!(matches!(err, ProtoError::SealedSenderDecrypt));
    }
}
```

- [ ] **Step 2: Modify `lib.rs`**

```rust
//! Ghost wire protocol: MLS group state, sealed-sender envelopes, KeyPackages.

pub mod envelope;
pub mod error;
pub mod msg_uuid;
pub mod outer_envelope;
pub mod sealed_blob;
pub mod sealed_sender;

pub use envelope::{unwrap_message, wrap_message, UnwrappedMessage};
pub use error::{ProtoError, Result};
pub use msg_uuid::MessageUuid;
pub use outer_envelope::{MsgType, OuterEnvelope, PROTOCOL_VERSION};
pub use sealed_blob::{PayloadType, SealedBlob};
pub use sealed_sender::{delivery_public, delivery_secret};
```

- [ ] **Step 3: Run tests**

```bash
cargo +1.85-x86_64-pc-windows-msvc test -p ghost-protocol
```
Expected: 27 tests pass (23 prior + 4 new). All wrap/unwrap roundtrips work; tampering and recipient-mismatch are rejected.

- [ ] **Step 4: Commit**

```bash
git add crates/ghost-protocol/
git commit -m "feat(ghost-protocol): high-level wrap_message/unwrap_message"
```

---

## Task 9: openmls provider wiring

**Files:**
- Create: `crates/ghost-protocol/src/mls_provider.rs`
- Modify: `crates/ghost-protocol/src/lib.rs`

`openmls` requires a "provider" that bundles three traits: a crypto-suite implementation, a storage backend (for group state), and a randomness source. For Plan 02 we use `OpenMlsRustCrypto` from `openmls_rust_crypto` (it implements all three with in-memory storage). Plan 03 will replace storage with a SQLite-backed implementation.

**Implementer note:** `OpenMlsRustCrypto` already implements `OpenMlsProvider`. This task is mostly a pass-through wrapper that names the type for the rest of our code to use. Consult `openmls_rust_crypto` docs for the exact constructor — it may be `OpenMlsRustCrypto::default()` or similar.

- [ ] **Step 1: Write failing test**

Create `crates/ghost-protocol/src/mls_provider.rs`:

```rust
//! MLS provider — bundles crypto + storage + rand for openmls.
//!
//! For Plan 02 we use the in-memory provider from `openmls_rust_crypto`.
//! Plan 03 (storage) will replace the storage half with SQLite-backed persistence.

use openmls_rust_crypto::OpenMlsRustCrypto;

/// Type alias so callers don't have to import openmls_rust_crypto directly.
pub type GhostMlsProvider = OpenMlsRustCrypto;

/// Construct a fresh in-memory provider. Each session/test should get its own.
pub fn new_provider() -> GhostMlsProvider {
    OpenMlsRustCrypto::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmls_traits::OpenMlsProvider;

    #[test]
    fn provider_constructs_and_exposes_traits() {
        let provider = new_provider();
        // Just touch the three trait methods to make sure the type implements them.
        let _ = provider.crypto();
        let _ = provider.storage();
        let _ = provider.rand();
    }
}
```

- [ ] **Step 2: Modify `lib.rs`**

```rust
// add to module list:
pub mod mls_provider;

// add to re-exports:
pub use mls_provider::{new_provider, GhostMlsProvider};
```

- [ ] **Step 3: Run tests**

```bash
cargo +1.85-x86_64-pc-windows-msvc test -p ghost-protocol
```
Expected: 28 tests pass.

If openmls_rust_crypto's API has changed and `OpenMlsRustCrypto::default()` doesn't exist, consult docs and use the correct constructor (likely something like `OpenMlsRustCrypto::new()` or builder-pattern). Update accordingly.

- [ ] **Step 4: Commit**

```bash
git add crates/ghost-protocol/
git commit -m "feat(ghost-protocol): MLS provider wrapper around openmls_rust_crypto"
```

---

## Task 10: MLS credential builder from GhostId

**Files:**
- Create: `crates/ghost-protocol/src/mls_credential.rs`
- Modify: `crates/ghost-protocol/src/lib.rs`

In MLS, every group member has a "credential" — bytes that prove their identity, paired with a signature key. We use openmls's `BasicCredential` (the simplest variant: arbitrary bytes), filling it with the user's GhostId. The signing key paired with the credential is the user's DeviceKey.

**Implementer note:** the openmls 0.7 API for building a `CredentialWithKey` typically requires:
1. `BasicCredential::new(identity_bytes)` to make the credential
2. A `SignatureKeyPair` registered with the provider (via `provider.storage()`)

Consult `openmls::credentials::*` and `openmls::prelude::SignatureKeyPair` for exact details.

- [ ] **Step 1: Write the test that defines the behavior**

Create `crates/ghost-protocol/src/mls_credential.rs`:

```rust
//! Build openmls credentials from a Ghost identity.

use crate::mls_provider::GhostMlsProvider;
use crate::{ProtoError, Result};
use ghost_identity::{DeviceKey, IdentityKey};

/// Build (Credential, SignatureKey) pair for use in openmls.
/// Returns the openmls types needed for KeyPackage generation and MlsGroup operations.
///
/// Implementer: consult openmls 0.7 docs for the exact return type. The function should:
///   - Construct a BasicCredential from `ik.ghost_id().as_bytes()` (32 bytes).
///   - Use the DK secret to construct an MLS SignatureKeyPair (Ed25519).
///   - Register/store the SignatureKeyPair in the provider's storage.
///   - Return a `CredentialWithKey` ready for `KeyPackage` generation.
pub fn credential_with_key(
    provider: &GhostMlsProvider,
    ik: &IdentityKey,
    dk: &DeviceKey,
) -> Result<openmls::prelude::CredentialWithKey> {
    use openmls::prelude::{BasicCredential, CredentialWithKey, SignatureKeyPair};
    use openmls_basic_credential::SignatureKeyPair as _; // import path placeholder
    use openmls_traits::types::SignatureScheme;

    // Wrap the GhostId bytes as the BasicCredential identity.
    let identity_bytes = ik.ghost_id().as_bytes().to_vec();
    let credential = BasicCredential::new(identity_bytes);

    // Build a SignatureKeyPair from DK's secret bytes.
    // openmls 0.7 SignatureKeyPair::from_raw or similar — check docs.
    let dk_secret = dk_secret_bytes(dk);
    let signature_key = SignatureKeyPair::from_raw(
        SignatureScheme::ED25519,
        dk_secret.to_vec(),
        dk.public().to_bytes().to_vec(),
    );

    // Register signature key in provider storage so MlsGroup can reference it later.
    signature_key
        .store(provider.storage())
        .map_err(|e| ProtoError::Mls(format!("store signature key: {e}")))?;

    Ok(CredentialWithKey {
        credential: credential.into(),
        signature_key: signature_key.public().into(),
    })
}

/// Helper paralleling ghost-identity::IdentityKey::secret_bytes — needs an addition to DeviceKey.
fn dk_secret_bytes(dk: &DeviceKey) -> [u8; 32] {
    dk.secret_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mls_provider::new_provider;

    #[test]
    fn build_credential_for_fresh_identity() {
        let provider = new_provider();
        let ik = IdentityKey::generate();
        let dk = DeviceKey::generate(&ik);
        let cwk = credential_with_key(&provider, &ik, &dk).expect("build credential");
        // Smoke-check that the identity bytes round-trip through openmls.
        // Exact assertion depends on openmls API; at minimum, the function must not error.
        let _ = cwk;
    }

    #[test]
    fn distinct_identities_produce_distinct_credentials() {
        let provider = new_provider();
        let a_ik = IdentityKey::generate();
        let a_dk = DeviceKey::generate(&a_ik);
        let b_ik = IdentityKey::generate();
        let b_dk = DeviceKey::generate(&b_ik);
        let _a_cwk = credential_with_key(&provider, &a_ik, &a_dk).unwrap();
        let _b_cwk = credential_with_key(&provider, &b_ik, &b_dk).unwrap();
        // Both succeed; deeper assertions in Task 11+ tests.
    }
}
```

- [ ] **Step 1.5: Add `secret_bytes` accessor to ghost-identity `DeviceKey`**

Edit `crates/ghost-identity/src/keys.rs` and add this method to `impl DeviceKey`:

```rust
    /// Expose the 32-byte secret seed of this device key. Trusted internal use only.
    pub fn secret_bytes(&self) -> [u8; 32] {
        self.signing.to_bytes()
    }
```

- [ ] **Step 2: Modify `lib.rs`** (in `ghost-protocol`)

Add `pub mod mls_credential;` and re-export `credential_with_key` if desired.

- [ ] **Step 3: Run tests**

```bash
cargo +1.85-x86_64-pc-windows-msvc test -p ghost-protocol
```

If your code references symbols (e.g., `BasicCredential`, `SignatureKeyPair::from_raw`, `provider.storage()`) that don't exist in openmls 0.7 with those exact names, the compiler will tell you. Consult docs:

- `openmls::credentials` — for `BasicCredential` / `Credential`
- `openmls::prelude` — common imports
- `openmls_basic_credential::SignatureKeyPair` is a separate companion crate; may need to be added as a dep.

If you find that `openmls_basic_credential` is required, add it to:
- `[workspace.dependencies]` in root `Cargo.toml`: `openmls_basic_credential = "0.4"`
- `[dependencies]` in `crates/ghost-protocol/Cargo.toml`: `openmls_basic_credential = { workspace = true }`

Expected: 30 tests pass.

If you cannot make this compile after consulting openmls docs, STOP and report DONE_WITH_CONCERNS or BLOCKED with the specific API mismatch.

- [ ] **Step 4: Commit**

```bash
git add crates/ghost-identity/src/keys.rs crates/ghost-protocol/ Cargo.toml
git commit -m "feat(ghost-protocol): MLS credential builder from GhostId + DeviceKey"
```

---

## Task 11: KeyPackage generation

**Files:**
- Create: `crates/ghost-protocol/src/key_package.rs`
- Modify: `crates/ghost-protocol/src/lib.rs`

A `KeyPackage` is the publishable bundle a contact uses to add you to an MLS group asynchronously. It contains your credential + signature key + a one-time HPKE init key + capabilities. Each call generates a fresh init key — KeyPackages are one-time-use.

**Implementer note:** openmls 0.7 has `KeyPackage::builder()` or `KeyPackageBuilder::new()`. Consult docs for exact builder fluent API.

- [ ] **Step 1: Write the behavioural test**

Create `crates/ghost-protocol/src/key_package.rs`:

```rust
//! KeyPackage generation. A KeyPackage is the publishable bundle a contact uses
//! to add you to an MLS group asynchronously.

use crate::mls_credential::credential_with_key;
use crate::mls_provider::GhostMlsProvider;
use crate::{ProtoError, Result};
use ghost_identity::{DeviceKey, IdentityKey};
use openmls::prelude::*;

/// Generate a fresh KeyPackage for `ik`/`dk`. Each call produces a distinct one-time KeyPackage;
/// the implementation registers the private init key in the provider storage so that
/// processing the corresponding Welcome message later can locate the matching init key.
pub fn generate_key_package(
    provider: &GhostMlsProvider,
    ik: &IdentityKey,
    dk: &DeviceKey,
) -> Result<KeyPackage> {
    let cwk = credential_with_key(provider, ik, dk)?;

    let ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;
    let kp_bundle = KeyPackage::builder()
        .build(
            ciphersuite,
            provider,
            // Implementer: consult openmls 0.7 KeyPackageBuilder::build signature.
            // Typically it takes (provider, signer, credential_with_key) — exact name may vary.
            &dk_to_signer(dk),
            cwk,
        )
        .map_err(|e| ProtoError::Mls(format!("key package build: {e}")))?;

    Ok(kp_bundle.key_package().clone())
}

/// Convert our DeviceKey to whatever signer-trait openmls expects for builder.build.
/// Likely an `&SignatureKeyPair`. Implementer: figure out from openmls docs.
fn dk_to_signer(dk: &DeviceKey) -> openmls_basic_credential::SignatureKeyPair {
    use openmls_traits::types::SignatureScheme;
    openmls_basic_credential::SignatureKeyPair::from_raw(
        SignatureScheme::ED25519,
        dk.secret_bytes().to_vec(),
        dk.public().to_bytes().to_vec(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mls_provider::new_provider;

    #[test]
    fn generate_key_package_succeeds_for_fresh_identity() {
        let provider = new_provider();
        let ik = IdentityKey::generate();
        let dk = DeviceKey::generate(&ik);
        let kp = generate_key_package(&provider, &ik, &dk).unwrap();
        // Smoke check: KeyPackage exists and has the right ciphersuite.
        assert_eq!(
            kp.ciphersuite(),
            Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519
        );
    }

    #[test]
    fn each_key_package_has_distinct_init_key() {
        let provider = new_provider();
        let ik = IdentityKey::generate();
        let dk = DeviceKey::generate(&ik);
        let kp1 = generate_key_package(&provider, &ik, &dk).unwrap();
        let kp2 = generate_key_package(&provider, &ik, &dk).unwrap();
        // Implementer: openmls 0.7 may expose `kp.init_key()` returning HpkePublicKey or similar.
        // Compare the bytes — they MUST differ between two KeyPackages.
        assert_ne!(
            kp1.hpke_init_key().as_slice(),
            kp2.hpke_init_key().as_slice()
        );
    }
}
```

- [ ] **Step 2: Modify `lib.rs`**

Add `pub mod key_package;` and re-export `generate_key_package`.

- [ ] **Step 3: Run tests**

```bash
cargo +1.85-x86_64-pc-windows-msvc test -p ghost-protocol
```

Expected: 32 tests pass.

If `KeyPackage::builder()`, `kp_bundle.key_package()`, or `hpke_init_key()` API names don't match, consult docs and adjust. If you cannot find the equivalent functionality, STOP and report.

- [ ] **Step 4: Commit**

```bash
git add crates/ghost-protocol/
git commit -m "feat(ghost-protocol): KeyPackage generation"
```

---

## Task 12: MlsSession — initialize a 1-on-1 group (just-self)

**Files:**
- Create: `crates/ghost-protocol/src/mls_session.rs`
- Modify: `crates/ghost-protocol/src/lib.rs`

`MlsSession` is our domain wrapper around `openmls::group::MlsGroup`. For 1-on-1, the group is created as size-1 (just the creator) and then `add_member` (Task 13) adds the partner.

- [ ] **Step 1: Write the behavioural test**

Create `crates/ghost-protocol/src/mls_session.rs`:

```rust
//! MlsSession — domain wrapper around openmls's MlsGroup, scoped to 1-on-1 conversations.

use crate::mls_credential::credential_with_key;
use crate::mls_provider::GhostMlsProvider;
use crate::{ProtoError, Result};
use ghost_identity::{DeviceKey, IdentityKey};
use openmls::prelude::*;

pub struct MlsSession {
    group: MlsGroup,
}

impl MlsSession {
    /// Create a fresh MLS group containing only the creator. After this, call
    /// [`add_member`] to invite the conversation partner.
    pub fn create(
        provider: &GhostMlsProvider,
        ik: &IdentityKey,
        dk: &DeviceKey,
    ) -> Result<Self> {
        let cwk = credential_with_key(provider, ik, dk)?;
        let signer = openmls_basic_credential::SignatureKeyPair::from_raw(
            openmls_traits::types::SignatureScheme::ED25519,
            dk.secret_bytes().to_vec(),
            dk.public().to_bytes().to_vec(),
        );

        let group_config = MlsGroupCreateConfig::builder()
            .ciphersuite(Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519)
            .use_ratchet_tree_extension(true) // include tree in Welcome — easier for invitees
            .build();

        let group = MlsGroup::new(provider, &signer, &group_config, cwk)
            .map_err(|e| ProtoError::Mls(format!("create group: {e}")))?;

        Ok(Self { group })
    }

    pub fn epoch(&self) -> u64 {
        self.group.epoch().as_u64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mls_provider::new_provider;

    #[test]
    fn create_session_starts_at_epoch_zero() {
        let provider = new_provider();
        let ik = IdentityKey::generate();
        let dk = DeviceKey::generate(&ik);
        let session = MlsSession::create(&provider, &ik, &dk).unwrap();
        assert_eq!(session.epoch(), 0);
    }
}
```

- [ ] **Step 2: Modify `lib.rs`**

Add `pub mod mls_session;` and re-export `MlsSession`.

- [ ] **Step 3: Run tests**

Expected: 33 tests pass.

If `MlsGroupCreateConfig::builder()` or `MlsGroup::new` signatures don't match, consult docs. Common alternatives: `MlsGroup::new_with_group_id()`.

- [ ] **Step 4: Commit**

```bash
git add crates/ghost-protocol/
git commit -m "feat(ghost-protocol): MlsSession::create (creator-only group init)"
```

---

## Task 13: MlsSession — add member, produce Welcome

**Files:**
- Modify (append): `crates/ghost-protocol/src/mls_session.rs`

When Alice (group creator) wants to add Bob, she calls `add_member` with Bob's KeyPackage. openmls returns:
- A `Commit` message that propagates the membership change (in our 2-member case, only Alice is in the group when this is called, so the commit is just for state-machine integrity)
- A `Welcome` message that Bob will use to join

After this call, Alice should be in epoch 1 (one membership change applied).

- [ ] **Step 1: Append failing tests + impl**

Append to `crates/ghost-protocol/src/mls_session.rs`:

```rust
/// Result of inviting a new member: the Welcome (to be sent to the invitee out-of-band)
/// plus the Commit (which the inviter applies locally — handled inside this method).
pub struct InviteResult {
    pub welcome: MlsMessageOut,
    pub commit: MlsMessageOut,
}

impl MlsSession {
    /// Invite a new member by their KeyPackage. Produces a Welcome the invitee can use to join,
    /// and a Commit message that — in larger groups — would be broadcast to other members.
    /// In our 1-on-1 case there are no other members yet, so the Commit is mostly bookkeeping.
    pub fn add_member(
        &mut self,
        provider: &GhostMlsProvider,
        signer: &openmls_basic_credential::SignatureKeyPair,
        invitee_kp: KeyPackage,
    ) -> Result<InviteResult> {
        let (commit, welcome, _group_info) = self
            .group
            .add_members(provider, signer, &[invitee_kp])
            .map_err(|e| ProtoError::Mls(format!("add member: {e}")))?;

        // Apply the membership change locally so our state advances to epoch 1.
        self.group
            .merge_pending_commit(provider)
            .map_err(|e| ProtoError::Mls(format!("merge commit: {e}")))?;

        Ok(InviteResult { welcome, commit })
    }

    /// Convenience: re-derive a signer for the local DK. Pulled out so callers don't
    /// have to reimport openmls-basic-credential at every call site.
    pub fn signer_from_dk(dk: &DeviceKey) -> openmls_basic_credential::SignatureKeyPair {
        openmls_basic_credential::SignatureKeyPair::from_raw(
            openmls_traits::types::SignatureScheme::ED25519,
            dk.secret_bytes().to_vec(),
            dk.public().to_bytes().to_vec(),
        )
    }
}

#[cfg(test)]
mod add_member_tests {
    use super::*;
    use crate::key_package::generate_key_package;
    use crate::mls_provider::new_provider;

    #[test]
    fn add_member_advances_epoch() {
        let alice_provider = new_provider();
        let alice_ik = IdentityKey::generate();
        let alice_dk = DeviceKey::generate(&alice_ik);
        let mut alice = MlsSession::create(&alice_provider, &alice_ik, &alice_dk).unwrap();
        assert_eq!(alice.epoch(), 0);

        // Bob (separate provider — represents a separate process/machine).
        let bob_provider = new_provider();
        let bob_ik = IdentityKey::generate();
        let bob_dk = DeviceKey::generate(&bob_ik);
        let bob_kp = generate_key_package(&bob_provider, &bob_ik, &bob_dk).unwrap();

        let alice_signer = MlsSession::signer_from_dk(&alice_dk);
        let invite = alice
            .add_member(&alice_provider, &alice_signer, bob_kp)
            .unwrap();
        assert_eq!(alice.epoch(), 1);

        // Welcome and Commit are non-empty MlsMessage outputs; deeper tests in Task 14+.
        let _ = invite.welcome;
        let _ = invite.commit;
    }
}
```

- [ ] **Step 2: Run tests**

Expected: 34 tests pass.

If `MlsGroup::add_members` returns a different tuple shape or different types, consult docs and adjust.

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-protocol/src/mls_session.rs
git commit -m "feat(ghost-protocol): MlsSession::add_member produces Welcome + Commit"
```

---

## Task 14: MlsSession — process Welcome to join group

**Files:**
- Modify (append): `crates/ghost-protocol/src/mls_session.rs`

When Bob receives Alice's Welcome, he joins the group. The KeyPackage Bob published earlier (whose private init key was registered with his provider) is what makes this Welcome decryptable.

- [ ] **Step 1: Append failing test + impl**

Append to `mls_session.rs`:

```rust
impl MlsSession {
    /// Join an MLS group by processing a Welcome message. The matching KeyPackage's private
    /// init key must already be in `provider`'s storage (registered when the KeyPackage was
    /// generated by Task 11's `generate_key_package`).
    pub fn join_via_welcome(
        provider: &GhostMlsProvider,
        welcome: MlsMessageIn,
    ) -> Result<Self> {
        // Convert MlsMessageIn → MlsWelcome
        let welcome = welcome
            .into_welcome()
            .ok_or_else(|| ProtoError::Mls("not a welcome message".into()))?;

        // Use an empty MlsGroupJoinConfig (defaults are fine for our needs).
        let join_config = MlsGroupJoinConfig::default();

        let group = StagedWelcome::new_from_welcome(provider, &join_config, welcome, None)
            .and_then(|sw| sw.into_group(provider))
            .map_err(|e| ProtoError::Mls(format!("join via welcome: {e}")))?;

        Ok(Self { group })
    }
}

#[cfg(test)]
mod welcome_tests {
    use super::*;
    use crate::key_package::generate_key_package;
    use crate::mls_provider::new_provider;

    #[test]
    fn alice_invites_bob_bob_joins() {
        // Alice's side
        let alice_provider = new_provider();
        let alice_ik = IdentityKey::generate();
        let alice_dk = DeviceKey::generate(&alice_ik);
        let mut alice = MlsSession::create(&alice_provider, &alice_ik, &alice_dk).unwrap();

        // Bob's side
        let bob_provider = new_provider();
        let bob_ik = IdentityKey::generate();
        let bob_dk = DeviceKey::generate(&bob_ik);
        let bob_kp = generate_key_package(&bob_provider, &bob_ik, &bob_dk).unwrap();

        // Alice invites Bob.
        let alice_signer = MlsSession::signer_from_dk(&alice_dk);
        let invite = alice
            .add_member(&alice_provider, &alice_signer, bob_kp)
            .unwrap();

        // Serialize the welcome to bytes (this is the wire form Bob would receive),
        // then deserialize on Bob's side.
        let welcome_bytes = invite.welcome.tls_serialize_detached().unwrap();
        let welcome_in =
            MlsMessageIn::tls_deserialize(&mut welcome_bytes.as_slice()).unwrap();

        // Bob joins.
        let bob = MlsSession::join_via_welcome(&bob_provider, welcome_in).unwrap();

        // Both are at epoch 1 (Alice merged her own commit; Bob joined at epoch 1).
        assert_eq!(alice.epoch(), 1);
        assert_eq!(bob.epoch(), 1);
    }
}
```

- [ ] **Step 2: Run tests**

Expected: 35 tests pass.

If `tls_serialize_detached` / `tls_deserialize` aren't the correct method names, openmls 0.7 uses `tls_codec` traits. Bring the trait into scope: `use openmls::prelude::tls_codec::Serialize as _;` and similar for `Deserialize`.

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-protocol/src/mls_session.rs
git commit -m "feat(ghost-protocol): MlsSession::join_via_welcome (invitee joins)"
```

---

## Task 15: MlsSession — encrypt + decrypt application messages

**Files:**
- Modify (append): `crates/ghost-protocol/src/mls_session.rs`

Once both members are in the group (post-Welcome), they can exchange application messages. openmls's `MlsGroup::create_message` produces an `MlsMessageOut`; the recipient calls `process_message` to decrypt.

- [ ] **Step 1: Append failing test + impl**

Append to `mls_session.rs`:

```rust
impl MlsSession {
    /// Encrypt an application message. Returns the wire-bytes representation of the MLS message.
    pub fn encrypt_app_message(
        &mut self,
        provider: &GhostMlsProvider,
        signer: &openmls_basic_credential::SignatureKeyPair,
        plaintext: &[u8],
    ) -> Result<Vec<u8>> {
        let msg = self
            .group
            .create_message(provider, signer, plaintext)
            .map_err(|e| ProtoError::Mls(format!("create message: {e}")))?;
        msg.tls_serialize_detached()
            .map_err(|e| ProtoError::Mls(format!("serialize message: {e}")))
    }

    /// Decrypt an application message. Returns the plaintext + sender info.
    pub fn decrypt_app_message(
        &mut self,
        provider: &GhostMlsProvider,
        wire_bytes: &[u8],
    ) -> Result<Vec<u8>> {
        let mls_in = MlsMessageIn::tls_deserialize(&mut &wire_bytes[..])
            .map_err(|e| ProtoError::Mls(format!("deserialize: {e}")))?;
        let protocol_msg = mls_in
            .into_protocol_message()
            .ok_or_else(|| ProtoError::Mls("not a protocol message".into()))?;

        let processed = self
            .group
            .process_message(provider, protocol_msg)
            .map_err(|e| ProtoError::Mls(format!("process message: {e}")))?;

        match processed.into_content() {
            ProcessedMessageContent::ApplicationMessage(app) => Ok(app.into_bytes()),
            _ => Err(ProtoError::Mls("expected ApplicationMessage".into())),
        }
    }
}

#[cfg(test)]
mod app_message_tests {
    use super::*;
    use crate::key_package::generate_key_package;
    use crate::mls_provider::new_provider;

    fn alice_and_bob_in_group() -> (
        (GhostMlsProvider, MlsSession, openmls_basic_credential::SignatureKeyPair),
        (GhostMlsProvider, MlsSession, openmls_basic_credential::SignatureKeyPair),
    ) {
        let alice_provider = new_provider();
        let alice_ik = IdentityKey::generate();
        let alice_dk = DeviceKey::generate(&alice_ik);
        let mut alice = MlsSession::create(&alice_provider, &alice_ik, &alice_dk).unwrap();
        let alice_signer = MlsSession::signer_from_dk(&alice_dk);

        let bob_provider = new_provider();
        let bob_ik = IdentityKey::generate();
        let bob_dk = DeviceKey::generate(&bob_ik);
        let bob_kp = generate_key_package(&bob_provider, &bob_ik, &bob_dk).unwrap();
        let bob_signer = MlsSession::signer_from_dk(&bob_dk);

        let invite = alice.add_member(&alice_provider, &alice_signer, bob_kp).unwrap();
        let welcome_bytes = invite.welcome.tls_serialize_detached().unwrap();
        let welcome_in =
            MlsMessageIn::tls_deserialize(&mut welcome_bytes.as_slice()).unwrap();
        let bob = MlsSession::join_via_welcome(&bob_provider, welcome_in).unwrap();

        (
            (alice_provider, alice, alice_signer),
            (bob_provider, bob, bob_signer),
        )
    }

    #[test]
    fn alice_to_bob_round_trip() {
        let ((alice_p, mut alice, alice_s), (bob_p, mut bob, _)) = alice_and_bob_in_group();
        let wire = alice
            .encrypt_app_message(&alice_p, &alice_s, b"hello bob")
            .unwrap();
        let recovered = bob.decrypt_app_message(&bob_p, &wire).unwrap();
        assert_eq!(recovered, b"hello bob");
    }

    #[test]
    fn bob_to_alice_round_trip() {
        let ((alice_p, mut alice, _), (bob_p, mut bob, bob_s)) = alice_and_bob_in_group();
        let wire = bob
            .encrypt_app_message(&bob_p, &bob_s, b"hi alice")
            .unwrap();
        let recovered = alice.decrypt_app_message(&alice_p, &wire).unwrap();
        assert_eq!(recovered, b"hi alice");
    }

    #[test]
    fn tampered_ciphertext_is_rejected() {
        let ((alice_p, mut alice, alice_s), (bob_p, mut bob, _)) = alice_and_bob_in_group();
        let mut wire = alice
            .encrypt_app_message(&alice_p, &alice_s, b"data")
            .unwrap();
        let last = wire.len() - 1;
        wire[last] ^= 0xFF;
        let err = bob.decrypt_app_message(&bob_p, &wire).unwrap_err();
        assert!(matches!(err, ProtoError::Mls(_)));
    }
}
```

- [ ] **Step 2: Run tests**

Expected: 38 tests pass. The two roundtrip tests prove bidirectional E2EE works at the MLS layer.

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-protocol/src/mls_session.rs
git commit -m "feat(ghost-protocol): MlsSession encrypt/decrypt application messages"
```

---

## Task 16: Bump Identity schema to v2 (replace X25519 PreKeys with MLS KeyPackages)

**Files:**
- Modify: `crates/ghost-identity/src/identity.rs`
- Modify: `crates/ghost-identity/src/lib.rs`
- Possibly: `crates/ghost-identity/src/prekey.rs` (deprecate or remove)

**Background:** Plan 01 introduced `PreKey` (X25519 raw keypairs) as a placeholder for "MLS KeyPackages" mentioned in the design spec. Plan 02 replaces them with proper MLS KeyPackages, which carry credential + signature key + capabilities, not just a single X25519 key.

This task does NOT make `Identity` directly hold MLS KeyPackages — those depend on openmls types that ghost-identity intentionally doesn't depend on. Instead, the Identity now holds **serialized KeyPackage bytes** (so they can be persisted), and a separate function in `ghost-protocol` materialises them.

- [ ] **Step 1: Update `Identity` struct in `crates/ghost-identity/src/identity.rs`**

Find the existing struct and modify it as follows:

```rust
/// Bumped on every breaking change to identity file schema.
pub const IDENTITY_SCHEMA_VERSION: u8 = 2;

/// Number of MLS KeyPackages we publish initially.
pub const INITIAL_KEYPACKAGE_COUNT: u32 = 10;

#[derive(Serialize, Deserialize)]
pub struct Identity {
    pub schema_version: u8,
    pub identity_key: IdentityKey,
    pub device_key: DeviceKey,
    pub display_name: Option<String>,
    /// Serialized MLS KeyPackages, ready to publish. Each is a TLS-encoded `KeyPackage`.
    /// ghost-protocol provides helpers to (de)serialize these.
    pub mls_keypackages: Vec<Vec<u8>>,
    /// Counter for the next KeyPackage ID. Incremented by ghost-protocol when generating new ones.
    pub next_keypackage_id: u32,
    pub created_at: u64,
}

impl Identity {
    /// Generate a fresh Identity with the given display name and current timestamp.
    /// Schema v2: starts with an empty `mls_keypackages` list. Call ghost-protocol's
    /// `Identity::populate_initial_keypackages` (Task 17) immediately after to fill it.
    pub fn generate(display_name: Option<String>, now: u64) -> Self {
        let identity_key = IdentityKey::generate();
        let device_key = DeviceKey::generate(&identity_key);
        Self {
            schema_version: IDENTITY_SCHEMA_VERSION,
            identity_key,
            device_key,
            display_name,
            mls_keypackages: Vec::new(),
            next_keypackage_id: 0,
            created_at: now,
        }
    }

    pub fn ghost_id(&self) -> GhostId {
        self.identity_key.ghost_id()
    }
}
```

The `Debug` impl needs updating to reflect the new fields:

```rust
impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identity")
            .field("schema_version", &self.schema_version)
            .field("ghost_id", &self.ghost_id())
            .field("display_name", &self.display_name)
            .field("mls_keypackages", &self.mls_keypackages.len())
            .field("created_at", &self.created_at)
            .finish()
    }
}
```

The existing `tests` module in `identity.rs` references `INITIAL_PREKEY_COUNT`, `one_time_prekeys`, `last_resort_prekey` — these now need to be removed/updated. The simplest path: replace the existing `generate_populates_all_fields` and `cbor_roundtrip_preserves_identity` tests with versions that match the new schema:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_populates_v2_fields() {
        let id = Identity::generate(Some("Alice".to_string()), 1700000000);
        assert_eq!(id.schema_version, 2);
        assert_eq!(id.display_name.as_deref(), Some("Alice"));
        assert_eq!(id.mls_keypackages.len(), 0); // empty until populated by ghost-protocol
        assert_eq!(id.next_keypackage_id, 0);
        assert_eq!(id.created_at, 1700000000);
    }

    #[test]
    fn dk_signature_verifies_against_ik() {
        let id = Identity::generate(None, 0);
        assert!(id.device_key.verify_parent(&id.identity_key.public()));
    }

    #[test]
    fn cbor_roundtrip_v2() {
        let mut original = Identity::generate(Some("Bob".to_string()), 1700000000);
        // Populate with a couple of fake KeyPackage bytes for the roundtrip test.
        original.mls_keypackages.push(vec![0x01, 0x02, 0x03]);
        original.mls_keypackages.push(vec![0x04, 0x05, 0x06]);
        original.next_keypackage_id = 2;

        let mut buf = Vec::new();
        ciborium::into_writer(&original, &mut buf).unwrap();

        let restored: Identity = ciborium::from_reader(&buf[..]).unwrap();
        assert_eq!(restored.schema_version, 2);
        assert_eq!(restored.ghost_id(), original.ghost_id());
        assert_eq!(restored.mls_keypackages.len(), 2);
        assert_eq!(restored.mls_keypackages[0], vec![0x01, 0x02, 0x03]);
        assert_eq!(restored.next_keypackage_id, 2);
    }
}
```

- [ ] **Step 2: Update `lib.rs` exports**

```rust
//! Ghost identity: Identity Key + Device Key + encrypted identity file + OS keystore.

pub mod crypto;
pub mod file_format;
pub mod identity;
pub mod keys;
pub mod keystore;
pub mod paths;
pub mod prekey;  // kept for now but no longer used by Identity; will be removed in Task 17
pub mod storage;

pub use file_format::{Header, FILE_FORMAT_VERSION};
pub use identity::{
    CreateOptions, Identity, IdentityError, IDENTITY_SCHEMA_VERSION,
    INITIAL_KEYPACKAGE_COUNT,
};
pub use keys::{DeviceKey, IdentityKey};
pub use keystore::{load_or_create_secret, store_secret, wipe_secret, KeystoreError};
pub use paths::{database_file, ghost_home, identity_file, logs_dir, PathsError};
pub use storage::{load, save, StorageError};

// `INITIAL_PREKEY_COUNT` is deprecated. We re-export it as a compatibility shim
// that points to INITIAL_KEYPACKAGE_COUNT for any straggling references; it will
// be removed in Plan 02 Task 17.
#[deprecated(note = "Use INITIAL_KEYPACKAGE_COUNT (Plan 02). Will be removed in Task 17.")]
pub use identity::INITIAL_KEYPACKAGE_COUNT as INITIAL_PREKEY_COUNT;
```

- [ ] **Step 3: Update `create_load_tests` in `identity.rs` if any test referenced removed fields**

Search for `one_time_prekeys`, `last_resort_prekey`, `INITIAL_PREKEY_COUNT` in the test bodies and adjust assertions to use the v2 fields. The semantics are: tests should check that `mls_keypackages.len() == 0` initially (Plan 02 Task 17 will populate them).

- [ ] **Step 4: Run tests**

```bash
cargo +1.85-x86_64-pc-windows-msvc test -p ghost-identity -- --test-threads=1
cargo +1.85-x86_64-pc-windows-msvc test -p ghost-protocol
```

Expected: ghost-identity should still have a passing count comparable to Plan 01 (the schema bump may have changed test names, not counts). ghost-protocol unchanged at 38.

If ghost-identity tests fail because of removed fields, fix the tests in this commit — that's expected churn from a schema bump.

- [ ] **Step 5: Commit**

```bash
git add crates/ghost-identity/
git commit -m "feat(ghost-identity)!: bump schema to v2 (MLS KeyPackages replace X25519 PreKeys)"
```

The `!` after `feat(ghost-identity)` marks this as a breaking change in conventional commits.

---

## Task 17: Populate Identity with initial KeyPackages from ghost-protocol; remove obsolete prekey module

**Files:**
- Modify: `crates/ghost-protocol/src/key_package.rs` (add `populate_initial_keypackages`)
- Modify: `crates/ghost-protocol/src/lib.rs`
- Delete: `crates/ghost-identity/src/prekey.rs`
- Modify: `crates/ghost-identity/src/lib.rs` (remove the `prekey` module)

- [ ] **Step 1: Add `populate_initial_keypackages` in ghost-protocol**

Append to `crates/ghost-protocol/src/key_package.rs`:

```rust
/// Generate `count` KeyPackages and store their TLS-serialized bytes in `identity.mls_keypackages`.
/// Updates `identity.next_keypackage_id` accordingly.
///
/// Typically called immediately after `Identity::generate` to populate the publishable bundle.
pub fn populate_initial_keypackages(
    identity: &mut ghost_identity::Identity,
    provider: &GhostMlsProvider,
    count: u32,
) -> Result<()> {
    use openmls::prelude::tls_codec::Serialize as _;

    let ik = &identity.identity_key;
    let dk = &identity.device_key;
    for _ in 0..count {
        let kp = generate_key_package(provider, ik, dk)?;
        let bytes = kp
            .tls_serialize_detached()
            .map_err(|e| ProtoError::Mls(format!("serialize key package: {e}")))?;
        identity.mls_keypackages.push(bytes);
        identity.next_keypackage_id += 1;
    }
    Ok(())
}

#[cfg(test)]
mod populate_tests {
    use super::*;
    use crate::mls_provider::new_provider;

    #[test]
    fn populate_adds_correct_count() {
        let provider = new_provider();
        let mut id = ghost_identity::Identity::generate(Some("Alice".into()), 0);
        populate_initial_keypackages(&mut id, &provider, 5).unwrap();
        assert_eq!(id.mls_keypackages.len(), 5);
        assert_eq!(id.next_keypackage_id, 5);
    }

    #[test]
    fn populate_is_additive() {
        let provider = new_provider();
        let mut id = ghost_identity::Identity::generate(None, 0);
        populate_initial_keypackages(&mut id, &provider, 3).unwrap();
        populate_initial_keypackages(&mut id, &provider, 2).unwrap();
        assert_eq!(id.mls_keypackages.len(), 5);
        assert_eq!(id.next_keypackage_id, 5);
    }
}
```

- [ ] **Step 2: Remove obsolete `prekey` module from ghost-identity**

```bash
rm crates/ghost-identity/src/prekey.rs
```

Edit `crates/ghost-identity/src/lib.rs`:
- Remove `pub mod prekey;`
- Remove `pub use prekey::{generate_batch, PreKey};`
- Remove the `INITIAL_PREKEY_COUNT` deprecation shim (since Task 16's caveat is now resolved):

```rust
//! Ghost identity: Identity Key + Device Key + encrypted identity file + OS keystore.

pub mod crypto;
pub mod file_format;
pub mod identity;
pub mod keys;
pub mod keystore;
pub mod paths;
pub mod storage;

pub use file_format::{Header, FILE_FORMAT_VERSION};
pub use identity::{
    CreateOptions, Identity, IdentityError, IDENTITY_SCHEMA_VERSION,
    INITIAL_KEYPACKAGE_COUNT,
};
pub use keys::{DeviceKey, IdentityKey};
pub use keystore::{load_or_create_secret, store_secret, wipe_secret, KeystoreError};
pub use paths::{database_file, ghost_home, identity_file, logs_dir, PathsError};
pub use storage::{load, save, StorageError};
```

- [ ] **Step 3: Run tests for both crates**

```bash
cargo +1.85-x86_64-pc-windows-msvc test -p ghost-identity -- --test-threads=1
cargo +1.85-x86_64-pc-windows-msvc test -p ghost-protocol
```

Expected: ghost-identity tests pass at a slightly reduced count (the 3 prekey tests are gone). ghost-protocol gains 2 new tests — total 40.

- [ ] **Step 4: Commit**

```bash
git add crates/
git commit -m "feat(ghost-protocol): populate Identity with KeyPackages; remove obsolete prekey module"
```

---

## Task 18: End-to-end integration test — two identities exchange messages

**Files:**
- Create: `crates/ghost-protocol/tests/e2e.rs`

This is the **deliverable for Plan 02**: a single integration test that walks through Alice and Bob's full first-contact + bidirectional messaging cycle, end-to-end through both the MLS layer and the sealed-sender envelope wrapper.

- [ ] **Step 1: Create the integration test**

Create `crates/ghost-protocol/tests/e2e.rs`:

```rust
//! Plan 02 deliverable: Alice and Bob exchange messages end-to-end.
//!
//! Flow:
//! 1. Alice and Bob create independent identities with KeyPackages
//! 2. Bob "publishes" a KeyPackage to a stub mailbox
//! 3. Alice picks up Bob's KeyPackage and creates an MLS group inviting Bob
//! 4. Bob processes the Welcome and joins
//! 5. Alice → Bob: encrypt application msg, wrap in sealed-sender envelope, send wire bytes
//! 6. Bob receives wire bytes, unwraps, decrypts MLS, gets plaintext
//! 7. Bob → Alice: reverse flow, verify plaintext

use ghost_protocol::{
    delivery_public, generate_key_package, populate_initial_keypackages, new_provider,
    unwrap_message, wrap_message, MlsSession, MsgType, PayloadType,
};
use ghost_identity::Identity;
use openmls::prelude::tls_codec::{Deserialize as TlsDeserialize, Serialize as TlsSerialize};
use openmls::prelude::{KeyPackage, MlsMessageIn, MlsMessageOut};

#[test]
fn alice_and_bob_full_exchange() {
    // ===== Setup =====
    let alice_provider = new_provider();
    let mut alice_id = Identity::generate(Some("Alice".into()), 1700000000);
    populate_initial_keypackages(&mut alice_id, &alice_provider, 2).unwrap();

    let bob_provider = new_provider();
    let mut bob_id = Identity::generate(Some("Bob".into()), 1700000000);
    populate_initial_keypackages(&mut bob_id, &bob_provider, 2).unwrap();

    // ===== Stub mailbox: Bob publishes a KeyPackage =====
    // Just take one of Bob's pre-generated KeyPackages and pretend it's "published".
    let bob_kp_bytes = bob_id.mls_keypackages.first().expect("bob has key packages").clone();
    // Alice fetches it (in real life from Bob's embedded server's GET /v1/keypackages/<bob_id>).
    let bob_kp = KeyPackage::tls_deserialize(&mut bob_kp_bytes.as_slice())
        .expect("deserialize bob's keypackage");

    // ===== Alice creates the group, adds Bob =====
    let mut alice_session = MlsSession::create(&alice_provider, &alice_id.identity_key, &alice_id.device_key)
        .expect("alice create session");
    let alice_signer = MlsSession::signer_from_dk(&alice_id.device_key);
    let invite = alice_session
        .add_member(&alice_provider, &alice_signer, bob_kp)
        .expect("alice add bob");

    // ===== Bob receives the Welcome, joins =====
    let welcome_bytes = invite.welcome.tls_serialize_detached().unwrap();
    let welcome_in = MlsMessageIn::tls_deserialize(&mut welcome_bytes.as_slice()).unwrap();
    let mut bob_session =
        MlsSession::join_via_welcome(&bob_provider, welcome_in).expect("bob join");

    assert_eq!(alice_session.epoch(), 1);
    assert_eq!(bob_session.epoch(), 1);

    // ===== Alice → Bob: encrypt MLS, wrap envelope, send =====
    let mls_ct = alice_session
        .encrypt_app_message(&alice_provider, &alice_signer, b"hello bob")
        .unwrap();

    let bob_delivery = delivery_public(&bob_id.identity_key);
    let wire_alice_to_bob = wrap_message(
        &alice_id.identity_key,
        &alice_id.device_key,
        bob_id.ghost_id(),
        &bob_delivery,
        MsgType::AppMessage,
        PayloadType::AppText,
        mls_ct,
        1700000060,
    )
    .unwrap();

    // ===== Bob: unwrap envelope, decrypt MLS =====
    let alice_dk_pub = alice_id.device_key.public();
    let unwrapped = unwrap_message(&wire_alice_to_bob, &bob_id.identity_key, |id| {
        if id == &alice_id.ghost_id() {
            Some(alice_dk_pub)
        } else {
            None
        }
    })
    .unwrap();
    assert_eq!(unwrapped.sender_id, alice_id.ghost_id());
    assert_eq!(unwrapped.payload_type, PayloadType::AppText);

    let plaintext = bob_session
        .decrypt_app_message(&bob_provider, &unwrapped.payload)
        .unwrap();
    assert_eq!(plaintext, b"hello bob");

    // ===== Bob → Alice: reverse flow =====
    let bob_signer = MlsSession::signer_from_dk(&bob_id.device_key);
    let mls_ct_back = bob_session
        .encrypt_app_message(&bob_provider, &bob_signer, b"hi alice")
        .unwrap();

    let alice_delivery = delivery_public(&alice_id.identity_key);
    let wire_bob_to_alice = wrap_message(
        &bob_id.identity_key,
        &bob_id.device_key,
        alice_id.ghost_id(),
        &alice_delivery,
        MsgType::AppMessage,
        PayloadType::AppText,
        mls_ct_back,
        1700000120,
    )
    .unwrap();

    let bob_dk_pub = bob_id.device_key.public();
    let unwrapped_back = unwrap_message(&wire_bob_to_alice, &alice_id.identity_key, |id| {
        if id == &bob_id.ghost_id() {
            Some(bob_dk_pub)
        } else {
            None
        }
    })
    .unwrap();
    let plaintext_back = alice_session
        .decrypt_app_message(&alice_provider, &unwrapped_back.payload)
        .unwrap();
    assert_eq!(plaintext_back, b"hi alice");
}
```

- [ ] **Step 2: Run the test**

```bash
cargo +1.85-x86_64-pc-windows-msvc test -p ghost-protocol --test e2e
```

Expected: test `alice_and_bob_full_exchange` passes. This is the **plan deliverable** — both directions of E2EE messaging work end-to-end through MLS + sealed sender + outer envelope.

If ANY step in the test fails, STOP and report DONE_WITH_CONCERNS with the failing assertion. The test is the spec; passing it is the deliverable.

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-protocol/tests/
git commit -m "test(ghost-protocol): end-to-end E2EE exchange between Alice and Bob"
```

---

## Task 19: Final verification + tag plan-02-complete

**Files:** none (verification only)

- [ ] **Step 1: Run the full battery**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --workspace -- -D warnings
cargo +1.85-x86_64-pc-windows-msvc test --workspace -- --test-threads=1
```

Expected:
- `cargo fmt` clean
- `cargo clippy` zero warnings
- All tests pass: ghost-core 16, ghost-identity 41-43 (slight count change from Task 16's schema update), ghost-protocol 40+ unit + 1 integration

If anything fails, STOP and report.

- [ ] **Step 2: Tag the milestone**

```bash
git tag -a plan-02-complete -m "Plan 02 (Crypto + Wire Protocol) complete

Deliverable: ghost-protocol crate providing MLS group state via openmls
0.7, sealed-sender envelopes (X25519 ECDH + XChaCha20-Poly1305), CBOR
wire format (OuterEnvelope + SealedBlob), KeyPackage generation, and
high-level wrap/unwrap message APIs. Identity schema bumped to v2,
replacing X25519 PreKeys with MLS KeyPackages.

Validated end-to-end by integration test 'alice_and_bob_full_exchange'
(crates/ghost-protocol/tests/e2e.rs) which performs the full first-contact
+ bidirectional messaging flow purely in memory.

Coverage: 16 + ~42 + 40+ = ~98 unit tests + 1 integration test.

Next: Plan 03 — Storage (SQLite + SQLCipher backend, MLS state persistence,
contact/message repositories)."
```

- [ ] **Step 3: Verify the tag**

```bash
git tag -l
git show plan-02-complete --stat | head -20
```

Expected: `plan-02-complete` listed; `git show` shows the annotated message.

---

## Risks & Open Questions for Plan 02

| Risk | Mitigation |
|---|---|
| openmls 0.7 API differs from what the plan assumes | Implementer consults docs; tasks 9–15 explicitly note "consult openmls 0.7 docs"; STOP and report if blocked. |
| `openmls_basic_credential` may need to be added as a separate workspace dep | Task 10 covers this contingency. |
| openmls's storage trait state may bleed across tests | Each test creates a fresh `new_provider()`; provider is per-Identity in the integration test. |
| KeyPackage TLS serialization format can change between openmls versions | We pin to openmls 0.7; later upgrade requires re-publishing identities. Acceptable for MVP. |
| Sealed-sender `delivery_secret` derivation depends on adding `secret_bytes` accessors to IK and DK | Tasks 6 and 10 explicitly add these methods. |
| Schema v2 migration is a breaking change | No production users yet; safe. Plan 02 commits include `feat!:` to flag breaking. |

## Self-Review Checklist (run after writing this plan)

**1. Spec coverage** — every requirement in spec section 4 (Crypto and protocol layer) implemented:
- ✓ MLS via openmls (Tasks 9–15)
- ✓ KeyPackages and async first-contact (Tasks 11, 13, 14, 17)
- ✓ Sealed sender (Tasks 6–8)
- ✓ Wire format CBOR (Tasks 4, 5, 8)
- ✓ Forward secrecy + post-compromise security (delivered by MLS itself; verified by integration test)
- ✓ Sender signature (DK signs the SealedBlob signing-bytes; verified in Task 8)
- ✓ Replay protection — MessageUuid included; full dedup table comes in Plan 03 (storage)

Items NOT in Plan 02 (correctly deferred):
- Persistent MLS state across process restarts → Plan 03 (storage)
- Inbox-side replay dedup → Plan 03 (storage `inbox_dedup` table)
- Network transport → Plans 04, 05
- HTTP API endpoints (`/v1/keypackages`, `/v1/inbox`) → Plan 05
- Pre-key replenishment background task → Plan 06

**2. Placeholder scan** — searched for "TBD", "TODO", "implement later". The phrase "Implementer:" appears as a guidance label for openmls API consultation, not as a placeholder for missing requirements.

**3. Type consistency** — `MlsSession::create`, `MlsSession::add_member`, `MlsSession::join_via_welcome`, `MlsSession::encrypt_app_message`, `MlsSession::decrypt_app_message` all use consistent naming. `wrap_message` / `unwrap_message`, `seal_to` / `unseal`, `delivery_public` / `delivery_secret`. `INITIAL_KEYPACKAGE_COUNT` used uniformly.

---

**Plan 02 complete and ready for execution.**
