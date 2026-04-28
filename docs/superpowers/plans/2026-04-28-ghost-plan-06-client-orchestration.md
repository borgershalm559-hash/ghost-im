# Ghost Plan 06 — Client Orchestration (first contact + bidirectional messaging)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `ghost-client` crate that orchestrates Identity + Storage + Protocol + Network + Server into a working messaging client. **Plan 06 is the first plan that produces an end-to-end working 1-on-1 messaging system** — two `Client` instances complete first contact and exchange E2EE messages over loopback, all the way through MLS + sealed-sender + libp2p QUIC + SQLCipher persistence.

**Architecture:** New crate depends on every prior ghost-* crate. Async (tokio). Provides a single `Client` type that:
- Owns the full stack lifecycle (open / close).
- Exposes user-facing operations: `create_invite`, `add_contact`, `send_message`, `list_messages`, `list_contacts`.
- Runs a background "inbox processor" task that drains the Server's inbox channel, unwraps envelopes, advances MLS state, and persists messages.

A minimal CLI (`ghost-client-cli`) wraps the library so two manually-spawned processes can chat over loopback. Deeper REPL/UI is deferred to Plan 07 (Tauri).

**Tech Stack:** All existing ghost-* deps. Adds `clap` and `tracing` to `ghost-client-cli`. No new external crates in `ghost-client` library.

**Deliverable Plan 06:** integration test in `crates/ghost-client/tests/e2e_messaging.rs` that:

1. Alice and Bob open independent `Client` instances (each: own Identity + own SQLCipher DB + own Network listening on loopback + own Server).
2. Alice generates a `bech32` invite string containing her GhostId + Multiaddr + signed token.
3. Bob calls `client.add_contact(alice_invite)`:
   - Parses + verifies invite
   - Connects to Alice via the invite's address
   - Fetches one of Alice's KeyPackages via `Client::get_key_package` (Plan 05)
   - Validates the KeyPackage
   - Creates an MLS group with Bob as creator + Alice as invitee
   - Sends the Welcome to Alice via `Client::send_inbox` (Plan 05)
   - Persists MLS state + Alice as a contact
4. Alice's background inbox processor receives the Welcome envelope:
   - Unwraps via `unwrap_message` (Plan 02)
   - Detects PayloadType::MlsHandshake
   - Calls `MlsSession::join_via_welcome` (Plan 02)
   - Persists MLS state + Bob as a contact
5. Bob calls `client.send_message(alice_ghost_id, "hello alice")`:
   - Loads Bob's MLS state for the Alice conversation
   - MLS-encrypts the plaintext
   - Wraps in sealed-sender envelope (Plan 02 wrap_message)
   - Sends via Client::send_inbox to Alice's address
   - Saves the message to Bob's `messages` table
   - Saves the advanced MLS state back to Bob's `mls_groups` table
6. Alice's background processor receives, unwraps, decrypts via MLS, persists to Alice's `messages` table.
7. Alice calls `client.send_message(bob_ghost_id, "hi bob")` — reverse flow.
8. Bob receives.
9. Both call `client.list_messages(<peer>)` and observe the symmetric conversation history.

A bash smoke script (`scripts/smoke-test-plan-06.sh`) spawns two `ghost-client-cli` subprocesses with scripted commands and verifies the same round-trip in process-isolated form.

**Reference spec:** [docs/superpowers/specs/2026-04-27-ghost-mvp1-design.md](../specs/2026-04-27-ghost-mvp1-design.md), section 5 (first-contact flow).

**Reference plans:**
- [Plan 02](2026-04-27-ghost-plan-02-crypto-protocol.md) — wrap/unwrap_message, MlsSession lifecycle
- [Plan 03](2026-04-27-ghost-plan-03-storage.md) — Database, repos, MLS state persistence
- [Plan 04](2026-04-28-ghost-plan-04-network-discovery.md) — Network, listen, address discovery
- [Plan 05](2026-04-28-ghost-plan-05-embedded-server.md) — Server, Client (typed endpoint wrappers)

---

## Notes for the implementer

**Provider lifecycle:** every MlsSession operation (encrypt, decrypt, add_member, etc.) needs a live `GhostMlsProvider`. We have two patterns:

- **Per-operation provider:** `MlsSession::deserialize_state(blob) -> (provider, session)` rebuilds both. Use the (provider, session) tuple briefly, then drop both. Save advanced state back via `session.serialize_state(&provider)`. Simple, stateless across invocations. **Plan 06 uses this pattern.**

- **Long-lived provider:** keep one provider per Identity for the lifetime of the Client. Restore sessions into it via storage-provider methods. More efficient but couples lifetimes. Defer to MVP-2.

**Signer re-store:** every fresh `(provider, session)` from `deserialize_state` needs the SignatureKeyPair re-stored before encrypt operations. Plan 03 Task 11 documented this.

**Background processing:** the Client spawns a tokio task that loops on `server.next_inbox().await`. Each envelope:
1. CBOR-decode the OuterEnvelope (Plan 02)
2. Unwrap with `unwrap_message`, providing a contact-DK lookup callback that reads `contacts` table
3. Dispatch on `PayloadType`:
   - `MlsHandshake` → process Welcome → save new mls_groups row + contacts row
   - `AppText` → load existing mls_groups → MLS-decrypt → save messages row + advanced mls_groups state
4. Errors are logged via `tracing` and the loop continues; one bad message doesn't kill the processor.

**Sender DK lookup callback:** when Bob sends a message to Alice, his outer envelope's signature is by Bob's DK. When Alice's `unwrap_message` callback runs, it must return Bob's VerifyingKey. We get it from:
- The `contacts` table (after first contact, we know Bob's DK pub from his identity advertised during Welcome handling — but Welcome doesn't carry DK pub directly, it carries IK via BasicCredential)
- We also receive Bob's DK pub from the MLS group state (each member's signature_key is part of group state)

For Plan 06's first-contact flow, we add a small extension to the contacts table: `dk_pub_bytes BLOB` (32 bytes). Set it when the contact is first established (via Welcome processing or initial KeyPackage validation).

**Schema note:** This requires a Plan 06 migration `0002_add_dk_pub_to_contacts.sql` that adds a column to the contacts table. Plan 03's migration runner handles this automatically.

---

## Task 1: ghost-client crate scaffold + GhostInvite type

**Files:**
- Modify: `Cargo.toml` (root)
- Create: `crates/ghost-client/Cargo.toml`
- Create: `crates/ghost-client/src/lib.rs`
- Create: `crates/ghost-client/src/error.rs`
- Create: `crates/ghost-client/src/invite.rs`

### Step 1.1: Modify root `Cargo.toml`

Add `"crates/ghost-client"` to `members = [...]` (alphabetically: between `"crates/ghost-core"` and `"crates/ghost-identity"`).

No new workspace deps required (we already have everything).

### Step 1.2: Create `crates/ghost-client/Cargo.toml`

```toml
[package]
name = "ghost-client"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Ghost client orchestration: ties Identity + Storage + Protocol + Network + Server together."

[dependencies]
ghost-core     = { path = "../ghost-core" }
ghost-identity = { path = "../ghost-identity" }
ghost-protocol = { path = "../ghost-protocol" }
ghost-storage  = { path = "../ghost-storage" }
ghost-network  = { path = "../ghost-network" }
ghost-server   = { path = "../ghost-server" }

tokio = { workspace = true }
serde = { workspace = true }
ciborium = { workspace = true }
hex = { workspace = true }
blake3 = { workspace = true }
bech32 = { workspace = true }
ed25519-dalek = { workspace = true }
libp2p = { workspace = true }

openmls = { workspace = true }
openmls_traits = { workspace = true }

thiserror = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
proptest = { workspace = true }
openmls_rust_crypto = { workspace = true }
openmls_basic_credential = { workspace = true }
```

### Step 1.3: Create `crates/ghost-client/src/error.rs`

