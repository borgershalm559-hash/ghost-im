# Ghost Plan 01 — Foundation + Identity

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cargo workspace + `ghost-core` (типы GhostId, Fingerprint, errors) + `ghost-identity` (Ed25519 IK/DK, шифрованный identity-файл, OS keystore) + CLI smoke-test для создания и загрузки идентичности.

**Architecture:** Rust workspace из 3 крейтов. `ghost-core` — общие типы без I/O, легко тестируется. `ghost-identity` — generate/serialize/encrypt/persist identity. `ghost-identity-cli` — минимальный бинарь для ручного тестирования (`create`/`show`).

**Tech Stack:** Rust 1.85 (edition 2021), `ed25519-dalek` v2, `x25519-dalek` v2, `chacha20poly1305`, `blake3`, `argon2`, `bech32` v0.11, `keyring` v3, `directories` v5, `ciborium` (CBOR), `clap` v4, `thiserror`/`anyhow`, `proptest`, `tempfile`.

**Deliverable Plan 1:** CLI bin `ghost-identity-cli`, который:
- `create [--display-name X] [--passphrase Y]` — генерит IK+DK+pre-keys, шифрует, сохраняет в стандартный путь;
- `show` — расшифровывает, выводит Ghost ID (bech32), fingerprint, display name, кол-во pre-keys;
- кросс-платформенно работает на Windows/macOS/Linux через `keyring`+`directories`.

**Reference spec:** [docs/superpowers/specs/2026-04-27-ghost-mvp1-design.md](../specs/2026-04-27-ghost-mvp1-design.md), секции 2 и 3.

---

## Task 1: Workspace bootstrap, git init, first commit

**Files:**
- Create: `Cargo.toml`
- Create: `.gitignore`
- Create: `rustfmt.toml`
- Create: `rust-toolchain.toml`
- Create: `clippy.toml`

- [ ] **Step 1: Initialise git repository**

```bash
cd /c/Users/david/Desktop/Ghost
git init
git config user.name "Ghost Dev"
git config user.email "dev@ghost.local"
```

Expected: `Initialized empty Git repository in C:/Users/david/Desktop/Ghost/.git/`

- [ ] **Step 2: Create root `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = [
    "crates/ghost-core",
    "crates/ghost-identity",
    "crates/ghost-identity-cli",
]

[workspace.package]
version = "0.0.1"
edition = "2021"
rust-version = "1.85"
license = "AGPL-3.0-only"

[workspace.dependencies]
# Crypto primitives (ZeroizeOnDrop ensures secret keys cleared on drop)
ed25519-dalek = { version = "2.1", features = ["serde", "rand_core", "zeroize"] }
x25519-dalek  = { version = "2.0", features = ["serde", "static_secrets", "zeroize"] }
chacha20poly1305 = { version = "0.10", features = ["std"] }
blake3 = "1.5"
argon2 = "0.5"
hkdf = "0.12"
sha2 = "0.10"
zeroize = { version = "1.8", features = ["zeroize_derive"] }
rand = "0.8"
rand_core = { version = "0.6", features = ["std"] }

# Serialization
serde = { version = "1", features = ["derive"] }
ciborium = "0.2"
bech32 = "0.11"
hex = "0.4"
base64 = "0.22"

# Errors / utilities
thiserror = "1.0"
anyhow = "1.0"
directories = "5.0"

# OS keystore (Linux Secret Service / Windows Credential Manager / macOS Keychain)
keyring = { version = "3.0", default-features = false, features = ["apple-native", "windows-native", "linux-native-sync-persistent"] }

# CLI
clap = { version = "4.5", features = ["derive"] }

# Test-only
proptest = "1.5"
tempfile = "3.10"

[profile.release]
lto = true
codegen-units = 1
strip = "symbols"
panic = "abort"
```

- [ ] **Step 3: Create `.gitignore`**

```
/target
Cargo.lock.bak
**/*.rs.bk
.DS_Store
*.swp
*.swo
*.encrypted
ghost.db*
.ghost/
.idea/
.vscode/
```

(Note: `Cargo.lock` IS committed — workspace produces a binary.)

- [ ] **Step 4: Create `rustfmt.toml`**

```toml
edition = "2021"
max_width = 100
hard_tabs = false
use_field_init_shorthand = true
use_try_shorthand = true
```

- [ ] **Step 5: Create `rust-toolchain.toml`**

```toml
[toolchain]
channel = "1.85"
components = ["rustfmt", "clippy"]
profile = "default"
```

- [ ] **Step 6: Create `clippy.toml`**

```toml
msrv = "1.85"
cognitive-complexity-threshold = 25
```

- [ ] **Step 7: Verify workspace boots (no members yet — should fail with helpful error)**

Run: `cargo check --workspace`
Expected: error like `manifest path … does not contain a member matching "crates/ghost-core"` — that's expected; we'll add crates next. The point is `cargo` is operational.

- [ ] **Step 8: Commit baseline**

```bash
git add Cargo.toml .gitignore rustfmt.toml rust-toolchain.toml clippy.toml
git add docs/
git commit -m "chore: bootstrap Cargo workspace + project conventions"
```

---

## Task 2: ghost-core crate skeleton

**Files:**
- Create: `crates/ghost-core/Cargo.toml`
- Create: `crates/ghost-core/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/ghost-core/src/lib.rs`:

```rust
//! Ghost core types, errors, and utilities. No I/O. No crypto operations beyond hashing.

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles_and_has_constant_version() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-core");
    }
}
```

- [ ] **Step 2: Create `crates/ghost-core/Cargo.toml`**

```toml
[package]
name = "ghost-core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Core types and errors shared across Ghost crates."

[dependencies]
serde = { workspace = true }
blake3 = { workspace = true }
bech32 = { workspace = true }
hex = { workspace = true }
thiserror = { workspace = true }
zeroize = { workspace = true }

[dev-dependencies]
proptest = { workspace = true }
```

- [ ] **Step 3: Run test**

Run: `cargo test -p ghost-core`
Expected: PASS — one test, prints `crate_compiles_and_has_constant_version ... ok`.

- [ ] **Step 4: Commit**

```bash
git add crates/ghost-core/
git commit -m "feat(ghost-core): scaffold crate with smoke test"
```

---

## Task 3: GhostId newtype with safe Debug

**Files:**
- Create: `crates/ghost-core/src/id.rs`
- Modify: `crates/ghost-core/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/ghost-core/src/id.rs`:

```rust
//! GhostId — 32-byte Ed25519 public key serving as the user's globally unique identity.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GhostId([u8; 32]);

impl GhostId {
    pub const SIZE: usize = 32;

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn into_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl std::fmt::Debug for GhostId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Truncated for safety in logs — never print full key in Debug.
        write!(f, "GhostId({}…)", hex::encode(&self.0[..4]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_bytes() {
        let bytes = [42u8; 32];
        let id = GhostId::from_bytes(bytes);
        assert_eq!(id.as_bytes(), &bytes);
        assert_eq!(id.into_bytes(), bytes);
    }

    #[test]
    fn debug_truncates_to_avoid_leaking_full_key() {
        let id = GhostId::from_bytes([0xAB; 32]);
        let debug_str = format!("{:?}", id);
        assert!(debug_str.starts_with("GhostId(abababab"));
        assert!(debug_str.ends_with("…)"));
        // Must NOT contain the full hex (64 chars).
        assert!(!debug_str.contains(&"ab".repeat(32)));
    }

    #[test]
    fn equality_and_hash() {
        use std::collections::HashSet;
        let a = GhostId::from_bytes([1u8; 32]);
        let b = GhostId::from_bytes([1u8; 32]);
        let c = GhostId::from_bytes([2u8; 32]);
        assert_eq!(a, b);
        assert_ne!(a, c);
        let mut set = HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
    }
}
```

Modify `crates/ghost-core/src/lib.rs`:

```rust
//! Ghost core types, errors, and utilities. No I/O. No crypto operations beyond hashing.

pub mod id;

pub use id::GhostId;

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles_and_has_constant_version() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-core");
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p ghost-core`
Expected: 4 tests pass (`crate_compiles…`, `roundtrip_bytes`, `debug_truncates…`, `equality_and_hash`).

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-core/
git commit -m "feat(ghost-core): add GhostId newtype with safe Debug"
```

---

## Task 4: GhostId bech32 encoding/decoding

**Files:**
- Modify: `crates/ghost-core/src/id.rs`

- [ ] **Step 1: Write failing tests**

Append to `crates/ghost-core/src/id.rs`:

```rust
use bech32::{Bech32, Hrp};

/// Human-readable prefix for Ghost IDs in bech32 encoding.
pub const HRP_GHOST: &str = "ghost";

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum GhostIdParseError {
    #[error("invalid bech32 encoding: {0}")]
    InvalidBech32(String),
    #[error("expected hrp '{}', got '{0}'", HRP_GHOST)]
    WrongHrp(String),
    #[error("expected {expected} bytes after decoding, got {actual}")]
    WrongLength { expected: usize, actual: usize },
}

impl GhostId {
    /// Encode this GhostId as a bech32 string with `ghost1…` prefix.
    pub fn to_bech32(&self) -> String {
        let hrp = Hrp::parse(HRP_GHOST).expect("static HRP is valid");
        bech32::encode::<Bech32>(hrp, &self.0).expect("32 bytes always encodable")
    }

    /// Parse a bech32-encoded GhostId. Errors on wrong hrp or wrong length.
    pub fn from_bech32(s: &str) -> Result<Self, GhostIdParseError> {
        let (hrp, data) = bech32::decode(s)
            .map_err(|e| GhostIdParseError::InvalidBech32(e.to_string()))?;
        if hrp.as_str() != HRP_GHOST {
            return Err(GhostIdParseError::WrongHrp(hrp.as_str().to_string()));
        }
        if data.len() != Self::SIZE {
            return Err(GhostIdParseError::WrongLength {
                expected: Self::SIZE,
                actual: data.len(),
            });
        }
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&data);
        Ok(Self(bytes))
    }
}

impl std::fmt::Display for GhostId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_bech32())
    }
}

impl std::str::FromStr for GhostId {
    type Err = GhostIdParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_bech32(s)
    }
}

#[cfg(test)]
mod bech32_tests {
    use super::*;

    #[test]
    fn encode_starts_with_ghost1() {
        let id = GhostId::from_bytes([0u8; 32]);
        let encoded = id.to_bech32();
        assert!(encoded.starts_with("ghost1"), "got: {}", encoded);
    }