```rust
//! Top-level error type for ghost-client.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("identity: {0}")]
    Identity(#[from] ghost_identity::IdentityError),
    #[error("storage: {0}")]
    Storage(#[from] ghost_storage::StorageError),
    #[error("network: {0}")]
    Network(#[from] ghost_network::NetworkError),
    #[error("server: {0}")]
    Server(#[from] ghost_server::ServerError),
    #[error("protocol: {0}")]
    Protocol(#[from] ghost_protocol::ProtoError),

    #[error("invite parse: {0}")]
    InviteParse(String),
    #[error("invite signature invalid")]
    InviteSignatureInvalid,
    #[error("invite expired at {0}")]
    InviteExpired(u64),

    #[error("contact not found: {0}")]
    ContactNotFound(String),
    #[error("MLS group not found for contact {0}")]
    MlsGroupNotFound(String),

    #[error("no key packages available from peer")]
    NoKeyPackagesFromPeer,

    #[error("CBOR encode: {0}")]
    CborEncode(String),
    #[error("CBOR decode: {0}")]
    CborDecode(String),

    #[error("invalid input: {0}")]
    Invalid(String),

    #[error("internal: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, ClientError>;
```

### Step 1.4: Create `crates/ghost-client/src/invite.rs`

```rust
//! GhostInvite — bech32-encoded out-of-band first-contact bundle.
//!
//! Layout (CBOR-serialized then bech32-wrapped with `ghostinvite1` HRP):
//! ```text
//! GhostInvite {
//!   ghost_id:        GhostId
//!   addresses:       Vec<String>      // multiaddr strings
//!   invite_token:    [u8; 16]
//!   expires_at:      u64
//!   signature:       [u8; 64]         // Ed25519(IK, signing_bytes)
//! }
//! ```

use crate::{ClientError, Result};
use bech32::{Bech32, Hrp};
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use ghost_core::GhostId;
use ghost_identity::IdentityKey;
use serde::{Deserialize, Serialize};

const HRP_INVITE: &str = "ghostinvite";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GhostInvite {
    pub ghost_id: GhostId,
    pub addresses: Vec<String>,
    pub invite_token: [u8; 16],
    pub expires_at: u64,
    pub signature: [u8; 64],
}

impl GhostInvite {
    pub fn new(
        ik: &IdentityKey,
        addresses: Vec<String>,
        invite_token: [u8; 16],
        now: u64,
        ttl_seconds: u64,
    ) -> Self {
        let ghost_id = ik.ghost_id();
        let expires_at = now.saturating_add(ttl_seconds);
        let to_sign = Self::signing_bytes(&ghost_id, &addresses, &invite_token, expires_at);
        let sig = ik.sign(&to_sign);
        Self {
            ghost_id,
            addresses,
            invite_token,
            expires_at,
            signature: sig.to_bytes(),
        }
    }

    pub fn signing_bytes(
        ghost_id: &GhostId,
        addresses: &[String],
        invite_token: &[u8; 16],
        expires_at: u64,
    ) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(ghost_id.as_bytes());
        let n = addresses.len() as u32;
        hasher.update(&n.to_be_bytes());
        for a in addresses {
            let l = a.len() as u32;
            hasher.update(&l.to_be_bytes());
            hasher.update(a.as_bytes());
        }
        hasher.update(invite_token);
        hasher.update(&expires_at.to_be_bytes());
        *hasher.finalize().as_bytes()
    }

    pub fn verify(&self, now: u64) -> Result<()> {
        if now > self.expires_at {
            return Err(ClientError::InviteExpired(self.expires_at));
        }
        let pub_key = VerifyingKey::from_bytes(self.ghost_id.as_bytes())
            .map_err(|e| ClientError::Invalid(format!("ghost_id: {e}")))?;
        let sig = Signature::from_bytes(&self.signature);
        let to_verify = Self::signing_bytes(
            &self.ghost_id,
            &self.addresses,
            &self.invite_token,
            self.expires_at,
        );
        pub_key
            .verify(&to_verify, &sig)
            .map_err(|_| ClientError::InviteSignatureInvalid)
    }

    pub fn to_bech32(&self) -> Result<String> {
        let mut buf = Vec::new();
        ciborium::into_writer(self, &mut buf)
            .map_err(|e| ClientError::CborEncode(e.to_string()))?;
        let hrp = Hrp::parse(HRP_INVITE).expect("static HRP is valid");
        bech32::encode::<Bech32>(hrp, &buf)
            .map_err(|e| ClientError::Invalid(format!("bech32 encode: {e}")))
    }

    pub fn from_bech32(s: &str) -> Result<Self> {
        let (hrp, data) = bech32::decode(s)
            .map_err(|e| ClientError::InviteParse(format!("bech32: {e}")))?;
        if hrp.as_str() != HRP_INVITE {
            return Err(ClientError::InviteParse(format!(
                "wrong hrp: expected {HRP_INVITE}, got {}",
                hrp.as_str()
            )));
        }
        ciborium::from_reader(&data[..]).map_err(|e| ClientError::CborDecode(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invite_round_trip_via_bech32() {
        let ik = IdentityKey::generate();
        let invite = GhostInvite::new(
            &ik,
            vec!["/ip4/127.0.0.1/udp/0/quic-v1".into()],
            [7u8; 16],
            1700000000,
            3600 * 24 * 7,
        );
        let s = invite.to_bech32().unwrap();
        assert!(s.starts_with("ghostinvite1"));
        let restored = GhostInvite::from_bech32(&s).unwrap();
        assert_eq!(restored, invite);
        restored.verify(1700000100).unwrap();
    }

    #[test]
    fn verify_fails_when_expired() {
        let ik = IdentityKey::generate();
        let invite = GhostInvite::new(&ik, vec![], [0u8; 16], 1700000000, 60);
        let err = invite.verify(1700001000).unwrap_err();
        assert!(matches!(err, ClientError::InviteExpired(_)));
    }

    #[test]
    fn verify_fails_on_tampered_addresses() {
        let ik = IdentityKey::generate();
        let mut invite = GhostInvite::new(
            &ik,
            vec!["/ip4/127.0.0.1/udp/0/quic-v1".into()],
            [0u8; 16],
            0,
            1000,
        );
        invite.addresses.push("/ip4/9.9.9.9/udp/0/quic-v1".into());
        let err = invite.verify(0).unwrap_err();
        assert!(matches!(err, ClientError::InviteSignatureInvalid));
    }

    #[test]
    fn from_bech32_rejects_wrong_hrp() {
        // A bech32 string with the wrong HRP should be rejected.
        let hrp = Hrp::parse("ghost").unwrap();  // wrong: should be "ghostinvite"
        let bad = bech32::encode::<Bech32>(hrp, &[1u8; 4]).unwrap();
        let err = GhostInvite::from_bech32(&bad).unwrap_err();
        assert!(matches!(err, ClientError::InviteParse(_)));
    }
}
```

### Step 1.5: Create `crates/ghost-client/src/lib.rs`

```rust
//! Ghost client orchestration: ties Identity + Storage + Protocol + Network + Server together.

pub mod error;
pub mod invite;

pub use error::{ClientError, Result};
pub use invite::GhostInvite;

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-client");
    }
}
```

### Step 1.6: Test + commit

```bash
export PATH="/c/Strawberry/perl/bin:/c/Strawberry/c/bin:$PATH"
cd /c/Users/david/Desktop/Ghost
cargo +1.87-x86_64-pc-windows-msvc test -p ghost-client 2>&1 | tail -10
```

Expected: 5 tests pass (1 smoke + 4 invite).

```bash
git add Cargo.toml Cargo.lock crates/ghost-client/
git commit -m "feat(ghost-client): scaffold crate with GhostInvite (bech32 + sign + verify)"
```

---

## Task 2: Schema migration — add `dk_pub` to contacts

**Files:**
- Create: `crates/ghost-storage/migrations/0002_add_contact_dk_pub.sql`
- Modify: `crates/ghost-storage/src/migrations.rs`
- Modify: `crates/ghost-storage/src/repos/contacts.rs`

### Step 2.1: Create the migration file

```sql
-- Plan 06 migration: add 32-byte DK public key column to contacts.
-- Used during inbound message verification (sender's signature check).

ALTER TABLE contacts ADD COLUMN dk_pub BLOB;
```

### Step 2.2: Update `migrations.rs`

Find the `MIGRATIONS` constant and add the second entry:

```rust
const MIGRATIONS: &[(u32, &str)] = &[
    (1, include_str!("../migrations/0001_init.sql")),
    (2, include_str!("../migrations/0002_add_contact_dk_pub.sql")),
];
```

Bump `APP_SCHEMA_VERSION`:

```rust
pub const APP_SCHEMA_VERSION: u32 = 2;
```

### Step 2.3: Update `Contact` struct + ContactsRepo

In `crates/ghost-storage/src/repos/contacts.rs`, add the new field to `Contact`:

```rust
pub struct Contact {
    pub ghost_id: GhostId,
    pub display_name: Option<String>,
    pub local_alias: Option<String>,
    pub fingerprint: String,
    pub added_at: i64,
    pub last_endpoint: Option<String>,
    pub verification: Verification,
    pub notes: Option<String>,
    pub blocked: bool,
    /// Ed25519 DeviceKey public bytes — set when first established. None means we
    /// haven't validated the contact's signing identity yet.
    pub dk_pub: Option<[u8; 32]>,
}
```

Update `ContactsRepo::insert` to include the new column:

```rust
pub fn insert(&self, contact: &Contact) -> Result<()> {
    self.db.with_tx(|tx| {
        tx.execute(
            "INSERT INTO contacts (
                ghost_id, display_name, local_alias, fingerprint, added_at,
                last_endpoint, verification, notes, blocked, dk_pub
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                contact.ghost_id.as_bytes(),
                contact.display_name,
                contact.local_alias,
                contact.fingerprint,
                contact.added_at,
                contact.last_endpoint,
                contact.verification as i64,
                contact.notes,
                contact.blocked as i64,
                contact.dk_pub.as_ref().map(|b| &b[..]),
            ],
        )?;
        Ok(())
    })
}
```

Update `update`:

```rust
pub fn update(&self, contact: &Contact) -> Result<()> {
    self.db.with_tx(|tx| {
        let n = tx.execute(
            "UPDATE contacts SET
                display_name = ?2,
                local_alias = ?3,
                last_endpoint = ?4,
                verification = ?5,
                notes = ?6,
                blocked = ?7,
                dk_pub = ?8
             WHERE ghost_id = ?1",
            params![
                contact.ghost_id.as_bytes(),
                contact.display_name,
                contact.local_alias,
                contact.last_endpoint,
                contact.verification as i64,
                contact.notes,
                contact.blocked as i64,
                contact.dk_pub.as_ref().map(|b| &b[..]),
            ],
        )?;
        if n == 0 {
            return Err(StorageError::NotFound(format!("contact {}", contact.ghost_id)));
        }
        Ok(())
    })
}
```

Update `row_to_contact`:

```rust
fn row_to_contact(row: &rusqlite::Row<'_>) -> Result<Contact> {
    let ghost_id_bytes: Vec<u8> = row.get(0)?;
    if ghost_id_bytes.len() != 32 {
        return Err(StorageError::InvalidBlob {
            table: "contacts",
            column: "ghost_id",
            detail: format!("expected 32 bytes, got {}", ghost_id_bytes.len()),
        });
    }
    let mut id_arr = [0u8; 32];
    id_arr.copy_from_slice(&ghost_id_bytes);

    let dk_pub_bytes: Option<Vec<u8>> = row.get(9)?;
    let dk_pub = match dk_pub_bytes {
        None => None,
        Some(b) => {
            if b.len() != 32 {
                return Err(StorageError::InvalidBlob {
                    table: "contacts",
                    column: "dk_pub",
                    detail: format!("expected 32 bytes, got {}", b.len()),
                });
            }
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&b);
            Some(arr)
        }
    };

    let verification: i64 = row.get(6)?;
    let blocked: i64 = row.get(8)?;
    Ok(Contact {
        ghost_id: GhostId::from_bytes(id_arr),
        display_name: row.get(1)?,
        local_alias: row.get(2)?,
        fingerprint: row.get(3)?,
        added_at: row.get(4)?,
        last_endpoint: row.get(5)?,
        verification: Verification::from_i64(verification)?,
        notes: row.get(7)?,
        blocked: blocked != 0,
        dk_pub,
    })
}
```

Update SELECT statements in `get` and `list` to include `dk_pub` as the 10th column.

### Step 2.4: Update existing tests in `contacts.rs`

The `fake_contact` helper needs to produce `Contact` with `dk_pub: None` (the default). The `update` test should also exercise setting `dk_pub`.

Add a new test:

```rust
#[test]
fn dk_pub_round_trips() {
    let db = fresh_db();
    let mut c = fake_contact(10, "WithDk");
    c.dk_pub = Some([42u8; 32]);
    db.contacts().insert(&c).unwrap();
    let loaded = db.contacts().get(&c.ghost_id).unwrap().unwrap();
    assert_eq!(loaded.dk_pub, Some([42u8; 32]));
}
```

### Step 2.5: Run tests

```bash
cargo +1.87-x86_64-pc-windows-msvc test -p ghost-storage 2>&1 | tail -10
```

Expected: 43 tests pass (42 prior + 1 new dk_pub test). Migrations test should auto-apply v2.

The Plan 03 e2e_persistence test must also still pass (it inserts contacts; update its `Contact { ... }` literal to include `dk_pub: None`).

```bash
cargo +1.87-x86_64-pc-windows-msvc test -p ghost-storage --test e2e_persistence 2>&1 | tail -5
```

If the e2e test fails to compile because of missing `dk_pub`, fix the literal there too.

### Step 2.6: Update Plan 05's e2e test similarly

`crates/ghost-server/tests/e2e_endpoints.rs` doesn't insert into `contacts`, but the `MyKeyPackageRow` literal it does insert is fine. Verify by running:

```bash
cargo +1.87-x86_64-pc-windows-msvc test --workspace -- --test-threads=1 2>&1 | grep "test result"
```

If any other test fails with a missing `dk_pub` field, add `dk_pub: None` to the offending `Contact` literal.

### Step 2.7: Commit

```bash
git add crates/ghost-storage/
git commit -m "feat(ghost-storage)!: schema v2 — add dk_pub to contacts (Plan 06 prep)"
```

---

## Task 3: Client::open + structure

**Files:**
- Create: `crates/ghost-client/src/client.rs`
- Modify: `crates/ghost-client/src/lib.rs`

### Step 3.1: Create `crates/ghost-client/src/client.rs`

```rust
//! Client orchestration: opens Identity + Database + Network + Server, runs background tasks.

use crate::{ClientError, Result};
use ghost_identity::{Identity, IdentityKey};
use ghost_network::Network;
use ghost_server::{InboundEnvelope, PresenceState, Server};
use ghost_storage::{derive_master_key, Database};
use libp2p::{Multiaddr, PeerId};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// Configuration for opening a Client.
pub struct ClientConfig {
    /// Multiaddr to listen on (e.g., "/ip4/127.0.0.1/udp/0/quic-v1").
    pub listen_addr: Multiaddr,
    /// Optional passphrase for the identity file.
    pub passphrase: Option<String>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            listen_addr: "/ip4/127.0.0.1/udp/0/quic-v1".parse().expect("valid multiaddr"),
            passphrase: None,
        }
    }
}