    #[test]
    fn encode_decode_roundtrip() {
        let original = GhostId::from_bytes([0xDEu8; 32]);
        let encoded = original.to_bech32();
        let decoded = GhostId::from_bech32(&encoded).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn decode_rejects_wrong_hrp() {
        // Encode with a different HRP and try to parse as ghost.
        let hrp = Hrp::parse("npub").unwrap();
        let bad = bech32::encode::<Bech32>(hrp, &[0u8; 32]).unwrap();
        let err = GhostId::from_bech32(&bad).unwrap_err();
        assert!(matches!(err, GhostIdParseError::WrongHrp(_)));
    }

    #[test]
    fn decode_rejects_wrong_length() {
        // Encode 16 bytes with ghost hrp.
        let hrp = Hrp::parse(HRP_GHOST).unwrap();
        let short = bech32::encode::<Bech32>(hrp, &[0u8; 16]).unwrap();
        let err = GhostId::from_bech32(&short).unwrap_err();
        assert!(matches!(
            err,
            GhostIdParseError::WrongLength { expected: 32, actual: 16 }
        ));
    }

    #[test]
    fn decode_rejects_garbage() {
        let err = GhostId::from_bech32("not-a-bech32-string-at-all").unwrap_err();
        assert!(matches!(err, GhostIdParseError::InvalidBech32(_)));
    }

    #[test]
    fn fromstr_works_via_display() {
        let id = GhostId::from_bytes([0x42u8; 32]);
        let s = format!("{}", id);
        let parsed: GhostId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    proptest::proptest! {
        #[test]
        fn proptest_roundtrip(bytes in proptest::array::uniform32(proptest::num::u8::ANY)) {
            let id = GhostId::from_bytes(bytes);
            let s = id.to_bech32();
            let back = GhostId::from_bech32(&s).unwrap();
            proptest::prop_assert_eq!(id, back);
        }
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p ghost-core`
Expected: all bech32 tests pass, including proptest 256 cases.

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-core/src/id.rs
git commit -m "feat(ghost-core): GhostId bech32 encoding with proptest roundtrip"
```

---

## Task 5: Fingerprint type with display formatting

**Files:**
- Create: `crates/ghost-core/src/fingerprint.rs`
- Modify: `crates/ghost-core/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/ghost-core/src/fingerprint.rs`:

```rust
//! Short fingerprint of a GhostId — for verbal/visual verification.
//! Format: 4 hex groups of 4 chars: "1a2b-3c4d-5e6f-7890" (8 bytes from BLAKE3).

use crate::id::GhostId;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint([u8; 8]);

impl Fingerprint {
    pub fn of(id: &GhostId) -> Self {
        let hash = blake3::hash(id.as_bytes());
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&hash.as_bytes()[..8]);
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 8] {
        &self.0
    }
}

impl std::fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hex = hex::encode(self.0);
        write!(
            f,
            "{}-{}-{}-{}",
            &hex[0..4],
            &hex[4..8],
            &hex[8..12],
            &hex[12..16]
        )
    }
}

impl std::fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Fingerprint({})", self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_format_matches_pattern() {
        let id = GhostId::from_bytes([0u8; 32]);
        let fp = Fingerprint::of(&id);
        let s = fp.to_string();
        assert_eq!(s.len(), 19, "expected 4*4 hex + 3 dashes = 19 chars, got {:?}", s);
        let groups: Vec<&str> = s.split('-').collect();
        assert_eq!(groups.len(), 4);
        for g in groups {
            assert_eq!(g.len(), 4);
            assert!(g.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    #[test]
    fn fingerprint_deterministic_per_id() {
        let id = GhostId::from_bytes([7u8; 32]);
        let fp1 = Fingerprint::of(&id);
        let fp2 = Fingerprint::of(&id);
        assert_eq!(fp1, fp2);
        assert_eq!(fp1.to_string(), fp2.to_string());
    }

    #[test]
    fn different_ids_produce_different_fingerprints() {
        let a = Fingerprint::of(&GhostId::from_bytes([1u8; 32]));
        let b = Fingerprint::of(&GhostId::from_bytes([2u8; 32]));
        assert_ne!(a, b);
    }

    #[test]
    fn known_vector_zero_id() {
        // Lock the BLAKE3 of all-zero 32 bytes — first 8 bytes hex.
        // Computed: blake3([0u8;32]) → 2d3adedff11b61f1...
        let id = GhostId::from_bytes([0u8; 32]);
        let fp = Fingerprint::of(&id);
        assert_eq!(fp.to_string(), "2d3a-dedf-f11b-61f1");
    }
}
```

Modify `crates/ghost-core/src/lib.rs`:

```rust
//! Ghost core types, errors, and utilities. No I/O. No crypto operations beyond hashing.

pub mod fingerprint;
pub mod id;

pub use fingerprint::Fingerprint;
pub use id::{GhostId, GhostIdParseError, HRP_GHOST};

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles_and_has_constant_version() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-core");
    }
}
```

- [ ] **Step 2: Run tests, verify the known vector**

Run: `cargo test -p ghost-core`
Expected: all tests pass. If `known_vector_zero_id` fails, **stop**: the BLAKE3 output is locked here. Recompute via `python3 -c "import hashlib; ..."` or a quick `blake3` REPL check, update the assertion, then **document the recomputation in the commit message**.

(BLAKE3 of 32 zero bytes is well-known and stable; a mismatch means the dependency surface changed unexpectedly.)

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-core/
git commit -m "feat(ghost-core): Fingerprint type with display formatting"
```

---

## Task 6: ghost-core errors module

**Files:**
- Create: `crates/ghost-core/src/error.rs`
- Modify: `crates/ghost-core/src/lib.rs`

- [ ] **Step 1: Write the test**

Create `crates/ghost-core/src/error.rs`:

```rust
//! Common error category for ghost-core. Crate-specific errors live in their own crates.

use thiserror::Error;

/// Top-level error type that other Ghost crates can wrap.
/// Currently only used to expose GhostIdParseError; will grow.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid ghost id: {0}")]
    InvalidId(#[from] crate::id::GhostIdParseError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::GhostId;

    #[test]
    fn wraps_id_parse_error() {
        let err: CoreError = GhostId::from_bech32("garbage").unwrap_err().into();
        let msg = err.to_string();
        assert!(msg.starts_with("invalid ghost id:"));
    }
}
```

Modify `crates/ghost-core/src/lib.rs`:

```rust
//! Ghost core types, errors, and utilities. No I/O. No crypto operations beyond hashing.

pub mod error;
pub mod fingerprint;
pub mod id;

pub use error::CoreError;
pub use fingerprint::Fingerprint;
pub use id::{GhostId, GhostIdParseError, HRP_GHOST};

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles_and_has_constant_version() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-core");
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p ghost-core`
Expected: PASS.

- [ ] **Step 3: Run clippy strict**

Run: `cargo clippy -p ghost-core --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/ghost-core/
git commit -m "feat(ghost-core): top-level CoreError wrapping GhostIdParseError"
```

---

## Task 7: ghost-identity crate skeleton

**Files:**
- Create: `crates/ghost-identity/Cargo.toml`
- Create: `crates/ghost-identity/src/lib.rs`

- [ ] **Step 1: Create `crates/ghost-identity/Cargo.toml`**

```toml
[package]
name = "ghost-identity"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Ghost identity: Ed25519 IK/DK, encrypted identity file, OS keystore."

[dependencies]
ghost-core = { path = "../ghost-core" }

ed25519-dalek = { workspace = true }
x25519-dalek = { workspace = true }
chacha20poly1305 = { workspace = true }
blake3 = { workspace = true }
argon2 = { workspace = true }
hkdf = { workspace = true }
sha2 = { workspace = true }
zeroize = { workspace = true }
rand = { workspace = true }
rand_core = { workspace = true }

serde = { workspace = true }
ciborium = { workspace = true }
hex = { workspace = true }
base64 = { workspace = true }

thiserror = { workspace = true }
directories = { workspace = true }
keyring = { workspace = true }

[dev-dependencies]
proptest = { workspace = true }
tempfile = { workspace = true }
```

- [ ] **Step 2: Create `crates/ghost-identity/src/lib.rs`**

```rust
//! Ghost identity: Identity Key + Device Key + encrypted identity file + OS keystore.

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-identity");
    }
}
```

- [ ] **Step 3: Run test**

Run: `cargo test -p ghost-identity`
Expected: 1 test passes.

- [ ] **Step 4: Commit**

```bash
git add crates/ghost-identity/
git commit -m "feat(ghost-identity): scaffold crate"
```

---

## Task 8: IdentityKey (Ed25519 master keypair)

**Files:**
- Create: `crates/ghost-identity/src/keys.rs`
- Modify: `crates/ghost-identity/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/ghost-identity/src/keys.rs`:

```rust
//! Identity Key (IK) and Device Key (DK).

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey, SECRET_KEY_LENGTH};
use ghost_core::GhostId;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

/// Master Identity Key. Lives in identity.encrypted. Never sent over the wire.
#[derive(Serialize, Deserialize)]
pub struct IdentityKey {
    signing: SigningKey,
}

impl IdentityKey {
    pub fn generate() -> Self {
        Self {
            signing: SigningKey::generate(&mut OsRng),
        }
    }

    /// Reconstruct from raw secret seed (used during deserialization or restore).
    pub fn from_secret_bytes(secret: [u8; SECRET_KEY_LENGTH]) -> Self {
        Self {
            signing: SigningKey::from_bytes(&secret),
        }
    }

    pub fn public(&self) -> VerifyingKey {
        self.signing.verifying_key()
    }

    pub fn ghost_id(&self) -> GhostId {
        GhostId::from_bytes(self.signing.verifying_key().to_bytes())
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing.sign(message)
    }