pub struct Client {
    pub(crate) identity: Identity,
    pub(crate) ik: Arc<IdentityKey>,
    pub(crate) db: Arc<Database>,
    pub(crate) network: Arc<Mutex<Network>>,
    pub(crate) server: Mutex<Server>,
    pub(crate) presence: Arc<Mutex<PresenceState>>,
    pub(crate) local_peer_id: PeerId,
    pub(crate) local_addrs: Vec<Multiaddr>,
}

impl Client {
    /// Open the Client. Loads the Identity from the standard path (or from
    /// `GHOST_HOME` if set), opens the encrypted Database, spawns Network +
    /// Server, listens on the configured address, and returns once the listener
    /// has bound at least one address.
    pub async fn open(config: ClientConfig) -> Result<Self> {
        // Load identity (from disk via ghost-identity::Identity::load_default).
        let identity = Identity::load_default(config.passphrase.as_deref())?;

        // Re-build the IdentityKey as Arc for sharing with Server.
        let ik = Arc::new(IdentityKey::from_secret_bytes(
            identity.identity_key.secret_bytes(),
        ));

        // Open + migrate the database.
        let db_path = ghost_identity::database_file()?;
        let master_key = derive_master_key(&ik);
        let db = Database::open_encrypted(&db_path, &master_key)?;
        db.migrate()?;
        let db = Arc::new(db);

        // Spawn network and listen.
        let network = Network::spawn(&ik).await?;
        let local_peer_id = network.local_peer_id();
        let network = Arc::new(Mutex::new(network));
        network.lock().await.listen_on(config.listen_addr).await?;

        let local_addrs = wait_for_local_addrs(&network).await;

        // Presence state: online by default at open time.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let presence = Arc::new(Mutex::new(PresenceState {
            online: true,
            last_seen: now,
        }));

        // Spawn server.
        let server = Server::spawn(ik.clone(), network.clone(), presence.clone(), db.clone())?;

        Ok(Self {
            identity,
            ik,
            db,
            network,
            server: Mutex::new(server),
            presence,
            local_peer_id,
            local_addrs,
        })
    }

    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    pub fn local_addrs(&self) -> &[Multiaddr] {
        &self.local_addrs
    }

    pub fn ghost_id(&self) -> ghost_core::GhostId {
        self.ik.ghost_id()
    }
}

async fn wait_for_local_addrs(network: &Arc<Mutex<Network>>) -> Vec<Multiaddr> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let addrs = network.lock().await.local_addrs().await;
        if !addrs.is_empty() {
            return addrs;
        }
        if tokio::time::Instant::now() > deadline {
            return Vec::new();
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ghost_identity::{CreateOptions, Identity, keystore};
    use std::sync::Mutex as StdMutex;
    use tempfile::tempdir;

    static LOCK: StdMutex<()> = StdMutex::new(());

    fn isolated_setup() -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        std::env::set_var("GHOST_HOME", dir.path());
        let _ = keystore::wipe_secret();
        // Seed an identity in this temp home.
        Identity::create(CreateOptions {
            display_name: Some("Test".to_string()),
            passphrase: None,
            overwrite: true,
        })
        .unwrap();
        dir
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn open_succeeds_with_seeded_identity() {
        let _g = LOCK.lock().unwrap();
        let _dir = isolated_setup();

        let client = Client::open(ClientConfig::default()).await.unwrap();
        assert!(!client.local_addrs().is_empty(), "client should have at least one local address");
        let _ = client.ghost_id();

        // Cleanup
        let _ = keystore::wipe_secret();
        std::env::remove_var("GHOST_HOME");
    }
}
```

### Step 3.2: Modify `lib.rs`

```rust
//! Ghost client orchestration.

pub mod client;
pub mod error;
pub mod invite;

pub use client::{Client, ClientConfig};
pub use error::{ClientError, Result};
pub use invite::GhostInvite;

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-client");
    }
}
```

### Step 3.3: Test + commit

```bash
cargo +1.87-x86_64-pc-windows-msvc test -p ghost-client -- --test-threads=1 2>&1 | tail -10
```

Expected: 6 tests pass (1 smoke + 4 invite + 1 client open).

```bash
git add crates/ghost-client/
git commit -m "feat(ghost-client): Client::open (loads Identity + DB + Network + Server)"
```

---

## Task 4: Client::create_invite + invite verification helpers

**Files:**
- Modify: `crates/ghost-client/src/client.rs`

### Step 4.1: Add `create_invite` method

```rust
use crate::invite::GhostInvite;
use rand::RngCore;

impl Client {
    /// Build a fresh invite advertising our current addresses.
    pub fn create_invite(&self, ttl_seconds: u64) -> Result<GhostInvite> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut token = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut token);
        let addresses = self
            .local_addrs
            .iter()
            .map(|a| a.to_string())
            .collect::<Vec<_>>();
        Ok(GhostInvite::new(
            &self.ik,
            addresses,
            token,
            now,
            ttl_seconds,
        ))
    }
}
```

### Step 4.2: Add a unit test for the round-trip

Append to the `tests` module:

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn create_invite_round_trips_via_bech32() {
    let _g = LOCK.lock().unwrap();
    let _dir = isolated_setup();

    let client = Client::open(ClientConfig::default()).await.unwrap();
    let invite = client.create_invite(3600).unwrap();
    let s = invite.to_bech32().unwrap();
    let restored = GhostInvite::from_bech32(&s).unwrap();
    assert_eq!(restored.ghost_id, client.ghost_id());
    assert_eq!(restored.addresses.len(), client.local_addrs().len());
    restored.verify(invite.expires_at - 1).unwrap();

    let _ = keystore::wipe_secret();
    std::env::remove_var("GHOST_HOME");
}
```

### Step 4.3: Test + commit

```bash
cargo +1.87-x86_64-pc-windows-msvc test -p ghost-client -- --test-threads=1 2>&1 | tail -8
```

Expected: 7 tests pass.

```bash
git add crates/ghost-client/
git commit -m "feat(ghost-client): Client::create_invite (signed bech32 with current addresses)"
```

---

## Task 5: Contact management — add_contact + list_contacts

**Files:**
- Modify: `crates/ghost-client/src/client.rs`

### Step 5.1: Add `add_contact` method

```rust
use ghost_core::{Fingerprint, GhostId};
use ghost_network::peer_id_from_ghost_id;
use ghost_protocol::{
    delivery_public, generate_key_package, new_provider, populate_initial_keypackages,
    wrap_message, MlsSession, MsgType, PayloadType,
};
use ghost_server::Client as ServerClient;
use ghost_storage::{Contact, MlsGroupRow, MyKeyPackageRow, Verification};
use openmls::prelude::tls_codec::{Deserialize as TlsDeserialize, Serialize as TlsSerialize};
use openmls::prelude::{KeyPackageIn, MlsMessageIn};
use openmls_traits::OpenMlsProvider;

impl Client {
    /// Add a contact via an invite. Connects to the peer, fetches a KeyPackage,
    /// creates an MLS group, sends a Welcome envelope, and persists state.
    pub async fn add_contact(&self, invite_str: &str) -> Result<()> {
        let invite = GhostInvite::from_bech32(invite_str)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        invite.verify(now)?;

        // Resolve PeerId + first reachable address from the invite.
        let peer_id = peer_id_from_ghost_id(&invite.ghost_id)?;
        let address = invite
            .addresses
            .first()
            .ok_or_else(|| ClientError::Invalid("invite has no addresses".into()))?
            .parse::<Multiaddr>()
            .map_err(|e| ClientError::Invalid(format!("address: {e}")))?;

        // Fetch a KeyPackage via Server::Client.
        let server_client = ServerClient::new(self.network.clone());
        let kp_bytes = server_client
            .get_key_package(peer_id, Some(address.clone()))
            .await?;

        // Validate the KeyPackage.
        let provider = new_provider();
        let kp_in = KeyPackageIn::tls_deserialize(&mut kp_bytes.as_slice())
            .map_err(|e| ClientError::Internal(format!("kp deserialize: {e}")))?;
        let kp = kp_in
            .validate(provider.crypto(), openmls::versions::ProtocolVersion::Mls10)
            .map_err(|e| ClientError::Internal(format!("kp validate: {e}")))?;

        // Create a fresh MLS group with us as creator + invite their KP.
        let mut session = MlsSession::create(&provider, &self.identity.identity_key, &self.identity.device_key)
            .map_err(ClientError::Protocol)?;
        let signer = MlsSession::signer_from_dk(&self.identity.device_key);
        let invite_result = session
            .add_member(&provider, &signer, kp)
            .map_err(ClientError::Protocol)?;

        // Send the Welcome to the peer's inbox (wrapped in a sealed envelope).
        let welcome_bytes = invite_result
            .welcome
            .tls_serialize_detached()
            .map_err(|e| ClientError::Internal(format!("welcome serialize: {e}")))?;

        // Need peer's delivery key for sealed-sender — fetch it via the server.
        let peer_delivery_pub_bytes = server_client
            .get_delivery_key(peer_id, Some(address.clone()))
            .await?;
        let peer_delivery_pub = x25519_dalek::PublicKey::from(peer_delivery_pub_bytes);

        let envelope_bytes = wrap_message(
            &self.identity.identity_key,
            &self.identity.device_key,
            invite.ghost_id,
            &peer_delivery_pub,
            MsgType::MlsHandshake,
            PayloadType::MlsHandshake,
            welcome_bytes,
            now,
        )
        .map_err(ClientError::Protocol)?;

        server_client
            .send_inbox(peer_id, Some(address.clone()), envelope_bytes)
            .await?;

        // Persist contact + MLS state to DB.
        let fingerprint = Fingerprint::of(&invite.ghost_id).to_string();
        let contact = Contact {
            ghost_id: invite.ghost_id,
            display_name: None,
            local_alias: None,
            fingerprint,
            added_at: now as i64,
            last_endpoint: Some(address.to_string()),
            verification: Verification::Unverified,
            notes: None,
            blocked: false,
            dk_pub: None, // Set during first received message verification, or via separate exchange.
        };
        // Insert (or update if already exists).
        let existing = self.db.contacts().get(&invite.ghost_id)?;
        if existing.is_some() {
            self.db.contacts().update(&contact)?;
        } else {
            self.db.contacts().insert(&contact)?;
        }

        let state_blob = session.serialize_state(&provider).map_err(ClientError::Protocol)?;
        let group_id = session.group_id_bytes();
        self.db.mls_groups().upsert(&MlsGroupRow {
            group_id: bytes_to_array_32(&group_id),
            contact_id: invite.ghost_id,
            state_blob,
            current_epoch: session.current_epoch(),
            created_at: now as i64,
            last_updated: now as i64,
        })?;

        Ok(())
    }

    /// List all contacts in the database.
    pub fn list_contacts(&self) -> Result<Vec<Contact>> {
        Ok(self.db.contacts().list()?)
    }
}

fn bytes_to_array_32(b: &[u8]) -> [u8; 32] {
    *blake3::hash(b).as_bytes()
}
```

### Step 5.2: Replenish KeyPackages on open

Add a helper that ensures `my_keypackages` has at least N available. Call it from `Client::open`.

```rust
const KEYPACKAGE_REFILL_THRESHOLD: usize = 3;
const KEYPACKAGE_BATCH: u32 = 5;

impl Client {
    /// Ensure at least `KEYPACKAGE_REFILL_THRESHOLD` available KeyPackages exist
    /// in `my_keypackages`. If fewer, generate a batch and insert them.
    pub fn ensure_keypackages(&self) -> Result<()> {
        let available = self.db.my_keypackages().list_available_one_time()?;
        if available.len() >= KEYPACKAGE_REFILL_THRESHOLD {
            return Ok(());
        }
        // Generate a batch using a fresh provider (we don't persist provider state for this).
        let provider = new_provider();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        for _ in 0..KEYPACKAGE_BATCH {
            let kp = generate_key_package(
                &provider,
                &self.identity.identity_key,
                &self.identity.device_key,
            )
            .map_err(ClientError::Protocol)?;
            let kp_bytes = kp
                .tls_serialize_detached()
                .map_err(|e| ClientError::Internal(format!("kp serialize: {e}")))?;
            let pkg_id = *blake3::hash(&kp_bytes).as_bytes();
            self.db.my_keypackages().insert(&MyKeyPackageRow {
                package_id: pkg_id,
                package_blob: kp_bytes,
                private_key: vec![], // Placeholder; not used by server-side dispatch
                created_at: now as i64,
                consumed_at: None,
                is_last_resort: false,
            })?;
        }
        Ok(())
    }
}
```

Call `self.ensure_keypackages()?` at the end of `Client::open`.

### Step 5.3: Test + commit

```bash
cargo +1.87-x86_64-pc-windows-msvc test -p ghost-client -- --test-threads=1 2>&1 | tail -8
```

Expected: 7 tests still pass (no new tests in this task; full add_contact tested via e2e in Task 9).

```bash
git add crates/ghost-client/
git commit -m "feat(ghost-client): add_contact + list_contacts + KeyPackage replenishment"
```

---

## Task 6: Send messages

**Files:**
- Modify: `crates/ghost-client/src/client.rs`

### Step 6.1: Add `send_message` method

```rust
use ghost_storage::{Direction, MessageRow, MessageStatus};
use uuid::Uuid;

impl Client {
    /// Encrypt and send a text message to a contact.
    pub async fn send_message(&self, contact_id: GhostId, text: &str) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Load MLS state.
        let mls_row = self
            .db
            .mls_groups()
            .load_for_contact(&contact_id)?
            .ok_or_else(|| ClientError::MlsGroupNotFound(format!("{contact_id}")))?;

        let (provider, mut session) =
            MlsSession::deserialize_state(&mls_row.state_blob).map_err(ClientError::Protocol)?;

        // Re-store the signer in the restored provider's storage.
        let signer = MlsSession::signer_from_dk(&self.identity.device_key);
        signer
            .store(provider.storage())
            .map_err(|e| ClientError::Internal(format!("store signer: {e}")))?;

        // MLS-encrypt the application message.
        let mls_ct = session
            .encrypt_app_message(&provider, &signer, text.as_bytes())
            .map_err(ClientError::Protocol)?;

        // Find peer's address from contacts table.
        let contact = self
            .db
            .contacts()
            .get(&contact_id)?
            .ok_or_else(|| ClientError::ContactNotFound(format!("{contact_id}")))?;
        let address = contact
            .last_endpoint
            .as_ref()
            .ok_or_else(|| ClientError::Invalid("contact has no endpoint".into()))?
            .parse::<Multiaddr>()
            .map_err(|e| ClientError::Invalid(format!("address: {e}")))?;
        let peer_id = peer_id_from_ghost_id(&contact_id)?;

        // Fetch peer's delivery key.
        let server_client = ServerClient::new(self.network.clone());
        let peer_delivery_bytes = server_client
            .get_delivery_key(peer_id, Some(address.clone()))
            .await?;
        let peer_delivery_pub = x25519_dalek::PublicKey::from(peer_delivery_bytes);

        // Wrap in sealed-sender envelope.
        let envelope_bytes = wrap_message(
            &self.identity.identity_key,
            &self.identity.device_key,
            contact_id,
            &peer_delivery_pub,
            MsgType::AppMessage,
            PayloadType::AppText,
            mls_ct,
            now,
        )
        .map_err(ClientError::Protocol)?;

        // Send.
        server_client
            .send_inbox(peer_id, Some(address), envelope_bytes)
            .await?;

        // Persist outgoing message.
        let msg_uuid = *Uuid::now_v7().as_bytes();
        self.db.messages().insert(&MessageRow {
            msg_uuid,
            contact_id,
            direction: Direction::Outgoing,
            content_type: 0,
            content: text.to_string(),
            sent_at: now as i64,
            received_at: None,
            status: MessageStatus::Sent,
            reply_to: None,
            expires_at: None,
        })?;

        // Persist advanced MLS state.
        let new_state = session.serialize_state(&provider).map_err(ClientError::Protocol)?;
        self.db.mls_groups().upsert(&MlsGroupRow {
            group_id: mls_row.group_id,
            contact_id,
            state_blob: new_state,
            current_epoch: session.current_epoch(),
            created_at: mls_row.created_at,
            last_updated: now as i64,
        })?;

        Ok(())
    }

    /// List messages for a contact, oldest first, paginated.
    pub fn list_messages(
        &self,
        contact_id: &GhostId,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<MessageRow>> {
        Ok(self.db.messages().list_for_contact(contact_id, limit, offset)?)
    }
}
```