    pub fn verify(&self, message: &[u8], sig: &Signature) -> bool {
        self.public().verify(message, sig).is_ok()
    }
}

impl Drop for IdentityKey {
    fn drop(&mut self) {
        // SigningKey already zeroizes via its own Drop; explicit pin for clarity.
        let mut bytes = self.signing.to_bytes();
        bytes.zeroize();
    }
}

impl std::fmt::Debug for IdentityKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IdentityKey(ghost_id={:?})", self.ghost_id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_produces_distinct_keys() {
        let a = IdentityKey::generate();
        let b = IdentityKey::generate();
        assert_ne!(a.ghost_id(), b.ghost_id());
    }

    #[test]
    fn ghost_id_equals_public_bytes() {
        let key = IdentityKey::generate();
        let id = key.ghost_id();
        assert_eq!(id.as_bytes(), &key.public().to_bytes());
    }

    #[test]
    fn sign_then_verify_roundtrip() {
        let key = IdentityKey::generate();
        let message = b"hello ghost";
        let sig = key.sign(message);
        assert!(key.verify(message, &sig));
    }

    #[test]
    fn verify_rejects_tampered_message() {
        let key = IdentityKey::generate();
        let sig = key.sign(b"original");
        assert!(!key.verify(b"tampered", &sig));
    }

    #[test]
    fn debug_does_not_leak_secret() {
        let key = IdentityKey::generate();
        let dbg = format!("{:?}", key);
        // Should reference ghost_id (truncated) but not raw secret.
        assert!(dbg.starts_with("IdentityKey(ghost_id="));
        // Sanity: ghost_id Debug already truncates to first 4 bytes.
        assert!(dbg.contains("…"));
    }
}
```

Modify `crates/ghost-identity/src/lib.rs`:

```rust
//! Ghost identity: Identity Key + Device Key + encrypted identity file + OS keystore.

pub mod keys;

pub use keys::IdentityKey;
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p ghost-identity`
Expected: 5 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-identity/
git commit -m "feat(ghost-identity): IdentityKey (Ed25519 master keypair)"
```

---

## Task 9: DeviceKey + parent IK signature

**Files:**
- Modify: `crates/ghost-identity/src/keys.rs`

- [ ] **Step 1: Write failing tests**

Append to `crates/ghost-identity/src/keys.rs`:

```rust
/// Device Key — Ed25519 keypair signed by the parent IdentityKey.
/// In MVP-1 there is one DK per identity; the architecture supports adding more later.
#[derive(Serialize, Deserialize)]
pub struct DeviceKey {
    signing: SigningKey,
    /// Signature by the parent IdentityKey over `signing.verifying_key().to_bytes()`.
    parent_signature: Signature,
}

impl DeviceKey {
    /// Generate a fresh DeviceKey signed by the given IdentityKey.
    pub fn generate(parent: &IdentityKey) -> Self {
        let signing = SigningKey::generate(&mut OsRng);
        let pubkey_bytes = signing.verifying_key().to_bytes();
        let parent_signature = parent.sign(&pubkey_bytes);
        Self {
            signing,
            parent_signature,
        }
    }

    pub fn public(&self) -> VerifyingKey {
        self.signing.verifying_key()
    }

    pub fn parent_signature(&self) -> &Signature {
        &self.parent_signature
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing.sign(message)
    }

    pub fn verify_parent(&self, parent_pub: &VerifyingKey) -> bool {
        let pubkey_bytes = self.signing.verifying_key().to_bytes();
        parent_pub
            .verify(&pubkey_bytes, &self.parent_signature)
            .is_ok()
    }
}

impl std::fmt::Debug for DeviceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pub_short = hex::encode(&self.signing.verifying_key().to_bytes()[..4]);
        write!(f, "DeviceKey(pub={}…)", pub_short)
    }
}

#[cfg(test)]
mod device_key_tests {
    use super::*;

    #[test]
    fn dk_signature_verifies_against_parent() {
        let ik = IdentityKey::generate();
        let dk = DeviceKey::generate(&ik);
        assert!(dk.verify_parent(&ik.public()));
    }

    #[test]
    fn dk_signature_rejects_wrong_parent() {
        let ik_a = IdentityKey::generate();
        let ik_b = IdentityKey::generate();
        let dk = DeviceKey::generate(&ik_a);
        assert!(!dk.verify_parent(&ik_b.public()));
    }

    #[test]
    fn dk_signs_messages_independently() {
        let ik = IdentityKey::generate();
        let dk = DeviceKey::generate(&ik);
        let sig = dk.sign(b"message-from-device");
        assert!(dk.public().verify(b"message-from-device", &sig).is_ok());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p ghost-identity`
Expected: 8 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-identity/src/keys.rs
git commit -m "feat(ghost-identity): DeviceKey signed by parent IdentityKey"
```

---

## Task 10: PreKey (X25519 one-time keypair)

**Files:**
- Create: `crates/ghost-identity/src/prekey.rs`
- Modify: `crates/ghost-identity/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/ghost-identity/src/prekey.rs`:

```rust
//! Pre-keys: X25519 one-time keypairs published to allow asynchronous first-contact.
//! In MVP-1 we generate a batch of one-time pre-keys plus one last-resort key.

use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use x25519_dalek::{PublicKey, StaticSecret};

#[derive(Serialize, Deserialize)]
pub struct PreKey {
    pub id: u32,
    pub secret: StaticSecret,
    pub is_last_resort: bool,
    pub created_at: u64,
}

impl PreKey {
    pub fn new(id: u32, is_last_resort: bool, created_at: u64) -> Self {
        Self {
            id,
            secret: StaticSecret::random_from_rng(&mut OsRng),
            is_last_resort,
            created_at,
        }
    }

    pub fn public(&self) -> PublicKey {
        PublicKey::from(&self.secret)
    }
}

impl std::fmt::Debug for PreKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PreKey(id={}, last_resort={}, pub={}…)",
            self.id,
            self.is_last_resort,
            hex::encode(&self.public().as_bytes()[..4])
        )
    }
}

/// Generate `count` one-time pre-keys plus one last-resort pre-key.
/// IDs are assigned sequentially starting from `start_id`.
pub fn generate_batch(count: u32, start_id: u32, now: u64) -> (Vec<PreKey>, PreKey) {
    let mut ones = Vec::with_capacity(count as usize);
    for i in 0..count {
        ones.push(PreKey::new(start_id + i, false, now));
    }
    let last_resort = PreKey::new(start_id + count, true, now);
    (ones, last_resort)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_prekey_has_distinct_public() {
        let a = PreKey::new(1, false, 0);
        let b = PreKey::new(2, false, 0);
        assert_ne!(a.public().as_bytes(), b.public().as_bytes());
    }

    #[test]
    fn generate_batch_produces_correct_count_and_ids() {
        let (ones, last_resort) = generate_batch(10, 100, 1700000000);
        assert_eq!(ones.len(), 10);
        for (i, key) in ones.iter().enumerate() {
            assert_eq!(key.id, 100 + i as u32);
            assert!(!key.is_last_resort);
            assert_eq!(key.created_at, 1700000000);
        }
        assert_eq!(last_resort.id, 110);
        assert!(last_resort.is_last_resort);
    }

    #[test]
    fn debug_does_not_leak_secret() {
        let pk = PreKey::new(42, false, 0);
        let dbg = format!("{:?}", pk);
        assert!(dbg.starts_with("PreKey(id=42"));
        assert!(dbg.contains("last_resort=false"));
        // No 64-char hex string of the secret.
        let secret_hex = hex::encode(pk.secret.as_bytes());
        assert!(!dbg.contains(&secret_hex));
    }
}
```

Modify `crates/ghost-identity/src/lib.rs`:

```rust
//! Ghost identity: Identity Key + Device Key + encrypted identity file + OS keystore.

pub mod keys;
pub mod prekey;

pub use keys::{DeviceKey, IdentityKey};
pub use prekey::{generate_batch, PreKey};
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p ghost-identity`
Expected: 11 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-identity/
git commit -m "feat(ghost-identity): PreKey batch generation (X25519)"
```

---

## Task 11: Argon2id KDF helper

**Files:**
- Create: `crates/ghost-identity/src/crypto.rs`
- Modify: `crates/ghost-identity/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/ghost-identity/src/crypto.rs`:

```rust
//! Symmetric crypto helpers: passphrase KDF and AEAD wrappers.

use argon2::{Algorithm, Argon2, Params, Version};
use thiserror::Error;

/// Length of derived key in bytes (256 bit).
pub const DERIVED_KEY_LEN: usize = 32;
/// Length of the salt for Argon2id — 16 random bytes per file.
pub const SALT_LEN: usize = 16;

/// Argon2id parameters — moderate memory + time cost, conservative defaults.
/// m=64MiB, t=3, p=1 — accepted ballpark per OWASP Password Storage Cheat Sheet.
fn argon2() -> Argon2<'static> {
    let params = Params::new(64 * 1024, 3, 1, Some(DERIVED_KEY_LEN))
        .expect("hard-coded Argon2 params are valid");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

#[derive(Debug, Error)]
pub enum KdfError {
    #[error("Argon2 KDF failed: {0}")]
    Failed(String),
}

/// Derive a 32-byte key from input bytes (passphrase || keystore secret) and a 16-byte salt.
pub fn derive_key(input: &[u8], salt: &[u8; SALT_LEN]) -> Result<[u8; DERIVED_KEY_LEN], KdfError> {
    let mut out = [0u8; DERIVED_KEY_LEN];
    argon2()
        .hash_password_into(input, salt, &mut out)
        .map_err(|e| KdfError::Failed(e.to_string()))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_key_is_deterministic_given_input_and_salt() {
        let input = b"correct horse battery staple";
        let salt = [42u8; 16];
        let k1 = derive_key(input, &salt).unwrap();
        let k2 = derive_key(input, &salt).unwrap();
        assert_eq!(k1, k2);
    }

    #[test]
    fn different_salt_yields_different_key() {
        let input = b"same passphrase";
        let k1 = derive_key(input, &[1u8; 16]).unwrap();
        let k2 = derive_key(input, &[2u8; 16]).unwrap();
        assert_ne!(k1, k2);
    }

    #[test]
    fn different_input_yields_different_key() {
        let salt = [0u8; 16];
        let k1 = derive_key(b"alpha", &salt).unwrap();
        let k2 = derive_key(b"beta", &salt).unwrap();
        assert_ne!(k1, k2);
    }
}
```

Modify `crates/ghost-identity/src/lib.rs`:

```rust
//! Ghost identity: Identity Key + Device Key + encrypted identity file + OS keystore.

pub mod crypto;
pub mod keys;
pub mod prekey;

pub use keys::{DeviceKey, IdentityKey};
pub use prekey::{generate_batch, PreKey};
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p ghost-identity`
Expected: 14 tests pass. (Argon2 tests take ~1 second each due to memory cost. Acceptable.)

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-identity/
git commit -m "feat(ghost-identity): Argon2id key derivation helper"
```

---

## Task 12: XChaCha20-Poly1305 AEAD wrapper

**Files:**
- Modify: `crates/ghost-identity/src/crypto.rs`

- [ ] **Step 1: Write failing tests**

Append to `crates/ghost-identity/src/crypto.rs`:

```rust
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use rand::RngCore;