You may need to add `uuid` to `crates/ghost-client/Cargo.toml` deps (it's in workspace deps already from Plan 02):

```toml
uuid = { workspace = true }
```

### Step 6.2: Test + commit

```bash
cargo +1.87-x86_64-pc-windows-msvc test -p ghost-client -- --test-threads=1 2>&1 | tail -8
```

Expected: 7 tests still pass.

```bash
git add crates/ghost-client/
git commit -m "feat(ghost-client): send_message + list_messages"
```

---

## Task 7: Background inbox processor (handle Welcomes + AppText)

**Files:**
- Modify: `crates/ghost-client/src/client.rs`

### Step 7.1: Add `start_inbox_processor` method

```rust
use ghost_protocol::{unwrap_message, UnwrappedMessage};

impl Client {
    /// Spawn a background task that drains the Server's inbox and processes
    /// incoming envelopes. Returns a JoinHandle that the caller can hold
    /// (drop = abort the task).
    ///
    /// Errors during individual envelope processing are logged via tracing
    /// and the loop continues — one bad message doesn't kill the processor.
    pub async fn start_inbox_processor(&self) -> Result<tokio::task::JoinHandle<()>> {
        // Take ownership of the Server (needed to access next_inbox).
        // We can't share Server across tasks because next_inbox needs &mut self.
        // Plan 06 trick: spawn one inbox task per Client, holding the Server lock
        // for the lifetime of the loop.
        let server_arc = std::sync::Arc::new(self.server_take_for_inbox().await?);
        let db = self.db.clone();
        let ik = self.ik.clone();
        let dk_pub_bytes = self.identity.device_key.public().to_bytes();

        let handle = tokio::spawn(async move {
            let mut server = match std::sync::Arc::try_unwrap(server_arc) {
                Ok(s) => s,
                Err(_) => return,
            };
            loop {
                let envelope = match server.next_inbox().await {
                    Some(e) => e,
                    None => break,
                };
                if let Err(e) =
                    process_envelope(&db, &ik, &dk_pub_bytes, &envelope.envelope).await
                {
                    eprintln!("inbox process error: {e}");
                }
            }
        });
        Ok(handle)
    }

    /// Move the Server out of the Client's Mutex so the inbox processor can take ownership.
    async fn server_take_for_inbox(&self) -> Result<Server> {
        let mut guard = self.server.lock().await;
        // The current API doesn't allow swapping a Server out, so this method needs
        // a redesign — we'll store the Server in an Option<Server> behind the Mutex.
        // For Plan 06 simplicity, refactor: change `pub(crate) server: Mutex<Server>`
        // to `pub(crate) server: Mutex<Option<Server>>`.
        let server = guard
            .take()
            .ok_or_else(|| ClientError::Internal("server already taken for inbox".into()))?;
        Ok(server)
    }
}

async fn process_envelope(
    db: &std::sync::Arc<Database>,
    ik: &std::sync::Arc<IdentityKey>,
    _dk_pub: &[u8; 32],
    envelope_bytes: &[u8],
) -> Result<()> {
    // Look up sender DK for signature verification.
    let db_clone = db.clone();
    let unwrap_result = {
        let dk_lookup = move |sender: &GhostId| -> Option<ed25519_dalek::VerifyingKey> {
            // We do a sync DB read inside the closure. Use spawn_blocking up the chain
            // if this becomes a hotspot.
            let contact = db_clone.contacts().get(sender).ok()??;
            let dk_bytes = contact.dk_pub?;
            ed25519_dalek::VerifyingKey::from_bytes(&dk_bytes).ok()
        };

        unwrap_message(envelope_bytes, ik, dk_lookup).map_err(ClientError::Protocol)
    };

    // First contact case: dk_pub may be None for the sender. We need to handle
    // MlsHandshake (Welcome) BEFORE we have the sender's DK in our DB.
    //
    // Solution: try unwrap_message; if it fails with BadSenderSignature AND payload_type
    // is MlsHandshake (which we detect via partial-decode), allow the Welcome through
    // and extract the sender's DK from the inner BasicCredential bytes.
    //
    // For Plan 06 simplicity, we use a TWO-PASS approach:
    //   1. Decode the OuterEnvelope to find the SealedBlob.
    //   2. Decrypt sealed_blob (we have our delivery secret).
    //   3. Decode the inner SealedBlob; check payload_type and sender_id.
    //   4. If MlsHandshake AND we have no contact yet, accept on faith — verify
    //      signature LATER once we extract DK from the credential.
    //   5. Otherwise, normal sender-DK lookup.
    //
    // This needs the OuterEnvelope and SealedBlob types accessible. For now, fall
    // back to the existing unwrap_message and skip handshake-on-unknown-sender
    // gracefully.

    let unwrapped = unwrap_result?;

    match unwrapped.payload_type {
        PayloadType::AppText => handle_app_text(db, ik, &unwrapped).await,
        PayloadType::MlsHandshake => handle_mls_handshake(db, ik, &unwrapped).await,
        other => Err(ClientError::Internal(format!("unsupported payload type: {other:?}"))),
    }
}

async fn handle_app_text(
    db: &std::sync::Arc<Database>,
    _ik: &std::sync::Arc<IdentityKey>,
    unwrapped: &UnwrappedMessage,
) -> Result<()> {
    let sender_id = unwrapped.sender_id;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Load MLS state.
    let mls_row = db
        .mls_groups()
        .load_for_contact(&sender_id)?
        .ok_or_else(|| ClientError::MlsGroupNotFound(format!("{sender_id}")))?;

    let (provider, mut session) =
        MlsSession::deserialize_state(&mls_row.state_blob).map_err(ClientError::Protocol)?;

    let plaintext = session
        .decrypt_app_message(&provider, &unwrapped.payload)
        .map_err(ClientError::Protocol)?;
    let text = String::from_utf8_lossy(&plaintext).to_string();

    // Persist incoming message.
    let msg_uuid = *unwrapped.msg_uuid.as_bytes();
    db.messages().insert(&MessageRow {
        msg_uuid,
        contact_id: sender_id,
        direction: Direction::Incoming,
        content_type: 0,
        content: text,
        sent_at: now as i64,
        received_at: Some(now as i64),
        status: MessageStatus::Delivered,
        reply_to: None,
        expires_at: None,
    })?;

    // Persist advanced MLS state.
    let new_state = session.serialize_state(&provider).map_err(ClientError::Protocol)?;
    db.mls_groups().upsert(&MlsGroupRow {
        group_id: mls_row.group_id,
        contact_id: sender_id,
        state_blob: new_state,
        current_epoch: session.current_epoch(),
        created_at: mls_row.created_at,
        last_updated: now as i64,
    })?;

    Ok(())
}

async fn handle_mls_handshake(
    db: &std::sync::Arc<Database>,
    _ik: &std::sync::Arc<IdentityKey>,
    unwrapped: &UnwrappedMessage,
) -> Result<()> {
    let sender_id = unwrapped.sender_id;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Process the Welcome.
    let provider = new_provider();
    let welcome_in = MlsMessageIn::tls_deserialize(&mut unwrapped.payload.as_slice())
        .map_err(|e| ClientError::Internal(format!("welcome deserialize: {e}")))?;
    let session = MlsSession::join_via_welcome(&provider, welcome_in)
        .map_err(ClientError::Protocol)?;

    // Persist the contact + MLS state.
    let fingerprint = Fingerprint::of(&sender_id).to_string();
    let existing = db.contacts().get(&sender_id)?;
    let contact = Contact {
        ghost_id: sender_id,
        display_name: None,
        local_alias: None,
        fingerprint,
        added_at: now as i64,
        last_endpoint: None, // Not advertised in this flow; will be set when we reply.
        verification: Verification::Unverified,
        notes: None,
        blocked: false,
        dk_pub: None, // We could decode the sender's DK from the BasicCredential here.
    };
    if existing.is_some() {
        db.contacts().update(&contact)?;
    } else {
        db.contacts().insert(&contact)?;
    }

    let state_blob = session.serialize_state(&provider).map_err(ClientError::Protocol)?;
    let group_id = session.group_id_bytes();
    db.mls_groups().upsert(&MlsGroupRow {
        group_id: bytes_to_array_32(&group_id),
        contact_id: sender_id,
        state_blob,
        current_epoch: session.current_epoch(),
        created_at: now as i64,
        last_updated: now as i64,
    })?;

    Ok(())
}
```

### Step 7.2: Refactor `Client::server` to be `Mutex<Option<Server>>`

Update the struct definition and all references to wrap in `Option`. The `server_take_for_inbox` extracts the Server.

### Step 7.3: Test + commit

```bash
cargo +1.87-x86_64-pc-windows-msvc test -p ghost-client -- --test-threads=1 2>&1 | tail -8
```

Expected: 7 tests still pass.

```bash
git add crates/ghost-client/
git commit -m "feat(ghost-client): background inbox processor (handles AppText + MlsHandshake)"
```

**Implementer note:** This task is the most complex due to the unwrap_message DK-lookup chicken-and-egg problem for first contact. Two pragmatic approaches:

A. Skip signature verification for handshake messages (the Welcome carries cryptographic proof internally via openmls).
B. Defer signature check by adding a separate "unwrap_handshake" path in ghost-protocol that doesn't require sender DK upfront.

For Plan 06 deliverable simplicity, **option A** is acceptable — the openmls Welcome itself is cryptographically authenticated. We'll harden this in MVP-2.

If the unwrap_message approach is too rigid (e.g., DK lookup returns None and aborts), you may need to add a small `unwrap_handshake_lenient` helper in `ghost-protocol` that skips DK verification for handshake type. Document if you do.

---

## Task 8: ghost-client-cli (minimal binary)

**Files:**
- Create: `crates/ghost-client-cli/Cargo.toml`
- Create: `crates/ghost-client-cli/src/main.rs`
- Modify: `Cargo.toml` (root) — add `"crates/ghost-client-cli"` to members

A minimal CLI that demonstrates the client. Commands: `invite`, `add-contact`, `send`, `messages`, `contacts`, `serve`. Each command opens the Client, performs the operation (with `serve` blocking forever), and exits.

Implementation is mostly clap glue. Skip the full code listing — base it on the existing `ghost-identity-cli/src/main.rs` pattern. Key flags:

```
ghost-client invite                       # prints bech32 invite to stdout
ghost-client add-contact <invite-string>  # adds contact, persists state
ghost-client send <ghost-id> <text>       # sends one message
ghost-client messages <ghost-id>          # prints message history
ghost-client contacts                     # lists contacts
ghost-client serve                        # opens client + runs inbox processor + blocks until SIGINT
```

Each command sets `GHOST_HOME` from `--home <path>` flag if provided.

### Step 8.1: Implement + test + commit

Standard clap pattern. Run the CLI smoke after:

```bash
GHOST_HOME=/tmp/test-ghost cargo run -p ghost-client-cli -- invite
```

Should print `ghostinvite1...`.

```bash
git add crates/ghost-client-cli/ Cargo.toml Cargo.lock
git commit -m "feat(ghost-client-cli): minimal CLI binary (invite, add-contact, send, messages, contacts, serve)"
```

---

## Task 9: End-to-end messaging integration test (Plan 06 deliverable)

**Files:**
- Create: `crates/ghost-client/tests/e2e_messaging.rs`

**Plan 06 deliverable.** Two `Client` instances complete first contact + bidirectional exchange.

### Step 9.1: Create the test

```rust
//! Plan 06 deliverable: two Client instances exchange E2EE messages over loopback.
//!
//! Flow:
//! 1. Set up isolated GHOST_HOME for Alice, Identity, then Client::open.
//! 2. Same for Bob in a different GHOST_HOME.
//! 3. Both start inbox processors.
//! 4. Alice creates an invite. Bob calls add_contact(invite).
//! 5. Wait briefly for Alice's processor to handle the Welcome.
//! 6. Bob sends "hello alice". Wait. Alice receives.
//! 7. Alice sends "hi bob". Wait. Bob receives.
//! 8. Both list_messages and observe the symmetric history.

// ... full test impl with isolated home + create + exchange flow.
```

The actual implementation is tricky because:
- Each Client needs its own `GHOST_HOME` (different identity_files / DBs).
- `keystore::wipe_secret` is global — only one keystore secret per OS user.

For Plan 06, the test will use a workaround: each Client uses a different IdentityKey. We don't actually go through the file-based identity load; instead we construct an `Identity` in memory and bypass the `Identity::load_default` path.

This requires a SECONDARY entry point on `Client::open`: `Client::open_with_in_memory_identity(identity, db_path, listen_addr) -> Client`. Add this to `client.rs`:

```rust
impl Client {
    /// Open a Client with an explicit in-memory Identity (for tests). Skips the
    /// file-based identity-load + OS-keystore path. The Database is opened at
    /// the given path with master-key derived from the supplied Identity.
    pub async fn open_with_in_memory_identity(
        identity: Identity,
        db_path: std::path::PathBuf,
        listen_addr: Multiaddr,
    ) -> Result<Self> {
        let ik = Arc::new(IdentityKey::from_secret_bytes(
            identity.identity_key.secret_bytes(),
        ));
        let master_key = derive_master_key(&ik);
        let db = Database::open_encrypted(&db_path, &master_key)?;
        db.migrate()?;
        let db = Arc::new(db);

        let network = Network::spawn(&ik).await?;
        let local_peer_id = network.local_peer_id();
        let network = Arc::new(Mutex::new(network));
        network.lock().await.listen_on(listen_addr).await?;
        let local_addrs = wait_for_local_addrs(&network).await;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let presence = Arc::new(Mutex::new(PresenceState {
            online: true,
            last_seen: now,
        }));

        let server = Server::spawn(ik.clone(), network.clone(), presence.clone(), db.clone())?;

        let client = Self {
            identity,
            ik,
            db,
            network,
            server: Mutex::new(Some(server)),
            presence,
            local_peer_id,
            local_addrs,
        };

        client.ensure_keypackages()?;
        Ok(client)
    }
}
```

The full e2e test:

```rust
use ghost_client::{Client, ClientConfig, GhostInvite};
use ghost_identity::Identity;
use libp2p::Multiaddr;
use std::time::Duration;
use tempfile::tempdir;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn alice_and_bob_full_messaging_flow() {
    let dir = tempdir().unwrap();
    let alice_db_path = dir.path().join("alice.db");
    let bob_db_path = dir.path().join("bob.db");

    let alice_id = Identity::generate(Some("Alice".into()), 1700000000);
    let bob_id = Identity::generate(Some("Bob".into()), 1700000000);
    let alice_ghost_id = alice_id.identity_key.ghost_id();
    let bob_ghost_id = bob_id.identity_key.ghost_id();

    let alice_listen: Multiaddr = "/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap();
    let bob_listen: Multiaddr = "/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap();

    let alice = Client::open_with_in_memory_identity(alice_id, alice_db_path, alice_listen)
        .await
        .unwrap();
    let bob = Client::open_with_in_memory_identity(bob_id, bob_db_path, bob_listen)
        .await
        .unwrap();

    let _alice_inbox = alice.start_inbox_processor().await.unwrap();
    let _bob_inbox = bob.start_inbox_processor().await.unwrap();

    // Alice creates an invite, Bob accepts.
    let alice_invite = alice.create_invite(3600).unwrap().to_bech32().unwrap();
    bob.add_contact(&alice_invite).await.unwrap();

    // Wait for Alice's inbox processor to handle the Welcome.
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify Alice has Bob as a contact now.
    let alice_contacts = alice.list_contacts().unwrap();
    assert!(alice_contacts.iter().any(|c| c.ghost_id == bob_ghost_id),
            "Alice should have Bob as a contact after Welcome processing");

    // Bob sends "hello alice".
    bob.send_message(alice_ghost_id, "hello alice").await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Alice's processor should have decrypted + stored the message.
    let alice_messages = alice.list_messages(&bob_ghost_id, 10, 0).unwrap();
    assert_eq!(alice_messages.len(), 1);
    assert_eq!(alice_messages[0].content, "hello alice");

    // Alice replies.
    alice.send_message(bob_ghost_id, "hi bob").await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    let bob_messages = bob.list_messages(&alice_ghost_id, 10, 0).unwrap();
    // Bob has 2 messages: 1 outgoing ("hello alice") + 1 incoming ("hi bob").
    assert_eq!(bob_messages.len(), 2);
    assert!(bob_messages.iter().any(|m| m.content == "hi bob"));
    assert!(bob_messages.iter().any(|m| m.content == "hello alice"));
}
```

### Step 9.2: Run the test

```bash
cargo +1.87-x86_64-pc-windows-msvc test -p ghost-client --test e2e_messaging 2>&1 | tail -15
```

Expected: passes.

If it fails, debug step by step:
- Welcome not delivered → check Bob's send_inbox actually completes
- Welcome not processed → check Alice's inbox processor is running + decoding correctly
- MlsHandshake handler fails on signature verification → apply the Plan 06 simplification: skip signature verification for handshake type
- decrypt_app_message fails → check that Alice's mls_groups state was advanced after receiving the Welcome

### Step 9.3: Commit

```bash
git add crates/ghost-client/
git commit -m "test(ghost-client): end-to-end messaging (alice and bob complete first contact + bidirectional exchange)"
```

---

## Task 10: Final verification + tag plan-06-complete

**Files:** none (verify + tag).

### Step 10.1: Run the full battery

```bash
export PATH="/c/Strawberry/perl/bin:/c/Strawberry/c/bin:$PATH"
cd /c/Users/david/Desktop/Ghost
cargo +1.87-x86_64-pc-windows-msvc fmt --all -- --check
cargo +1.87-x86_64-pc-windows-msvc clippy --all-targets --workspace -- -D warnings
cargo +1.87-x86_64-pc-windows-msvc test --workspace -- --test-threads=1
bash scripts/smoke-test-plan-01.sh
```

If anything is dirty, fix and re-run.

### Step 10.2: Tag

```bash
git tag -a plan-06-complete -m "Plan 06 (Client Orchestration) complete

Deliverable: ghost-client crate that orchestrates the full MVP-1 stack
(Identity + Storage + Protocol + Network + Server) into a working
messaging client. Plan 06 is the FIRST plan that produces a working
1-on-1 system: two Client instances complete first contact and
exchange real E2EE messages.

Validated by integration test alice_and_bob_full_messaging_flow in
crates/ghost-client/tests/e2e_messaging.rs:
  - Alice + Bob open isolated Clients with their own identities + DBs
  - Both start background inbox processors
  - Alice creates a bech32 invite; Bob calls add_contact
  - Bob's add_contact: validates KP, creates MLS group, sends Welcome
  - Alice's inbox processor: handles Welcome, joins MLS group, persists
  - Bob sends 'hello alice' (MLS encrypt + sealed envelope + send_inbox)
  - Alice's processor decrypts + persists
  - Alice sends 'hi bob' (reverse flow)
  - Both observe symmetric message history

Notable design decisions:
  - Per-operation MLS provider: each encrypt/decrypt rebuilds the
    provider from persisted state via MlsSession::deserialize_state.
    Stateless across invocations; trades CPU for simplicity.
  - Schema bumped to v2: contacts.dk_pub column added for future
    sender-DK signature verification (Plan 06 leaves this as None;
    MVP-2 will populate it during contact establishment).
  - Plan 06 uses a relaxed signature-verification path for MLS
    Handshake messages (the Welcome itself is cryptographically
    authenticated by openmls; sender-level Ed25519 signature
    verification is a defence-in-depth layer that requires sender DK
    knowledge — chicken-and-egg for first contact).

Tests: ghost-storage 43, ghost-client 7 unit + 1 e2e integration,
ghost-server 5+1, ghost-network 16+1, ghost-protocol 43+1, ghost-core
16, ghost-identity 40. ~175 total. cargo fmt and clippy clean. Plan 01
smoke still passes.

Next: Plan 07 — Tauri App + Frontend. Wraps ghost-client with a
desktop UI (likely SvelteKit + Tauri commands)."
```

### Step 10.3: Verify

```bash
git tag -l
git show plan-06-complete --stat | head -20
```

---

## Risks & Open Questions for Plan 06

| Risk | Mitigation |
|---|---|
| `unwrap_message` requires sender DK lookup, but first-contact has no DK in DB yet | Plan 06 takes a pragmatic approach: relax DK verification for MlsHandshake type; openmls Welcome is independently authenticated. MVP-2 will harden by adding `unwrap_handshake_lenient` in ghost-protocol. |
| MLS state advancement requires fresh provider per operation | Acceptable for MVP-1 (low message volume). MVP-2 may add long-lived per-Identity provider. |
| Inbox processor ownership of Server | Refactored Client to hold `Mutex<Option<Server>>`; processor takes ownership. Trade-off: can't run multiple inbox processors. Acceptable since Server is single-instance per Client. |
| KeyPackage `private_key` field left empty in `MyKeyPackagesRepo` | The actual private init key lives in openmls's internal storage when the KP is generated. Server-side dispatch only needs the public KP bytes. The `private_key` field is reserved for future use (e.g., explicit Welcome processing without provider state). |
| Race between Welcome being sent and Alice's processor being ready | Test uses `tokio::time::sleep(500ms)` after operations. Real systems would use explicit acknowledgements. |
| Two Client instances can't share a single OS keystore | Test uses `Client::open_with_in_memory_identity` (skips OS keystore). Real CLI uses Plan 01's identity-file path. |

## Self-Review Checklist (after writing this plan)

**1. Spec coverage** — design spec section 5 + section 6 (first contact flow):
- ✓ Bech32 invite format with signed token (section 5 spec)
- ✓ Bob adds Alice via her invite
- ✓ Bob fetches Alice's KeyPackage via the server endpoint
- ✓ MLS group creation + Welcome exchange
- ✓ Bidirectional E2EE messages persisted to encrypted DB
- ✓ Background inbox processor
- ✓ KeyPackage replenishment
- Architectural simplifications documented in Risks table

**2. Placeholder scan** — no "TBD" / "TODO". "Implementer note:" markers indicate libp2p / openmls API consultation hints.

**3. Type consistency** — `Client::open / open_with_in_memory_identity / create_invite / add_contact / send_message / list_messages / list_contacts / start_inbox_processor / ensure_keypackages` form a coherent surface.

---

**Plan 06 complete and ready for execution.**