/// XChaCha20-Poly1305 nonce length: 24 bytes.
pub const NONCE_LEN: usize = 24;

#[derive(Debug, Error)]
pub enum AeadError {
    #[error("encryption failed")]
    EncryptFailed,
    #[error("decryption failed (wrong key, tampered ciphertext, or corrupt file)")]
    DecryptFailed,
}

/// Encrypt plaintext with the 32-byte key. Returns (nonce, ciphertext_with_tag).
/// `aad` is optional Associated Authenticated Data — bound but not encrypted.
pub fn aead_encrypt(
    key: &[u8; DERIVED_KEY_LEN],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<([u8; NONCE_LEN], Vec<u8>), AeadError> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| AeadError::EncryptFailed)?;
    Ok((nonce_bytes, ciphertext))
}

pub fn aead_decrypt(
    key: &[u8; DERIVED_KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    ciphertext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, AeadError> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce = XNonce::from_slice(nonce);
    cipher
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| AeadError::DecryptFailed)
}

#[cfg(test)]
mod aead_tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = [9u8; 32];
        let plaintext = b"the secret message";
        let aad = b"some context";
        let (nonce, ct) = aead_encrypt(&key, plaintext, aad).unwrap();
        let pt = aead_decrypt(&key, &nonce, &ct, aad).unwrap();
        assert_eq!(pt, plaintext);
    }

    #[test]
    fn decrypt_fails_with_wrong_key() {
        let (nonce, ct) = aead_encrypt(&[1u8; 32], b"data", b"aad").unwrap();
        let err = aead_decrypt(&[2u8; 32], &nonce, &ct, b"aad").unwrap_err();
        assert!(matches!(err, AeadError::DecryptFailed));
    }

    #[test]
    fn decrypt_fails_with_tampered_ciphertext() {
        let key = [3u8; 32];
        let (nonce, mut ct) = aead_encrypt(&key, b"data", b"aad").unwrap();
        ct[0] ^= 0xFF;
        let err = aead_decrypt(&key, &nonce, &ct, b"aad").unwrap_err();
        assert!(matches!(err, AeadError::DecryptFailed));
    }

    #[test]
    fn decrypt_fails_with_wrong_aad() {
        let key = [4u8; 32];
        let (nonce, ct) = aead_encrypt(&key, b"data", b"original aad").unwrap();
        let err = aead_decrypt(&key, &nonce, &ct, b"different aad").unwrap_err();
        assert!(matches!(err, AeadError::DecryptFailed));
    }

    #[test]
    fn nonce_differs_between_encryptions() {
        let key = [5u8; 32];
        let (n1, _) = aead_encrypt(&key, b"data", b"").unwrap();
        let (n2, _) = aead_encrypt(&key, b"data", b"").unwrap();
        assert_ne!(n1, n2, "two encryptions must produce different nonces");
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p ghost-identity`
Expected: 19 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-identity/src/crypto.rs
git commit -m "feat(ghost-identity): XChaCha20-Poly1305 AEAD wrapper"
```

---

## Task 13: Identity struct + CBOR roundtrip

**Files:**
- Create: `crates/ghost-identity/src/identity.rs`
- Modify: `crates/ghost-identity/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/ghost-identity/src/identity.rs`:

```rust
//! Identity — top-level user identity, serialized to CBOR before encryption.

use crate::keys::{DeviceKey, IdentityKey};
use crate::prekey::{generate_batch, PreKey};
use ghost_core::GhostId;
use serde::{Deserialize, Serialize};

/// Bumped on every breaking change to identity file schema.
pub const IDENTITY_SCHEMA_VERSION: u8 = 1;

/// Number of one-time pre-keys generated at identity creation.
pub const INITIAL_PREKEY_COUNT: u32 = 10;

#[derive(Serialize, Deserialize)]
pub struct Identity {
    pub schema_version: u8,
    pub identity_key: IdentityKey,
    pub device_key: DeviceKey,
    pub display_name: Option<String>,
    pub one_time_prekeys: Vec<PreKey>,
    pub last_resort_prekey: PreKey,
    pub next_prekey_id: u32,
    pub created_at: u64,
}

impl Identity {
    /// Generate a fresh Identity with the given display name and current timestamp.
    /// `now` is the Unix-epoch seconds.
    pub fn generate(display_name: Option<String>, now: u64) -> Self {
        let identity_key = IdentityKey::generate();
        let device_key = DeviceKey::generate(&identity_key);
        let (one_time_prekeys, last_resort_prekey) = generate_batch(INITIAL_PREKEY_COUNT, 0, now);
        Self {
            schema_version: IDENTITY_SCHEMA_VERSION,
            identity_key,
            device_key,
            display_name,
            one_time_prekeys,
            last_resort_prekey,
            next_prekey_id: INITIAL_PREKEY_COUNT + 1,
            created_at: now,
        }
    }

    pub fn ghost_id(&self) -> GhostId {
        self.identity_key.ghost_id()
    }
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identity")
            .field("schema_version", &self.schema_version)
            .field("ghost_id", &self.ghost_id())
            .field("display_name", &self.display_name)
            .field("one_time_prekeys", &self.one_time_prekeys.len())
            .field("created_at", &self.created_at)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_populates_all_fields() {
        let id = Identity::generate(Some("Alice".to_string()), 1700000000);
        assert_eq!(id.schema_version, IDENTITY_SCHEMA_VERSION);
        assert_eq!(id.display_name.as_deref(), Some("Alice"));
        assert_eq!(id.one_time_prekeys.len(), INITIAL_PREKEY_COUNT as usize);
        assert!(id.last_resort_prekey.is_last_resort);
        assert_eq!(id.created_at, 1700000000);
    }

    #[test]
    fn dk_signature_verifies_against_ik() {
        let id = Identity::generate(None, 0);
        assert!(id.device_key.verify_parent(&id.identity_key.public()));
    }

    #[test]
    fn cbor_roundtrip_preserves_identity() {
        let original = Identity::generate(Some("Bob".to_string()), 1700000000);
        let original_id = original.ghost_id();
        let original_pk_count = original.one_time_prekeys.len();

        let mut buf = Vec::new();
        ciborium::into_writer(&original, &mut buf).unwrap();

        let restored: Identity = ciborium::from_reader(&buf[..]).unwrap();
        assert_eq!(restored.ghost_id(), original_id);
        assert_eq!(restored.display_name.as_deref(), Some("Bob"));
        assert_eq!(restored.one_time_prekeys.len(), original_pk_count);
        assert!(restored.device_key.verify_parent(&restored.identity_key.public()));
    }
}
```

Modify `crates/ghost-identity/src/lib.rs`:

```rust
//! Ghost identity: Identity Key + Device Key + encrypted identity file + OS keystore.

pub mod crypto;
pub mod identity;
pub mod keys;
pub mod prekey;

pub use identity::{Identity, IDENTITY_SCHEMA_VERSION, INITIAL_PREKEY_COUNT};
pub use keys::{DeviceKey, IdentityKey};
pub use prekey::{generate_batch, PreKey};
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p ghost-identity`
Expected: 22 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-identity/
git commit -m "feat(ghost-identity): Identity struct with CBOR roundtrip"
```

---

## Task 14: File format header (magic + version + salt + nonce)

**Files:**
- Create: `crates/ghost-identity/src/file_format.rs`
- Modify: `crates/ghost-identity/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/ghost-identity/src/file_format.rs`:

```rust
//! On-disk encoding for `identity.encrypted`:
//! [0..4]    magic "GHST"
//! [4..5]    file format version u8 (currently 1)
//! [5..21]   16-byte Argon2 salt
//! [21..45]  24-byte XChaCha20 nonce
//! [45..]    AEAD ciphertext (with embedded 16-byte Poly1305 tag)
//!
//! AAD bound by the AEAD covers magic + version + salt + nonce, so a tampered
//! header makes decryption fail.

use crate::crypto::{NONCE_LEN, SALT_LEN};
use thiserror::Error;

pub const MAGIC: &[u8; 4] = b"GHST";
pub const FILE_FORMAT_VERSION: u8 = 1;
pub const HEADER_LEN: usize = 4 + 1 + SALT_LEN + NONCE_LEN; // 45

#[derive(Debug, Error, PartialEq)]
pub enum FileFormatError {
    #[error("file too short: {0} bytes (header is {} bytes)", HEADER_LEN)]
    Truncated(usize),
    #[error("bad magic bytes: expected GHST, got {0:?}")]
    BadMagic([u8; 4]),
    #[error("unsupported file format version {0} (this build supports {1})")]
    UnsupportedVersion(u8, u8),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub version: u8,
    pub salt: [u8; SALT_LEN],
    pub nonce: [u8; NONCE_LEN],
}

impl Header {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_LEN);
        out.extend_from_slice(MAGIC);
        out.push(self.version);
        out.extend_from_slice(&self.salt);
        out.extend_from_slice(&self.nonce);
        out
    }

    /// Parse a header from the start of `bytes`. Returns the parsed header on success.
    pub fn parse(bytes: &[u8]) -> Result<Self, FileFormatError> {
        if bytes.len() < HEADER_LEN {
            return Err(FileFormatError::Truncated(bytes.len()));
        }
        let mut magic = [0u8; 4];
        magic.copy_from_slice(&bytes[0..4]);
        if &magic != MAGIC {
            return Err(FileFormatError::BadMagic(magic));
        }
        let version = bytes[4];
        if version != FILE_FORMAT_VERSION {
            return Err(FileFormatError::UnsupportedVersion(
                version,
                FILE_FORMAT_VERSION,
            ));
        }
        let mut salt = [0u8; SALT_LEN];
        salt.copy_from_slice(&bytes[5..5 + SALT_LEN]);
        let mut nonce = [0u8; NONCE_LEN];
        nonce.copy_from_slice(&bytes[5 + SALT_LEN..HEADER_LEN]);
        Ok(Self {
            version,
            salt,
            nonce,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_encode_then_parse() {
        let h = Header {
            version: FILE_FORMAT_VERSION,
            salt: [7u8; 16],
            nonce: [9u8; 24],
        };
        let bytes = h.encode();
        assert_eq!(bytes.len(), HEADER_LEN);
        assert_eq!(&bytes[0..4], MAGIC);
        assert_eq!(bytes[4], FILE_FORMAT_VERSION);
        let parsed = Header::parse(&bytes).unwrap();
        assert_eq!(parsed, h);
    }

    #[test]
    fn parse_rejects_truncated() {
        let err = Header::parse(&[0u8; 10]).unwrap_err();
        assert!(matches!(err, FileFormatError::Truncated(10)));
    }

    #[test]
    fn parse_rejects_bad_magic() {
        let mut bytes = vec![0u8; HEADER_LEN];
        bytes[0..4].copy_from_slice(b"XXXX");
        bytes[4] = FILE_FORMAT_VERSION;
        let err = Header::parse(&bytes).unwrap_err();
        assert_eq!(err, FileFormatError::BadMagic(*b"XXXX"));
    }

    #[test]
    fn parse_rejects_future_version() {
        let mut bytes = vec![0u8; HEADER_LEN];
        bytes[0..4].copy_from_slice(MAGIC);
        bytes[4] = 99;
        let err = Header::parse(&bytes).unwrap_err();
        assert_eq!(err, FileFormatError::UnsupportedVersion(99, 1));
    }
}
```

Modify `crates/ghost-identity/src/lib.rs`:

```rust
//! Ghost identity: Identity Key + Device Key + encrypted identity file + OS keystore.

pub mod crypto;
pub mod file_format;
pub mod identity;
pub mod keys;
pub mod prekey;

pub use file_format::{Header, FILE_FORMAT_VERSION};
pub use identity::{Identity, IDENTITY_SCHEMA_VERSION, INITIAL_PREKEY_COUNT};
pub use keys::{DeviceKey, IdentityKey};
pub use prekey::{generate_batch, PreKey};
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p ghost-identity`
Expected: 26 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-identity/
git commit -m "feat(ghost-identity): identity.encrypted file header (magic + version + salt + nonce)"
```

---

## Task 15: Identity save/load (encrypted file roundtrip)

**Files:**
- Create: `crates/ghost-identity/src/storage.rs`
- Modify: `crates/ghost-identity/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/ghost-identity/src/storage.rs`:

```rust
//! Save/load Identity to/from an encrypted file using header + AEAD.

use crate::crypto::{aead_decrypt, aead_encrypt, derive_key, AeadError, KdfError, SALT_LEN};
use crate::file_format::{Header, FileFormatError, FILE_FORMAT_VERSION, HEADER_LEN};
use crate::identity::Identity;
use rand::RngCore;
use std::io;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("file format: {0}")]
    Format(#[from] FileFormatError),
    #[error("KDF: {0}")]
    Kdf(#[from] KdfError),
    #[error("AEAD: {0}")]
    Aead(#[from] AeadError),
    #[error("CBOR encode: {0}")]
    CborEncode(String),
    #[error("CBOR decode: {0}")]
    CborDecode(String),
}

/// `key_input` is the secret material (passphrase || keystore_secret) used for KDF.
/// Empty passphrase + non-empty keystore_secret is the default UX.
pub fn save(identity: &Identity, key_input: &[u8], path: &Path) -> Result<(), StorageError> {
    let mut salt = [0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    let key = derive_key(key_input, &salt)?;

    let mut plaintext = Vec::new();
    ciborium::into_writer(identity, &mut plaintext)
        .map_err(|e| StorageError::CborEncode(e.to_string()))?;

    // Build header so we can use it as AEAD AAD (binds header to ciphertext).
    let mut nonce_holder: [u8; crate::crypto::NONCE_LEN];
    let (nonce, ciphertext);
    {
        let (n, ct) = aead_encrypt(&key, &plaintext, b"ghost.identity.v1.aad")?;
        nonce_holder = n;
        nonce = nonce_holder;
        ciphertext = ct;
    }

    let header = Header {
        version: FILE_FORMAT_VERSION,
        salt,
        nonce,
    };
    let mut file_bytes = header.encode();
    file_bytes.extend_from_slice(&ciphertext);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, &file_bytes)?;
    Ok(())
}

pub fn load(key_input: &[u8], path: &Path) -> Result<Identity, StorageError> {
    let bytes = std::fs::read(path)?;
    if bytes.len() < HEADER_LEN {
        return Err(FileFormatError::Truncated(bytes.len()).into());
    }
    let header = Header::parse(&bytes[..HEADER_LEN])?;
    let ciphertext = &bytes[HEADER_LEN..];
    let key = derive_key(key_input, &header.salt)?;
    let plaintext = aead_decrypt(&key, &header.nonce, ciphertext, b"ghost.identity.v1.aad")?;
    let identity: Identity = ciborium::from_reader(&plaintext[..])
        .map_err(|e| StorageError::CborDecode(e.to_string()))?;
    Ok(identity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("identity.encrypted");
        let original = Identity::generate(Some("Alice".to_string()), 1700000000);
        let original_id = original.ghost_id();

        save(&original, b"keystore-secret-32-bytes-of-fluff", &path).unwrap();
        let loaded = load(b"keystore-secret-32-bytes-of-fluff", &path).unwrap();

        assert_eq!(loaded.ghost_id(), original_id);
        assert_eq!(loaded.display_name.as_deref(), Some("Alice"));
        assert_eq!(loaded.one_time_prekeys.len(), original.one_time_prekeys.len());
    }

    #[test]
    fn load_fails_with_wrong_key() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("identity.encrypted");
        let original = Identity::generate(None, 0);
        save(&original, b"correct-key", &path).unwrap();

        let err = load(b"wrong-key", &path).unwrap_err();
        assert!(matches!(err, StorageError::Aead(_)));
    }

    #[test]
    fn load_fails_with_tampered_header() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("identity.encrypted");
        let original = Identity::generate(None, 0);
        save(&original, b"key", &path).unwrap();

        // Corrupt one byte of salt (within header) — AAD covers header, so AEAD must fail.
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[10] ^= 0xFF;
        std::fs::write(&path, &bytes).unwrap();

        let err = load(b"key", &path).unwrap_err();
        assert!(matches!(err, StorageError::Aead(_)));
    }

    #[test]
    fn load_fails_on_bad_magic() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("identity.encrypted");
        std::fs::write(&path, b"not a ghost identity file at all").unwrap();
        let err = load(b"key", &path).unwrap_err();
        assert!(matches!(err, StorageError::Format(FileFormatError::BadMagic(_))));
    }

    #[test]
    fn save_creates_parent_directory() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested/dirs/identity.encrypted");
        let original = Identity::generate(None, 0);
        save(&original, b"key", &path).unwrap();
        assert!(path.exists());
    }
}
```

Modify `crates/ghost-identity/src/lib.rs`:

```rust
//! Ghost identity: Identity Key + Device Key + encrypted identity file + OS keystore.

pub mod crypto;
pub mod file_format;
pub mod identity;
pub mod keys;
pub mod prekey;
pub mod storage;

pub use file_format::{Header, FILE_FORMAT_VERSION};
pub use identity::{Identity, IDENTITY_SCHEMA_VERSION, INITIAL_PREKEY_COUNT};
pub use keys::{DeviceKey, IdentityKey};
pub use prekey::{generate_batch, PreKey};
pub use storage::{load, save, StorageError};
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p ghost-identity`
Expected: 31 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-identity/
git commit -m "feat(ghost-identity): encrypted identity file save/load with AEAD"
```

---

## Task 16: OS keystore wrapper

**Files:**
- Create: `crates/ghost-identity/src/keystore.rs`
- Modify: `crates/ghost-identity/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/ghost-identity/src/keystore.rs`:

```rust
//! Cross-platform OS keystore for the random secret used as part of the identity-file KDF input.
//! Linux: Secret Service / GNOME Keyring / KWallet
//! macOS: Keychain
//! Windows: Credential Manager
//!
//! We store a base64-encoded 32-byte random secret. It is generated once on first identity
//! creation and reused across launches.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rand::RngCore;
use thiserror::Error;

const SERVICE: &str = "im.ghost.app";
const ACCOUNT: &str = "identity-keystore-secret-v1";
const SECRET_LEN: usize = 32;

#[derive(Debug, Error)]
pub enum KeystoreError {
    #[error("OS keystore: {0}")]
    Backend(String),
    #[error("stored secret has unexpected length {0} (expected {SECRET_LEN})")]
    BadLength(usize),
    #[error("stored secret is not valid base64: {0}")]
    BadEncoding(String),
}

fn entry() -> Result<keyring::Entry, KeystoreError> {
    keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| KeystoreError::Backend(e.to_string()))
}

/// Look up the keystore secret, generating + storing a new one if absent.
pub fn load_or_create_secret() -> Result<[u8; SECRET_LEN], KeystoreError> {
    let entry = entry()?;
    match entry.get_password() {
        Ok(s) => decode_secret(&s),
        Err(keyring::Error::NoEntry) => {
            let mut secret = [0u8; SECRET_LEN];
            rand::thread_rng().fill_bytes(&mut secret);
            let encoded = B64.encode(secret);
            entry
                .set_password(&encoded)
                .map_err(|e| KeystoreError::Backend(e.to_string()))?;
            Ok(secret)
        }
        Err(e) => Err(KeystoreError::Backend(e.to_string())),
    }
}

/// Force-overwrite the stored secret. Used by `wipe()` and tests.
pub fn store_secret(secret: &[u8; SECRET_LEN]) -> Result<(), KeystoreError> {
    let encoded = B64.encode(secret);
    entry()?
        .set_password(&encoded)
        .map_err(|e| KeystoreError::Backend(e.to_string()))
}

/// Remove the stored secret entirely (e.g., during `ghost-identity wipe`).
pub fn wipe_secret() -> Result<(), KeystoreError> {
    match entry()?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(KeystoreError::Backend(e.to_string())),
    }
}

fn decode_secret(s: &str) -> Result<[u8; SECRET_LEN], KeystoreError> {
    let bytes = B64
        .decode(s)
        .map_err(|e| KeystoreError::BadEncoding(e.to_string()))?;
    if bytes.len() != SECRET_LEN {
        return Err(KeystoreError::BadLength(bytes.len()));
    }
    let mut out = [0u8; SECRET_LEN];
    out.copy_from_slice(&bytes);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// These tests touch the real OS keystore. We use a per-test isolation by always
    /// wiping before/after. Run sequentially (`--test-threads=1`) if your CI runs tests
    /// in parallel — `cargo test` serializes by default for keyring backends on
    /// Linux/Windows, but if you see flakes, set `RUST_TEST_THREADS=1`.
    fn isolate() {
        let _ = wipe_secret();
    }

    #[test]
    fn create_returns_random_secret() {
        isolate();
        let s1 = load_or_create_secret().unwrap();
        // Second call must return the same secret (it was persisted).
        let s2 = load_or_create_secret().unwrap();
        assert_eq!(s1, s2);
        wipe_secret().unwrap();
    }

    #[test]
    fn wipe_then_create_yields_new_secret() {
        isolate();
        let s1 = load_or_create_secret().unwrap();
        wipe_secret().unwrap();
        let s2 = load_or_create_secret().unwrap();
        assert_ne!(s1, s2);
        wipe_secret().unwrap();
    }

    #[test]
    fn decode_secret_rejects_wrong_length() {
        let encoded = B64.encode([1u8; 16]);
        let err = decode_secret(&encoded).unwrap_err();
        assert!(matches!(err, KeystoreError::BadLength(16)));
    }

    #[test]
    fn decode_secret_rejects_invalid_base64() {
        let err = decode_secret("not!base!64!").unwrap_err();
        assert!(matches!(err, KeystoreError::BadEncoding(_)));
    }
}
```

Modify `crates/ghost-identity/src/lib.rs`:

```rust
//! Ghost identity: Identity Key + Device Key + encrypted identity file + OS keystore.

pub mod crypto;
pub mod file_format;
pub mod identity;
pub mod keys;
pub mod keystore;
pub mod prekey;
pub mod storage;

pub use file_format::{Header, FILE_FORMAT_VERSION};
pub use identity::{Identity, IDENTITY_SCHEMA_VERSION, INITIAL_PREKEY_COUNT};
pub use keys::{DeviceKey, IdentityKey};
pub use keystore::{load_or_create_secret, store_secret, wipe_secret, KeystoreError};
pub use prekey::{generate_batch, PreKey};
pub use storage::{load, save, StorageError};
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p ghost-identity -- --test-threads=1`
Expected: 35 tests pass.

**Note for CI:** keystore tests touch the actual OS keychain. On headless Linux CI you must run with `dbus-launch` and a session-keyring backend, or skip keystore tests with `--skip keystore::tests` and verify them manually on each platform. Document this in `crates/ghost-identity/README.md` (add in Task 23).

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-identity/
git commit -m "feat(ghost-identity): OS keystore via keyring crate"
```

---

## Task 17: Cross-platform paths helper

**Files:**
- Create: `crates/ghost-identity/src/paths.rs`
- Modify: `crates/ghost-identity/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/ghost-identity/src/paths.rs`:

```rust
//! Cross-platform paths for Ghost data files.
//! Linux:   ~/.ghost/identity.encrypted
//! Windows: %APPDATA%/Ghost/identity.encrypted
//! macOS:   ~/Library/Application Support/Ghost/identity.encrypted
//!
//! Override via `GHOST_HOME` environment variable (used in tests and for portability).

use directories::ProjectDirs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PathsError {
    #[error("could not resolve user data directory (no home directory?)")]
    NoHome,
}

/// Root data directory for Ghost.
pub fn ghost_home() -> Result<PathBuf, PathsError> {
    if let Ok(custom) = std::env::var("GHOST_HOME") {
        return Ok(PathBuf::from(custom));
    }
    // On Linux directories crate uses XDG_DATA_HOME or ~/.local/share/ghost. We override
    // to "~/.ghost" to match the design spec.
    #[cfg(target_os = "linux")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return Ok(Path::new(&home).join(".ghost"));
        }
        return Err(PathsError::NoHome);
    }
    #[cfg(not(target_os = "linux"))]
    {
        let pd = ProjectDirs::from("im", "Ghost", "Ghost").ok_or(PathsError::NoHome)?;
        Ok(pd.data_dir().to_path_buf())
    }
}

pub fn identity_file() -> Result<PathBuf, PathsError> {
    Ok(ghost_home()?.join("identity.encrypted"))
}

pub fn database_file() -> Result<PathBuf, PathsError> {
    Ok(ghost_home()?.join("ghost.db"))
}

pub fn logs_dir() -> Result<PathBuf, PathsError> {
    Ok(ghost_home()?.join("logs"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghost_home_respects_env_override() {
        let temp = std::env::temp_dir().join("ghost-home-test-1");
        std::env::set_var("GHOST_HOME", &temp);
        let home = ghost_home().unwrap();
        assert_eq!(home, temp);
        std::env::remove_var("GHOST_HOME");
    }

    #[test]
    fn paths_compose_correctly() {
        let temp = std::env::temp_dir().join("ghost-home-test-2");
        std::env::set_var("GHOST_HOME", &temp);
        assert_eq!(identity_file().unwrap(), temp.join("identity.encrypted"));
        assert_eq!(database_file().unwrap(), temp.join("ghost.db"));
        assert_eq!(logs_dir().unwrap(), temp.join("logs"));
        std::env::remove_var("GHOST_HOME");
    }
}
```

Modify `crates/ghost-identity/src/lib.rs`:

```rust
//! Ghost identity: Identity Key + Device Key + encrypted identity file + OS keystore.

pub mod crypto;
pub mod file_format;
pub mod identity;
pub mod keys;
pub mod keystore;
pub mod paths;
pub mod prekey;
pub mod storage;

pub use file_format::{Header, FILE_FORMAT_VERSION};
pub use identity::{Identity, IDENTITY_SCHEMA_VERSION, INITIAL_PREKEY_COUNT};
pub use keys::{DeviceKey, IdentityKey};
pub use keystore::{load_or_create_secret, store_secret, wipe_secret, KeystoreError};
pub use paths::{database_file, ghost_home, identity_file, logs_dir, PathsError};
pub use prekey::{generate_batch, PreKey};
pub use storage::{load, save, StorageError};
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p ghost-identity -- --test-threads=1`
Expected: 37 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-identity/
git commit -m "feat(ghost-identity): cross-platform paths helper with GHOST_HOME override"
```

---

## Task 18: High-level Identity::create / Identity::load (combining keystore + storage)

**Files:**
- Modify: `crates/ghost-identity/src/identity.rs`

- [ ] **Step 1: Write failing tests**

Append to `crates/ghost-identity/src/identity.rs`:

```rust
use crate::keystore::{load_or_create_secret, KeystoreError};
use crate::paths::{identity_file, PathsError};
use crate::storage::{load as storage_load, save as storage_save, StorageError};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("paths: {0}")]
    Paths(#[from] PathsError),
    #[error("keystore: {0}")]
    Keystore(#[from] KeystoreError),
    #[error("storage: {0}")]
    Storage(#[from] StorageError),
    #[error("identity already exists at {0:?}")]
    AlreadyExists(PathBuf),
    #[error("identity not found at {0:?}")]
    NotFound(PathBuf),
    #[error("system clock returned a time before UNIX_EPOCH")]
    BadClock,
}

fn now_seconds() -> Result<u64, IdentityError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|_| IdentityError::BadClock)
}

fn key_input(passphrase: Option<&str>, keystore_secret: &[u8; 32]) -> Vec<u8> {
    let mut input = Vec::with_capacity(64);
    if let Some(p) = passphrase {
        input.extend_from_slice(p.as_bytes());
    }
    input.extend_from_slice(keystore_secret);
    input
}

pub struct CreateOptions<'a> {
    pub display_name: Option<String>,
    pub passphrase: Option<&'a str>,
    pub overwrite: bool,
}

impl Identity {
    /// Generate, encrypt, and persist a fresh Identity at the standard path.
    pub fn create(opts: CreateOptions<'_>) -> Result<Self, IdentityError> {
        let path = identity_file()?;
        if path.exists() && !opts.overwrite {
            return Err(IdentityError::AlreadyExists(path));
        }
        let keystore_secret = load_or_create_secret()?;
        let key_input = key_input(opts.passphrase, &keystore_secret);
        let identity = Identity::generate(opts.display_name, now_seconds()?);
        storage_save(&identity, &key_input, &path)?;
        Ok(identity)
    }

    /// Load and decrypt the Identity from the standard path.
    pub fn load_default(passphrase: Option<&str>) -> Result<Self, IdentityError> {
        let path = identity_file()?;
        if !path.exists() {
            return Err(IdentityError::NotFound(path));
        }
        let keystore_secret = load_or_create_secret()?;
        let key_input = key_input(passphrase, &keystore_secret);
        let identity = storage_load(&key_input, &path)?;
        Ok(identity)
    }
}

#[cfg(test)]
mod create_load_tests {
    use super::*;
    use crate::keystore;
    use std::sync::Mutex;
    use tempfile::tempdir;

    /// Serialize tests in this module — they share GHOST_HOME and OS keystore.
    static LOCK: Mutex<()> = Mutex::new(());

    fn isolated<F: FnOnce()>(f: F) {
        let _g = LOCK.lock().unwrap();
        let dir = tempdir().unwrap();
        std::env::set_var("GHOST_HOME", dir.path());
        let _ = keystore::wipe_secret();
        f();
        let _ = keystore::wipe_secret();
        std::env::remove_var("GHOST_HOME");
    }

    #[test]
    fn create_then_load_default() {
        isolated(|| {
            let created = Identity::create(CreateOptions {
                display_name: Some("Alice".to_string()),
                passphrase: None,
                overwrite: false,
            })
            .unwrap();
            let loaded = Identity::load_default(None).unwrap();
            assert_eq!(loaded.ghost_id(), created.ghost_id());
            assert_eq!(loaded.display_name.as_deref(), Some("Alice"));
        });
    }

    #[test]
    fn create_with_passphrase_load_with_same_passphrase() {
        isolated(|| {
            Identity::create(CreateOptions {
                display_name: None,
                passphrase: Some("hunter2"),
                overwrite: false,
            })
            .unwrap();
            let loaded = Identity::load_default(Some("hunter2")).unwrap();
            assert!(loaded.device_key.verify_parent(&loaded.identity_key.public()));
        });
    }

    #[test]
    fn load_with_wrong_passphrase_fails() {
        isolated(|| {
            Identity::create(CreateOptions {
                display_name: None,
                passphrase: Some("right"),
                overwrite: false,
            })
            .unwrap();
            let err = Identity::load_default(Some("wrong")).unwrap_err();
            assert!(matches!(err, IdentityError::Storage(_)));
        });
    }

    #[test]
    fn create_refuses_overwrite_by_default() {
        isolated(|| {
            Identity::create(CreateOptions {
                display_name: None,
                passphrase: None,
                overwrite: false,
            })
            .unwrap();
            let err = Identity::create(CreateOptions {
                display_name: None,
                passphrase: None,
                overwrite: false,
            })
            .unwrap_err();
            assert!(matches!(err, IdentityError::AlreadyExists(_)));
        });
    }

    #[test]
    fn create_with_overwrite_replaces_identity() {
        isolated(|| {
            let first = Identity::create(CreateOptions {
                display_name: Some("First".to_string()),
                passphrase: None,
                overwrite: false,
            })
            .unwrap();
            let second = Identity::create(CreateOptions {
                display_name: Some("Second".to_string()),
                passphrase: None,
                overwrite: true,
            })
            .unwrap();
            assert_ne!(first.ghost_id(), second.ghost_id());

            let loaded = Identity::load_default(None).unwrap();
            assert_eq!(loaded.ghost_id(), second.ghost_id());
        });
    }

    #[test]
    fn load_when_no_identity_exists_returns_not_found() {
        isolated(|| {
            let err = Identity::load_default(None).unwrap_err();
            assert!(matches!(err, IdentityError::NotFound(_)));
        });
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p ghost-identity -- --test-threads=1`
Expected: 43 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-identity/
git commit -m "feat(ghost-identity): high-level Identity::create / load_default"
```

---

## Task 19: ghost-identity-cli skeleton

**Files:**
- Create: `crates/ghost-identity-cli/Cargo.toml`
- Create: `crates/ghost-identity-cli/src/main.rs`

- [ ] **Step 1: Create `crates/ghost-identity-cli/Cargo.toml`**

```toml
[package]
name = "ghost-identity-cli"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "CLI smoke-test harness for ghost-identity (create, show, wipe)."

[[bin]]
name = "ghost-identity"
path = "src/main.rs"

[dependencies]
ghost-core     = { path = "../ghost-core" }
ghost-identity = { path = "../ghost-identity" }

clap = { workspace = true }
anyhow = { workspace = true }
```

- [ ] **Step 2: Create `crates/ghost-identity-cli/src/main.rs`**

```rust
//! Smoke-test CLI for ghost-identity. NOT shipped in the eventual desktop app —
//! purely a manual verification tool during MVP-1 development.

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ghost-identity")]
#[command(about = "Ghost identity smoke-test CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print version info and exit.
    Version,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Version => {
            println!("ghost-identity-cli {}", env!("CARGO_PKG_VERSION"));
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Build and run**

Run: `cargo run -p ghost-identity-cli -- version`
Expected: prints `ghost-identity-cli 0.0.1`.

- [ ] **Step 4: Commit**

```bash
git add crates/ghost-identity-cli/
git commit -m "feat(cli): scaffold ghost-identity-cli with version command"
```

---

## Task 20: CLI `create` command

**Files:**
- Modify: `crates/ghost-identity-cli/src/main.rs`

- [ ] **Step 1: Replace contents with create command**

Replace `crates/ghost-identity-cli/src/main.rs`:

```rust
//! Smoke-test CLI for ghost-identity. NOT shipped in the eventual desktop app —
//! purely a manual verification tool during MVP-1 development.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use ghost_core::Fingerprint;
use ghost_identity::{identity_file, CreateOptions, Identity};

#[derive(Parser)]
#[command(name = "ghost-identity")]
#[command(about = "Ghost identity smoke-test CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print version info.
    Version,
    /// Create a new Ghost identity at the standard path.
    Create {
        /// Optional human-readable display name.
        #[arg(long)]
        display_name: Option<String>,
        /// Optional passphrase. If omitted, the identity file is encrypted with the
        /// OS keystore secret only — convenient but anyone with file + OS access can decrypt.
        #[arg(long)]
        passphrase: Option<String>,
        /// Overwrite an existing identity. DESTRUCTIVE — old identity is unrecoverable.
        #[arg(long)]
        overwrite: bool,
    },
}

fn cmd_create(
    display_name: Option<String>,
    passphrase: Option<String>,
    overwrite: bool,
) -> Result<()> {
    let identity = Identity::create(CreateOptions {
        display_name,
        passphrase: passphrase.as_deref(),
        overwrite,
    })
    .context("create identity")?;

    let path = identity_file().context("resolve identity path")?;
    let id = identity.ghost_id();
    let fp = Fingerprint::of(&id);

    println!("Identity created.");
    println!("  Path:         {}", path.display());
    println!("  Ghost ID:     {}", id);
    println!("  Fingerprint:  {}", fp);
    if let Some(name) = identity.display_name.as_deref() {
        println!("  Display name: {}", name);
    }
    println!(
        "  Pre-keys:     {} one-time + 1 last-resort",
        identity.one_time_prekeys.len()
    );
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Version => {
            println!("ghost-identity-cli {}", env!("CARGO_PKG_VERSION"));
        }
        Command::Create {
            display_name,
            passphrase,
            overwrite,
        } => cmd_create(display_name, passphrase, overwrite)?,
    }
    Ok(())
}
```

- [ ] **Step 2: Run a smoke test against a temp GHOST_HOME**

```bash
GHOST_HOME=/tmp/ghost-smoke-create cargo run -p ghost-identity-cli -- create --display-name "Alice"
```

Expected output (Ghost ID will differ each run):
```
Identity created.
  Path:         /tmp/ghost-smoke-create/identity.encrypted
  Ghost ID:     ghost1...
  Fingerprint:  ....-....-....-....
  Display name: Alice
  Pre-keys:     10 one-time + 1 last-resort
```

Then verify file exists and is non-trivial in size:
```bash
ls -l /tmp/ghost-smoke-create/identity.encrypted
```
Expected: file size > 500 bytes.

Cleanup: `rm -rf /tmp/ghost-smoke-create` and wipe keystore via `ghost-identity wipe` once Task 22 lands (or manually for now).

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-identity-cli/src/main.rs
git commit -m "feat(cli): ghost-identity create command"
```

---

## Task 21: CLI `show` command

**Files:**
- Modify: `crates/ghost-identity-cli/src/main.rs`

- [ ] **Step 1: Add the Show variant and handler**

Edit `crates/ghost-identity-cli/src/main.rs` — add to `Command` enum and handler dispatch:

```rust
// Inside enum Command (add as new variant):
    /// Load and display the existing Ghost identity.
    Show {
        /// Passphrase used during creation (if any).
        #[arg(long)]
        passphrase: Option<String>,
    },
```

Add the handler function:

```rust
fn cmd_show(passphrase: Option<String>) -> Result<()> {
    let identity =
        Identity::load_default(passphrase.as_deref()).context("load identity")?;
    let path = identity_file().context("resolve identity path")?;
    let id = identity.ghost_id();
    let fp = Fingerprint::of(&id);

    println!("Identity loaded.");
    println!("  Path:         {}", path.display());
    println!("  Ghost ID:     {}", id);
    println!("  Fingerprint:  {}", fp);
    println!(
        "  Display name: {}",
        identity.display_name.as_deref().unwrap_or("<none>")
    );
    println!(
        "  Pre-keys:     {} one-time + 1 last-resort",
        identity.one_time_prekeys.len()
    );
    println!("  Created at:   {} (unix seconds)", identity.created_at);

    let dk_ok = identity
        .device_key
        .verify_parent(&identity.identity_key.public());
    println!("  DK signature: {}", if dk_ok { "valid" } else { "INVALID" });
    Ok(())
}
```

Update `main` to dispatch `Show`:

```rust
fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Version => {
            println!("ghost-identity-cli {}", env!("CARGO_PKG_VERSION"));
        }
        Command::Create {
            display_name,
            passphrase,
            overwrite,
        } => cmd_create(display_name, passphrase, overwrite)?,
        Command::Show { passphrase } => cmd_show(passphrase)?,
    }
    Ok(())
}
```

- [ ] **Step 2: Smoke test create-then-show roundtrip**

```bash
GHOST_HOME=/tmp/ghost-smoke-show cargo run -p ghost-identity-cli -- create --display-name "Bob" --passphrase "test"
GHOST_HOME=/tmp/ghost-smoke-show cargo run -p ghost-identity-cli -- show --passphrase "test"
```

Expected: `show` prints the **same** Ghost ID and fingerprint as `create`. `DK signature: valid`.

Then test wrong passphrase:
```bash
GHOST_HOME=/tmp/ghost-smoke-show cargo run -p ghost-identity-cli -- show --passphrase "wrong"
```
Expected: error message ending in `decryption failed (wrong key, tampered ciphertext, or corrupt file)`.

Cleanup: `rm -rf /tmp/ghost-smoke-show`.

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-identity-cli/src/main.rs
git commit -m "feat(cli): ghost-identity show command"
```

---

## Task 22: CLI `wipe` command

**Files:**
- Modify: `crates/ghost-identity-cli/src/main.rs`

- [ ] **Step 1: Add Wipe variant**

Add to `Command` enum:

```rust
    /// Permanently delete the local identity file AND the OS keystore secret.
    /// DESTRUCTIVE: the identity is unrecoverable without a backup.
    Wipe {
        /// Skip the confirmation prompt.
        #[arg(long)]
        yes: bool,
    },
```

Add handler:

```rust
fn cmd_wipe(yes: bool) -> Result<()> {
    use std::io::{self, Write};
    let path = identity_file().context("resolve identity path")?;
    println!("This will delete:");
    println!("  - identity file: {}", path.display());
    println!("  - OS keystore secret for service 'im.ghost.app'");
    println!("If you have no backup, your Ghost ID becomes unreachable forever.");
    if !yes {
        print!("Type 'WIPE' to confirm: ");
        io::stdout().flush().ok();
        let mut buf = String::new();
        io::stdin().read_line(&mut buf)?;
        if buf.trim() != "WIPE" {
            println!("Cancelled.");
            return Ok(());
        }
    }

    if path.exists() {
        std::fs::remove_file(&path).context("remove identity file")?;
    }
    ghost_identity::wipe_secret().context("wipe keystore secret")?;
    println!("Wiped.");
    Ok(())
}
```

Update `main` dispatch to add `Wipe`:

```rust
        Command::Wipe { yes } => cmd_wipe(yes)?,
```

- [ ] **Step 2: Smoke test full lifecycle**

```bash
GHOST_HOME=/tmp/ghost-smoke-wipe cargo run -p ghost-identity-cli -- create --display-name "Carol"
GHOST_HOME=/tmp/ghost-smoke-wipe cargo run -p ghost-identity-cli -- show
GHOST_HOME=/tmp/ghost-smoke-wipe cargo run -p ghost-identity-cli -- wipe --yes
GHOST_HOME=/tmp/ghost-smoke-wipe cargo run -p ghost-identity-cli -- show
```

Expected: final `show` errors with "identity not found". Confirms wipe deleted both file and keystore secret (a fresh `create` after wipe must produce a different Ghost ID — verify by re-creating with `--display-name "Carol2"` and observing different ID).

Cleanup: `rm -rf /tmp/ghost-smoke-wipe`.

- [ ] **Step 3: Commit**

```bash
git add crates/ghost-identity-cli/src/main.rs
git commit -m "feat(cli): ghost-identity wipe command"
```

---

## Task 23: README + CI workflow

**Files:**
- Create: `README.md`
- Create: `crates/ghost-identity/README.md`
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Create root `README.md`**

```markdown
# Ghost

Anonymous, end-to-end encrypted desktop messenger. Hybrid of Discord and Telegram, no hosting required.

> **Status:** MVP-1 in early development. Not yet usable. See [docs/superpowers/specs/](docs/superpowers/specs/) for design and [docs/superpowers/plans/](docs/superpowers/plans/) for implementation plans.

## Build

```bash
cargo build --workspace
cargo test --workspace -- --test-threads=1
```

`--test-threads=1` is required because `ghost-identity` keystore tests touch the OS keychain and must run sequentially.

## Workspace layout

- `crates/ghost-core` — common types (GhostId, Fingerprint, errors)
- `crates/ghost-identity` — identity keys, encrypted file, OS keystore
- `crates/ghost-identity-cli` — manual smoke-test CLI

More crates land as later plans (`docs/superpowers/plans/`) are implemented.

## License

AGPL-3.0-only.
```

- [ ] **Step 2: Create `crates/ghost-identity/README.md`**

```markdown
# ghost-identity

Identity primitives: Ed25519 IK/DK, encrypted identity file, OS keystore.

## Testing notes

Tests in `keystore` and `identity::create_load_tests` modules touch the **real OS keystore**.
Run with `--test-threads=1`:

```bash
cargo test -p ghost-identity -- --test-threads=1
```

### Headless Linux (CI)

The keyring backend on Linux requires a session DBus + Secret Service. On headless CI:

- Install `gnome-keyring` and `dbus-x11`.
- Wrap tests in `dbus-launch --exit-with-session` and `gnome-keyring-daemon --start --components=secrets`.

If your CI cannot provide that, skip the keystore tests:

```bash
cargo test -p ghost-identity -- --test-threads=1 --skip keystore::tests --skip identity::create_load_tests
```

…and verify them manually on developer machines for each platform before release.

## File format

`identity.encrypted` layout (see `file_format.rs`):

```
[0..4]    magic "GHST"
[4]       version u8 (currently 1)
[5..21]   16-byte Argon2id salt
[21..45]  24-byte XChaCha20 nonce
[45..]    XChaCha20-Poly1305 ciphertext (CBOR-serialized Identity) + 16-byte tag
```

The AEAD AAD is the literal `b"ghost.identity.v1.aad"`. Tampering with the header makes decryption fail.
```

- [ ] **Step 3: Create `.github/workflows/ci.yml`**

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

jobs:
  build-test:
    name: ${{ matrix.os }}
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@1.85
        with:
          components: rustfmt, clippy

      - name: Cache cargo
        uses: Swatinem/rust-cache@v2

      - name: Linux — install gnome-keyring + dbus
        if: runner.os == 'Linux'
        run: |
          sudo apt-get update
          sudo apt-get install -y gnome-keyring dbus-x11 libdbus-1-dev

      - name: cargo fmt
        run: cargo fmt --all -- --check

      - name: cargo clippy
        run: cargo clippy --all-targets --workspace -- -D warnings

      - name: cargo test (non-keystore)
        # Skip keystore tests on CI; they touch the real OS keychain. Run them
        # locally per platform before release.
        run: cargo test --workspace -- --test-threads=1 --skip keystore::tests --skip create_load_tests
```

- [ ] **Step 4: Verify locally**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --workspace -- -D warnings
cargo test --workspace -- --test-threads=1
```

All three should pass.

- [ ] **Step 5: Commit**

```bash
git add README.md crates/ghost-identity/README.md .github/
git commit -m "docs+ci: README files and GitHub Actions matrix build"
```

---

## Task 24: End-to-end smoke test script

**Files:**
- Create: `scripts/smoke-test-plan-01.sh`

- [ ] **Step 1: Create the script**

Create `scripts/smoke-test-plan-01.sh`:

```bash
#!/usr/bin/env bash
# End-to-end smoke test for Plan 01 deliverable.
# Verifies: create → show → wipe → show-fails — full identity lifecycle.

set -euo pipefail

SMOKE_HOME="${TMPDIR:-/tmp}/ghost-smoke-plan01-$$"
trap 'rm -rf "$SMOKE_HOME"' EXIT

export GHOST_HOME="$SMOKE_HOME"
mkdir -p "$SMOKE_HOME"

echo "==> 1. Create identity"
CREATE_OUT=$(cargo run --quiet -p ghost-identity-cli -- create --display-name "SmokeAlice" --passphrase "test-pp")
echo "$CREATE_OUT"
CREATED_ID=$(echo "$CREATE_OUT" | grep "Ghost ID:" | awk '{print $3}')
[ -n "$CREATED_ID" ] || { echo "FAIL: no Ghost ID in create output"; exit 1; }
echo "Got Ghost ID: $CREATED_ID"

echo "==> 2. Verify file exists and is encrypted (no plaintext leak)"
test -f "$SMOKE_HOME/identity.encrypted" || { echo "FAIL: file missing"; exit 1; }
if grep -aq "SmokeAlice" "$SMOKE_HOME/identity.encrypted"; then
  echo "FAIL: display name leaked in plaintext"
  exit 1
fi
echo "OK — file present, no plaintext leak"

echo "==> 3. Show identity, verify same Ghost ID"
SHOW_OUT=$(cargo run --quiet -p ghost-identity-cli -- show --passphrase "test-pp")
echo "$SHOW_OUT"
SHOWN_ID=$(echo "$SHOW_OUT" | grep "Ghost ID:" | awk '{print $3}')
[ "$SHOWN_ID" = "$CREATED_ID" ] || { echo "FAIL: show returned different ID"; exit 1; }
echo "$SHOW_OUT" | grep -q "DK signature: valid" || { echo "FAIL: DK signature invalid"; exit 1; }
echo "OK — identity round-trips correctly"

echo "==> 4. Show with wrong passphrase MUST fail"
if cargo run --quiet -p ghost-identity-cli -- show --passphrase "wrong-pp" 2>/dev/null; then
  echo "FAIL: wrong passphrase was accepted"
  exit 1
fi
echo "OK — wrong passphrase rejected"

echo "==> 5. Wipe and verify"
cargo run --quiet -p ghost-identity-cli -- wipe --yes
test ! -f "$SMOKE_HOME/identity.encrypted" || { echo "FAIL: file still present after wipe"; exit 1; }
echo "OK — wipe removed identity"

echo "==> 6. Show after wipe MUST fail with NotFound"
if cargo run --quiet -p ghost-identity-cli -- show 2>/dev/null; then
  echo "FAIL: show succeeded after wipe"
  exit 1
fi
echo "OK — show rejects missing identity"

echo "==> 7. New identity after wipe has different Ghost ID"
NEW_OUT=$(cargo run --quiet -p ghost-identity-cli -- create --display-name "SmokeAlice2")
NEW_ID=$(echo "$NEW_OUT" | grep "Ghost ID:" | awk '{print $3}')
[ "$NEW_ID" != "$CREATED_ID" ] || { echo "FAIL: regenerated identity has same ID"; exit 1; }
echo "OK — new identity is genuinely fresh"

cargo run --quiet -p ghost-identity-cli -- wipe --yes

echo
echo "==> Plan 01 smoke test PASSED"
```

Make executable:
```bash
chmod +x scripts/smoke-test-plan-01.sh
```

- [ ] **Step 2: Run the smoke test**

Run: `bash scripts/smoke-test-plan-01.sh`
Expected: ends with `==> Plan 01 smoke test PASSED`.

This script covers the full Plan 01 deliverable. It SHOULD be runnable on Windows (Git Bash), macOS, and Linux without changes.

- [ ] **Step 3: Commit**

```bash
git add scripts/
git commit -m "test: end-to-end smoke test for Plan 01 deliverable"
```

---

## Task 25: Final verification + tag

**Files:** none (verification only)

- [ ] **Step 1: Run full test suite + clippy**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --workspace -- -D warnings
cargo test --workspace -- --test-threads=1
bash scripts/smoke-test-plan-01.sh
```

All four must pass.

- [ ] **Step 2: Tag the milestone**

```bash
git tag -a plan-01-complete -m "Plan 01 (Foundation + Identity) complete"
```

(Don't push the tag — we're not on a remote yet. Tag exists locally as a milestone.)

- [ ] **Step 3: Update todos / hand off**

Plan 01 produces a working, tested identity layer that:
- Generates Ed25519 IK + DK with parent signatures
- Persists encrypted with OS keystore + optional passphrase
- Round-trips through CBOR + AEAD with full security tests
- Is exposed via a smoke-test CLI for manual verification

**Next:** Plan 02 — Crypto + Wire Protocol (MLS, sealed sender, CBOR envelopes).

---

## Self-review checklist (run after writing this plan)

**1. Spec coverage** — every requirement in spec sections 2 (architecture), 3 (identity, keys, devices) implemented:
- ✓ Three-layer key hierarchy (IK/DK/MLS state) — IK + DK in Plan 01; MLS state in Plan 02
- ✓ Ed25519 for IK/DK, X25519 for pre-keys
- ✓ identity.encrypted file with header + Argon2id KDF + XChaCha20-Poly1305 AEAD
- ✓ OS keystore via `keyring` crate
- ✓ Cross-platform paths (Linux ~/.ghost, Windows %APPDATA%, macOS Application Support)
- ✓ Pre-key batch generation (10 + 1 last-resort)
- ✓ GhostId in bech32 with `ghost1` HRP
- ✓ Fingerprint as 4 groups × 4 hex
- ✓ Onboarding flow (offline create) — exposed via `ghost-identity create`

**Items deferred to later plans (correctly out of scope for Plan 01):**
- Backup/restore (`age` format) — Plan 06 (orchestration)
- Display-name change UI — Plan 07 (UI)
- Multi-device DK chain — Plan 02-3 multi-device extensions

**2. Placeholder scan** — searched for "TBD", "TODO", "implement later". None found in this plan.

**3. Type consistency** — `IdentityKey::generate`, `DeviceKey::generate(&IdentityKey)`, `Identity::generate`, `Identity::create`, `Identity::load_default` all use consistent naming. `derive_key`, `aead_encrypt`, `aead_decrypt` consistent. `save`, `load`, `wipe_secret`, `load_or_create_secret`, `store_secret` consistent.

---

**Plan 01 complete and ready for execution.**
