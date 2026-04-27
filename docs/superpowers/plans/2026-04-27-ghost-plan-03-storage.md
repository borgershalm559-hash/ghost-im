# Ghost Plan 03 — Storage (SQLite + SQLCipher)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `ghost-storage` crate that provides a SQLCipher-encrypted SQLite database with repository APIs for contacts, MLS group state, KeyPackages, messages, outbox, inbox dedup, and settings. Validated by an end-to-end persistence test: Alice and Bob complete the Plan 02 messaging flow, drop in-memory MLS state, restore it from disk, and continue messaging across a simulated restart.

**Architecture:** New crate depends on `ghost-identity` + `ghost-core`. Master DB encryption key is derived deterministically from the IdentityKey (HKDF), so unlocking the identity unlocks the DB. Each repository takes a `&Database` (which wraps `Mutex<rusqlite::Connection>`) and exposes a small focused API. MLS state is persisted as **serialized blobs** via openmls 0.8's existing `Serialize`/`Deserialize` for `MlsGroup` — Plan 03 does NOT implement the full `openmls_traits::StorageProvider` trait. That deferral keeps Plan 03 tractable; persistence is at the granularity of "session checkpoint" (load → operate → save), which is appropriate for our 1-on-1 MVP-1.

**Tech Stack:** `rusqlite` v0.32 with `bundled-sqlcipher-vendored-openssl` feature, `hkdf` (already in workspace), all existing ghost-identity/ghost-protocol deps.

**Deliverable Plan 03:** integration test in `crates/ghost-storage/tests/e2e_persistence.rs` that:
1. Alice and Bob create identities + populate KeyPackages
2. Open encrypted DB for each
3. Alice and Bob complete first-contact + bidirectional exchange (Plan 02 flow)
4. Both call `MlsGroupsRepo::save` to checkpoint MLS state
5. Drop the in-memory `MlsSession` instances
6. Reopen DB (simulating process restart)
7. Restore `MlsSession` via `MlsGroupsRepo::load_for_contact`
8. Continue messaging — round-trip persists across restart

**Reference spec:** [docs/superpowers/specs/2026-04-27-ghost-mvp1-design.md](../specs/2026-04-27-ghost-mvp1-design.md), section 6.

**Reference plans:**
- [Plan 01](2026-04-27-ghost-plan-01-foundation-identity.md) — Identity, OS keystore, paths
- [Plan 02](2026-04-27-ghost-plan-02-crypto-protocol.md) — MLS / sealed sender / wire format

---

## Notes for the implementer

**SQLCipher dependency:** `rusqlite` 0.32 with feature `bundled-sqlcipher-vendored-openssl` bundles both SQLCipher-patched SQLite AND a vendored OpenSSL. This works cross-platform (Windows MSVC, macOS, Linux) **provided a C compiler is installed**. On Windows the user already has the MSVC toolchain (Plan 02's openmls 0.8 build needed it). First build of rusqlite with this feature will compile SQLCipher + OpenSSL — expect ~3-6 minutes initial build, cached thereafter.

If the build fails with cryptic linker errors (missing `cc.exe`, missing `OpenSSL`), STOP and report BLOCKED with the error. Common alternatives:
- `bundled-sqlcipher` (without `-vendored-openssl`) — uses system OpenSSL, fails on Windows without OPENSSL_DIR
- `rusqlite` with feature `sqlcipher` (link against system SQLCipher) — requires user to install SQLCipher manually

**Connection threading:** `rusqlite::Connection` is `Send` but not `Sync`. We wrap it in `Mutex<Connection>` so the `Database` itself is `Send + Sync` and can be shared via `Arc`. All repository methods take `&self` and acquire the lock internally for the duration of the call. This is fine for desktop-app concurrency (one user, occasional background tasks).

**MLS state persistence approach:** openmls 0.8's `MlsGroup` does NOT implement `Serialize`/`Deserialize` directly. The proper API for persistence is via the storage provider — but openmls also exposes `MlsGroup::load(provider, &group_id)` and the group state is automatically written to provider storage during operations. For Plan 03 we use a hybrid:
- **In-memory provider** (`OpenMlsRustCrypto::default()`) for MLS operations during a session (matches Plan 02)
- **Snapshot/restore via TLS-codec** at session boundaries: `MlsGroup` provides `tls_serialize_detached()` and `MlsGroup::load_from_tls()` (or equivalent — consult openmls 0.8 docs for exact method names)

If the openmls 0.8 API doesn't have a clean serialize-state-as-bytes pathway, the implementer will need to switch to implementing `StorageProvider` for SQLite — which is a substantial increase in scope and should be flagged as DONE_WITH_CONCERNS.

---

## Task 1: ghost-storage crate skeleton + workspace integration

**Files:**
- Create: `crates/ghost-storage/Cargo.toml`
- Create: `crates/ghost-storage/src/lib.rs`
- Modify: `Cargo.toml` (root)

- [ ] **Step 1: Modify root `Cargo.toml`**

a) Add `"crates/ghost-storage"` to `members = [...]` (after `"crates/ghost-protocol"` and before `"crates/ghost-identity-cli"`).

b) Add to `[workspace.dependencies]` (alphabetically):

```toml
rusqlite = { version = "0.32", features = ["bundled-sqlcipher-vendored-openssl", "blob", "uuid", "chrono"] }
```

- [ ] **Step 2: Create `crates/ghost-storage/Cargo.toml`**

```toml
[package]
name = "ghost-storage"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Ghost storage: SQLCipher-encrypted SQLite, schema, migrations, repositories."

[dependencies]
ghost-core     = { path = "../ghost-core" }
ghost-identity = { path = "../ghost-identity" }

rusqlite = { workspace = true }
hkdf = { workspace = true }
sha2 = { workspace = true }
zeroize = { workspace = true }
hex = { workspace = true }
uuid = { workspace = true }

thiserror = { workspace = true }

[dev-dependencies]
ghost-protocol = { path = "../ghost-protocol" }
tempfile = { workspace = true }
proptest = { workspace = true }
openmls = { workspace = true }
openmls_rust_crypto = { workspace = true }
openmls_traits = { workspace = true }
openmls_basic_credential = { workspace = true }
```

- [ ] **Step 3: Create `crates/ghost-storage/src/lib.rs`**

```rust
//! Ghost storage: SQLCipher-encrypted SQLite database.
//!
//! Provides a `Database` wrapper around `rusqlite::Connection` plus repository APIs
//! for the seven core tables (contacts, mls_groups, my_keypackages, messages,
//! outbox, inbox_dedup, settings). The DB encryption key is derived from the
//! user's IdentityKey via HKDF, so unlocking the identity unlocks the DB.

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-storage");
    }
}
```

- [ ] **Step 4: Verify the workspace compiles**

```bash
cargo +1.87-x86_64-pc-windows-msvc test -p ghost-storage
```

**First build will compile SQLCipher + OpenSSL — expect 3-6 minutes.** Subsequent builds reuse cache.

If the build fails, STOP and report BLOCKED with the exact error. Most likely failure: missing C compiler. On Windows, ensure `cl.exe` is in PATH (Visual Studio Build Tools installed). On Linux, `gcc` and `make` must be installed.

Expected: 1 test passes (smoke).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/ghost-storage/
git commit -m "feat(ghost-storage): scaffold crate with rusqlite + bundled SQLCipher"
```

---

## Task 2: StorageError + Result alias

**Files:**
- Create: `crates/ghost-storage/src/error.rs`
- Modify: `crates/ghost-storage/src/lib.rs`

- [ ] **Step 1: Create `crates/ghost-storage/src/error.rs`**

```rust
//! Top-level error type for ghost-storage.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("schema migration failed at version {version}: {detail}")]
    Migration { version: u32, detail: String },

    #[error("schema downgrade attempted: db is at v{db_version}, app supports v{app_version}")]
    SchemaTooNew { db_version: u32, app_version: u32 },

    #[error("invalid blob in {table}.{column}: {detail}")]
    InvalidBlob {
        table: &'static str,
        column: &'static str,
        detail: String,
    },

    #[error("entity not found: {0}")]
    NotFound(String),

    #[error("invalid input: {0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;
```

- [ ] **Step 2: Modify `crates/ghost-storage/src/lib.rs`**

```rust
//! Ghost storage: SQLCipher-encrypted SQLite database.

pub mod error;

pub use error::{Result, StorageError};

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-storage");
    }
}
```

- [ ] **Step 3: Test + commit**

```bash
cargo test -p ghost-storage
git add crates/ghost-storage/
git commit -m "feat(ghost-storage): StorageError and Result alias"
```

Expected: 1 test passes.

---

## Task 3: Master DB key derivation from IdentityKey

**Files:**
- Create: `crates/ghost-storage/src/master_key.rs`
- Modify: `crates/ghost-storage/src/lib.rs`

The DB encryption key is derived deterministically from the IdentityKey via HKDF. Holders of the IK can derive the DB key; outside parties cannot (since the IK secret is private). This binds the DB to the identity — losing the identity file means losing the DB key.

- [ ] **Step 1: Write failing tests + impl**

Create `crates/ghost-storage/src/master_key.rs`:

```rust
//! Derive the SQLCipher master key from an IdentityKey.
//!
//! Holders of the IK can derive both halves of this key deterministically.
//! Without the IK there is no path to recover this key.

use ghost_identity::IdentityKey;
use hkdf::Hkdf;
use sha2::Sha256;

const HKDF_SALT: &[u8] = b"ghost.db.encryption.v1";
const HKDF_INFO: &[u8] = b"sqlcipher-master-key";
pub const MASTER_KEY_LEN: usize = 32;

/// Derive a 32-byte SQLCipher master key from `ik`.
/// Deterministic: same IK -> same key, on every machine, every time.
pub fn derive_master_key(ik: &IdentityKey) -> [u8; MASTER_KEY_LEN] {
    let seed = ik.secret_bytes();
    let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT), &seed);
    let mut okm = [0u8; MASTER_KEY_LEN];
    hk.expand(HKDF_INFO, &mut okm)
        .expect("32-byte expand always succeeds");
    okm
}

/// Format the key as a SQLCipher hex literal: `x'...'`. SQLCipher accepts this form
/// in `PRAGMA key = ...` and skips its built-in PBKDF2 (since the key is already
/// derived material).
pub fn master_key_pragma(key: &[u8; MASTER_KEY_LEN]) -> String {
    format!("x'{}'", hex::encode(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_is_deterministic_per_identity() {
        let ik = IdentityKey::generate();
        let k1 = derive_master_key(&ik);
        let k2 = derive_master_key(&ik);
        assert_eq!(k1, k2);
    }

    #[test]
    fn distinct_identities_yield_distinct_keys() {
        let a = IdentityKey::generate();
        let b = IdentityKey::generate();
        assert_ne!(derive_master_key(&a), derive_master_key(&b));
    }

    #[test]
    fn pragma_format_matches_sqlcipher_hex_literal() {
        let key = [0xABu8; MASTER_KEY_LEN];
        let pragma = master_key_pragma(&key);
        assert!(pragma.starts_with("x'"));
        assert!(pragma.ends_with('\''));
        assert_eq!(pragma.len(), 32 * 2 + 3); // 64 hex + "x'" + "'"
        assert!(pragma.contains(&"ab".repeat(32)));
    }

    #[test]
    fn master_key_differs_from_identity_secret_bytes() {
        // The key derived for the DB must NOT equal the raw IK secret seed —
        // the HKDF chain must mix in the salt+info to produce distinct material.
        let ik = IdentityKey::generate();
        let key = derive_master_key(&ik);
        assert_ne!(key, ik.secret_bytes());
    }
}
```

- [ ] **Step 2: Modify `crates/ghost-storage/src/lib.rs`**

```rust
pub mod error;
pub mod master_key;

pub use error::{Result, StorageError};
pub use master_key::{derive_master_key, master_key_pragma, MASTER_KEY_LEN};

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-storage");
    }
}
```

- [ ] **Step 3: Test + commit**

```bash
cargo test -p ghost-storage
git add crates/ghost-storage/
git commit -m "feat(ghost-storage): master DB key derivation from IdentityKey"
```

Expected: 5 tests pass.

---

## Task 4: Database struct + open_encrypted

**Files:**
- Create: `crates/ghost-storage/src/database.rs`
- Modify: `crates/ghost-storage/src/lib.rs`

- [ ] **Step 1: Write failing tests + impl**

Create `crates/ghost-storage/src/database.rs`:

```rust
//! Database wrapper around rusqlite::Connection with SQLCipher PRAGMA key.

use crate::master_key::{master_key_pragma, MASTER_KEY_LEN};
use crate::{Result, StorageError};
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Open or create an encrypted SQLite database at `path` using the supplied
    /// 32-byte master key. The key is fed to SQLCipher via `PRAGMA key = x'...'`,
    /// which skips SQLCipher's own PBKDF2 (we already derived the key with HKDF).
    ///
    /// On first creation, the file is empty; SQLCipher will write its encrypted
    /// header on the first write. To make the header deterministic across creates
    /// we explicitly run `PRAGMA cipher_page_size`/`PRAGMA cipher_kdf_iter` to fix
    /// SQLCipher options. Defaults are fine for our case but we pin them so that
    /// future SQLCipher version bumps don't change the file format silently.
    pub fn open_encrypted(path: &Path, master_key: &[u8; MASTER_KEY_LEN]) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;

        // Apply key BEFORE any other statement (SQLCipher requirement).
        let pragma = master_key_pragma(master_key);
        conn.execute_batch(&format!("PRAGMA key = {pragma};"))?;

        // Pin SQLCipher format options for forward stability.
        conn.execute_batch(
            "PRAGMA cipher_page_size = 4096;
             PRAGMA cipher_kdf_iter = 256000;
             PRAGMA cipher_default_kdf_iter = 256000;
             PRAGMA foreign_keys = ON;",
        )?;

        // Verify the key is correct by trying a no-op read. If the key is wrong,
        // SQLCipher returns a "file is not a database" error.
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| {
            row.get::<_, i64>(0)
        })?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Open an in-memory encrypted database — used by tests.
    pub fn open_in_memory(master_key: &[u8; MASTER_KEY_LEN]) -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let pragma = master_key_pragma(master_key);
        conn.execute_batch(&format!("PRAGMA key = {pragma};"))?;
        conn.execute_batch(
            "PRAGMA cipher_page_size = 4096;
             PRAGMA cipher_kdf_iter = 256000;
             PRAGMA foreign_keys = ON;",
        )?;
        // No verify-read needed: in-memory DBs don't have a persisted header.
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Run `f` with the held connection lock. Internal helper for repos.
    pub(crate) fn with_conn<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&Connection) -> Result<R>,
    {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Invalid(format!("connection lock poisoned: {e}")))?;
        f(&conn)
    }

    /// Run `f` with a mutable transaction. Internal helper for repos.
    pub(crate) fn with_tx<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&rusqlite::Transaction<'_>) -> Result<R>,
    {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| StorageError::Invalid(format!("connection lock poisoned: {e}")))?;
        let tx = conn.transaction()?;
        let result = f(&tx)?;
        tx.commit()?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master_key::{derive_master_key, MASTER_KEY_LEN};
    use ghost_identity::IdentityKey;
    use tempfile::tempdir;

    fn fresh_master_key() -> [u8; MASTER_KEY_LEN] {
        derive_master_key(&IdentityKey::generate())
    }

    #[test]
    fn open_in_memory_succeeds() {
        let key = fresh_master_key();
        let db = Database::open_in_memory(&key).unwrap();
        let count: i64 = db
            .with_conn(|c| {
                c.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get(0))
                    .map_err(Into::into)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn open_encrypted_persists_across_handles_with_same_key() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("ghost.db");
        let key = fresh_master_key();

        // Create + write something + close
        {
            let db = Database::open_encrypted(&path, &key).unwrap();
            db.with_conn(|c| {
                c.execute("CREATE TABLE t (v INTEGER)", []).map_err(Into::into)
            })
            .unwrap();
            db.with_tx(|tx| {
                tx.execute("INSERT INTO t (v) VALUES (?1)", [42_i64]).map_err(Into::into)
            })
            .unwrap();
        }

        // Reopen with same key — must read back
        {
            let db = Database::open_encrypted(&path, &key).unwrap();
            let v: i64 = db
                .with_conn(|c| {
                    c.query_row("SELECT v FROM t", [], |r| r.get(0)).map_err(Into::into)
                })
                .unwrap();
            assert_eq!(v, 42);
        }
    }

    #[test]
    fn open_encrypted_fails_with_wrong_key() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("ghost.db");
        let right = fresh_master_key();
        let wrong = fresh_master_key();
        assert_ne!(right, wrong);

        // Write with `right`
        {
            let db = Database::open_encrypted(&path, &right).unwrap();
            db.with_conn(|c| {
                c.execute("CREATE TABLE t (v INTEGER)", [])
                    .map(|_| ())
                    .map_err(Into::into)
            })
            .unwrap();
        }

        // Try to open with `wrong` — must fail
        let err = Database::open_encrypted(&path, &wrong).unwrap_err();
        assert!(matches!(err, StorageError::Sqlite(_)));
    }

    #[test]
    fn foreign_keys_pragma_is_on() {
        let key = fresh_master_key();
        let db = Database::open_in_memory(&key).unwrap();
        let on: i64 = db
            .with_conn(|c| {
                c.query_row("PRAGMA foreign_keys", [], |r| r.get(0))
                    .map_err(Into::into)
            })
            .unwrap();
        assert_eq!(on, 1, "foreign_keys must be enabled for FK constraints to apply");
    }
}
```

- [ ] **Step 2: Modify `lib.rs`**

```rust
pub mod database;
pub mod error;
pub mod master_key;

pub use database::Database;
pub use error::{Result, StorageError};
pub use master_key::{derive_master_key, master_key_pragma, MASTER_KEY_LEN};

#[cfg(test)]
mod smoke_tests { /* unchanged */ }
```

- [ ] **Step 3: Test + commit**

```bash
cargo test -p ghost-storage
```

Expected: 9 tests pass (5 prior + 4 new).

If `open_encrypted_fails_with_wrong_key` doesn't return an error (i.e., SQLCipher silently produces empty results), STOP and report — that's a SQLCipher misconfiguration we need to fix.

```bash
git add crates/ghost-storage/
git commit -m "feat(ghost-storage): Database wrapper with SQLCipher PRAGMA key"
```

---

## Task 5: Schema + migration runner (0001_init.sql)

**Files:**
- Create: `crates/ghost-storage/migrations/0001_init.sql`
- Create: `crates/ghost-storage/src/migrations.rs`
- Modify: `crates/ghost-storage/src/database.rs` (add `migrate` method)
- Modify: `crates/ghost-storage/src/lib.rs`

- [ ] **Step 1: Create `crates/ghost-storage/migrations/0001_init.sql`**

```sql
-- Ghost MVP-1 database schema, version 1.

-- Schema versioning: each applied migration appends a row.
CREATE TABLE IF NOT EXISTS schema_version (
    version    INTEGER PRIMARY KEY,
    applied_at INTEGER NOT NULL
);

-- Contacts.
CREATE TABLE contacts (
    ghost_id      BLOB PRIMARY KEY,
    display_name  TEXT,
    local_alias   TEXT,
    fingerprint   TEXT NOT NULL,
    added_at      INTEGER NOT NULL,
    last_endpoint TEXT,
    verification  INTEGER NOT NULL DEFAULT 0,
    notes         TEXT,
    blocked       INTEGER NOT NULL DEFAULT 0
);

-- MLS group state per 1-on-1 conversation.
CREATE TABLE mls_groups (
    group_id      BLOB PRIMARY KEY,
    contact_id    BLOB NOT NULL,
    state_blob    BLOB NOT NULL,
    current_epoch INTEGER NOT NULL,
    created_at    INTEGER NOT NULL,
    last_updated  INTEGER NOT NULL,
    FOREIGN KEY (contact_id) REFERENCES contacts(ghost_id)
);

CREATE INDEX idx_mls_groups_contact ON mls_groups(contact_id);

-- Our own published KeyPackages.
CREATE TABLE my_keypackages (
    package_id     BLOB PRIMARY KEY,
    package_blob   BLOB NOT NULL,
    private_key    BLOB NOT NULL,
    created_at     INTEGER NOT NULL,
    consumed_at    INTEGER,
    is_last_resort INTEGER NOT NULL DEFAULT 0
);

-- Messages.
CREATE TABLE messages (
    msg_uuid     BLOB PRIMARY KEY,
    contact_id   BLOB NOT NULL,
    direction    INTEGER NOT NULL,
    content_type INTEGER NOT NULL,
    content      TEXT NOT NULL,
    sent_at      INTEGER NOT NULL,
    received_at  INTEGER,
    status       INTEGER NOT NULL DEFAULT 0,
    reply_to     BLOB,
    expires_at   INTEGER,
    FOREIGN KEY (contact_id) REFERENCES contacts(ghost_id)
);

CREATE INDEX idx_messages_contact_time ON messages(contact_id, sent_at);
CREATE INDEX idx_messages_expires ON messages(expires_at) WHERE expires_at IS NOT NULL;

-- Outbox.
CREATE TABLE outbox (
    msg_uuid       BLOB PRIMARY KEY,
    recipient_id   BLOB NOT NULL,
    envelope_blob  BLOB NOT NULL,
    attempts       INTEGER NOT NULL DEFAULT 0,
    next_retry_at  INTEGER NOT NULL,
    last_error     TEXT
);

CREATE INDEX idx_outbox_retry ON outbox(next_retry_at);

-- Inbox dedup.
CREATE TABLE inbox_dedup (
    msg_uuid    BLOB PRIMARY KEY,
    received_at INTEGER NOT NULL
);

CREATE INDEX idx_inbox_dedup_time ON inbox_dedup(received_at);

-- Settings.
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

- [ ] **Step 2: Create `crates/ghost-storage/src/migrations.rs`**

```rust
//! Schema migrations.
//!
//! Each migration is a `&str` of SQL embedded via `include_str!`. They are applied
//! in order if `schema_version` does not contain their version number. All
//! migrations run inside a single transaction so a partial failure rolls back.

use crate::{Database, Result, StorageError};
use std::time::{SystemTime, UNIX_EPOCH};

pub const APP_SCHEMA_VERSION: u32 = 1;

const MIGRATIONS: &[(u32, &str)] = &[(1, include_str!("../migrations/0001_init.sql"))];

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Database {
    /// Apply any unapplied migrations.
    pub fn migrate(&self) -> Result<()> {
        // Ensure schema_version table exists (in a one-shot connection statement —
        // it's idempotent because of IF NOT EXISTS in the migration). We do this
        // outside the transaction so the version probe works on a fresh DB.
        self.with_conn(|c| {
            c.execute(
                "CREATE TABLE IF NOT EXISTS schema_version (
                    version    INTEGER PRIMARY KEY,
                    applied_at INTEGER NOT NULL
                )",
                [],
            )?;
            Ok(())
        })?;

        let current_version: u32 = self.with_conn(|c| {
            c.query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|v| v as u32)
            .map_err(Into::into)
        })?;

        if current_version > APP_SCHEMA_VERSION {
            return Err(StorageError::SchemaTooNew {
                db_version: current_version,
                app_version: APP_SCHEMA_VERSION,
            });
        }

        for &(version, sql) in MIGRATIONS {
            if version <= current_version {
                continue;
            }
            self.with_tx(|tx| {
                tx.execute_batch(sql).map_err(|e| StorageError::Migration {
                    version,
                    detail: e.to_string(),
                })?;
                tx.execute(
                    "INSERT INTO schema_version (version, applied_at) VALUES (?1, ?2)",
                    rusqlite::params![version, now_seconds()],
                )?;
                Ok(())
            })?;
        }
        Ok(())
    }

    /// Read the current schema version (highest applied).
    pub fn schema_version(&self) -> Result<u32> {
        self.with_conn(|c| {
            c.query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|v| v as u32)
            .map_err(Into::into)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master_key::derive_master_key;
    use ghost_identity::IdentityKey;

    fn fresh_db() -> Database {
        Database::open_in_memory(&derive_master_key(&IdentityKey::generate())).unwrap()
    }

    #[test]
    fn migrate_brings_fresh_db_to_app_version() {
        let db = fresh_db();
        db.migrate().unwrap();
        assert_eq!(db.schema_version().unwrap(), APP_SCHEMA_VERSION);
    }

    #[test]
    fn migrate_is_idempotent() {
        let db = fresh_db();
        db.migrate().unwrap();
        db.migrate().unwrap();
        db.migrate().unwrap();
        assert_eq!(db.schema_version().unwrap(), APP_SCHEMA_VERSION);
    }

    #[test]
    fn after_migrate_all_seven_app_tables_exist() {
        let db = fresh_db();
        db.migrate().unwrap();
        let tables: Vec<String> = db
            .with_conn(|c| {
                let mut stmt = c.prepare(
                    "SELECT name FROM sqlite_master \
                     WHERE type = 'table' AND name NOT LIKE 'sqlite_%' \
                     ORDER BY name",
                )?;
                let rows = stmt
                    .query_map([], |r| r.get::<_, String>(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .unwrap();
        // Expect: contacts, inbox_dedup, messages, mls_groups, my_keypackages, outbox, schema_version, settings
        assert_eq!(
            tables,
            vec![
                "contacts".to_string(),
                "inbox_dedup".to_string(),
                "messages".to_string(),
                "mls_groups".to_string(),
                "my_keypackages".to_string(),
                "outbox".to_string(),
                "schema_version".to_string(),
                "settings".to_string(),
            ]
        );
    }

    #[test]
    fn schema_too_new_returns_error() {
        let db = fresh_db();
        // Manually bump schema_version above APP_SCHEMA_VERSION
        db.migrate().unwrap();
        db.with_conn(|c| {
            c.execute(
                "INSERT INTO schema_version (version, applied_at) VALUES (?1, ?2)",
                rusqlite::params![APP_SCHEMA_VERSION + 99, 0_i64],
            )?;
            Ok(())
        })
        .unwrap();
        let err = db.migrate().unwrap_err();
        assert!(matches!(err, StorageError::SchemaTooNew { .. }));
    }
}
```

- [ ] **Step 3: Modify `crates/ghost-storage/src/lib.rs`**

```rust
pub mod database;
pub mod error;
pub mod master_key;
pub mod migrations;

pub use database::Database;
pub use error::{Result, StorageError};
pub use master_key::{derive_master_key, master_key_pragma, MASTER_KEY_LEN};
pub use migrations::APP_SCHEMA_VERSION;

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-storage");
    }
}
```

- [ ] **Step 4: Test + commit**

```bash
cargo test -p ghost-storage
```

Expected: 13 tests pass.

```bash
git add crates/ghost-storage/
git commit -m "feat(ghost-storage): schema v1 + migration runner"
```

---

## Task 6: ContactsRepo

**Files:**
- Create: `crates/ghost-storage/src/repos/mod.rs`
- Create: `crates/ghost-storage/src/repos/contacts.rs`
- Modify: `crates/ghost-storage/src/lib.rs`

- [ ] **Step 1: Create `crates/ghost-storage/src/repos/mod.rs`**

```rust
//! Repositories — one per table. Each takes `&Database` and exposes a focused API.

pub mod contacts;

pub use contacts::{Contact, ContactsRepo, Verification};
```

- [ ] **Step 2: Create `crates/ghost-storage/src/repos/contacts.rs`**

```rust
//! Contacts repository.

use crate::{Database, Result, StorageError};
use ghost_core::GhostId;
use rusqlite::params;

/// Verification status of a contact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verification {
    Unverified = 0,
    Verified = 1,
}

impl Verification {
    pub fn from_i64(v: i64) -> Result<Self> {
        match v {
            0 => Ok(Self::Unverified),
            1 => Ok(Self::Verified),
            other => Err(StorageError::Invalid(format!(
                "unknown verification value {other}"
            ))),
        }
    }
}

#[derive(Clone, Debug)]
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
}

pub struct ContactsRepo<'a> {
    db: &'a Database,
}

impl<'a> ContactsRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Insert a new contact. Errors if the GhostId already exists.
    pub fn insert(&self, contact: &Contact) -> Result<()> {
        self.db.with_tx(|tx| {
            tx.execute(
                "INSERT INTO contacts (
                    ghost_id, display_name, local_alias, fingerprint, added_at,
                    last_endpoint, verification, notes, blocked
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
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
                ],
            )?;
            Ok(())
        })
    }

    /// Fetch a contact by GhostId. Returns `Ok(None)` if absent.
    pub fn get(&self, id: &GhostId) -> Result<Option<Contact>> {
        self.db.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT ghost_id, display_name, local_alias, fingerprint, added_at,
                        last_endpoint, verification, notes, blocked
                   FROM contacts WHERE ghost_id = ?1",
            )?;
            let mut rows = stmt.query(params![id.as_bytes()])?;
            match rows.next()? {
                Some(row) => Ok(Some(Self::row_to_contact(row)?)),
                None => Ok(None),
            }
        })
    }

    /// List all contacts, ordered by `added_at` ascending.
    pub fn list(&self) -> Result<Vec<Contact>> {
        self.db.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT ghost_id, display_name, local_alias, fingerprint, added_at,
                        last_endpoint, verification, notes, blocked
                   FROM contacts ORDER BY added_at ASC",
            )?;
            let rows = stmt
                .query_map([], |row| Ok(Self::row_to_contact(row)))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows.into_iter().collect()
        })
    }

    /// Update mutable fields of an existing contact (display_name, local_alias,
    /// last_endpoint, verification, notes, blocked). The GhostId and fingerprint
    /// are immutable.
    pub fn update(&self, contact: &Contact) -> Result<()> {
        self.db.with_tx(|tx| {
            let n = tx.execute(
                "UPDATE contacts SET
                    display_name = ?2,
                    local_alias = ?3,
                    last_endpoint = ?4,
                    verification = ?5,
                    notes = ?6,
                    blocked = ?7
                 WHERE ghost_id = ?1",
                params![
                    contact.ghost_id.as_bytes(),
                    contact.display_name,
                    contact.local_alias,
                    contact.last_endpoint,
                    contact.verification as i64,
                    contact.notes,
                    contact.blocked as i64,
                ],
            )?;
            if n == 0 {
                return Err(StorageError::NotFound(format!(
                    "contact {}",
                    contact.ghost_id
                )));
            }
            Ok(())
        })
    }

    /// Delete a contact by GhostId. Returns `Ok(true)` if a row was deleted, `Ok(false)` otherwise.
    pub fn delete(&self, id: &GhostId) -> Result<bool> {
        self.db.with_tx(|tx| {
            let n = tx.execute("DELETE FROM contacts WHERE ghost_id = ?1", params![id.as_bytes()])?;
            Ok(n > 0)
        })
    }

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
        })
    }
}

impl Database {
    pub fn contacts(&self) -> ContactsRepo<'_> {
        ContactsRepo::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master_key::derive_master_key;
    use ghost_core::Fingerprint;
    use ghost_identity::IdentityKey;

    fn fresh_db() -> Database {
        let db = Database::open_in_memory(&derive_master_key(&IdentityKey::generate())).unwrap();
        db.migrate().unwrap();
        db
    }

    fn fake_contact(seed: u8, name: &str) -> Contact {
        let id = GhostId::from_bytes([seed; 32]);
        let fp = Fingerprint::of(&id).to_string();
        Contact {
            ghost_id: id,
            display_name: Some(name.to_string()),
            local_alias: None,
            fingerprint: fp,
            added_at: 1700000000 + seed as i64,
            last_endpoint: None,
            verification: Verification::Unverified,
            notes: None,
            blocked: false,
        }
    }

    #[test]
    fn insert_then_get_roundtrips() {
        let db = fresh_db();
        let c = fake_contact(1, "Alice");
        db.contacts().insert(&c).unwrap();
        let loaded = db.contacts().get(&c.ghost_id).unwrap().unwrap();
        assert_eq!(loaded.ghost_id, c.ghost_id);
        assert_eq!(loaded.display_name.as_deref(), Some("Alice"));
        assert_eq!(loaded.fingerprint, c.fingerprint);
    }

    #[test]
    fn get_returns_none_for_missing() {
        let db = fresh_db();
        let id = GhostId::from_bytes([99; 32]);
        assert!(db.contacts().get(&id).unwrap().is_none());
    }

    #[test]
    fn list_orders_by_added_at_asc() {
        let db = fresh_db();
        db.contacts().insert(&fake_contact(3, "C")).unwrap();
        db.contacts().insert(&fake_contact(1, "A")).unwrap();
        db.contacts().insert(&fake_contact(2, "B")).unwrap();
        let list = db.contacts().list().unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].display_name.as_deref(), Some("A"));
        assert_eq!(list[1].display_name.as_deref(), Some("B"));
        assert_eq!(list[2].display_name.as_deref(), Some("C"));
    }

    #[test]
    fn update_changes_mutable_fields() {
        let db = fresh_db();
        let mut c = fake_contact(5, "Old");
        db.contacts().insert(&c).unwrap();
        c.display_name = Some("New".to_string());
        c.verification = Verification::Verified;
        c.blocked = true;
        db.contacts().update(&c).unwrap();
        let loaded = db.contacts().get(&c.ghost_id).unwrap().unwrap();
        assert_eq!(loaded.display_name.as_deref(), Some("New"));
        assert_eq!(loaded.verification, Verification::Verified);
        assert!(loaded.blocked);
    }

    #[test]
    fn update_missing_returns_not_found() {
        let db = fresh_db();
        let c = fake_contact(7, "Ghost");
        let err = db.contacts().update(&c).unwrap_err();
        assert!(matches!(err, StorageError::NotFound(_)));
    }

    #[test]
    fn delete_removes_row() {
        let db = fresh_db();
        let c = fake_contact(8, "Bye");
        db.contacts().insert(&c).unwrap();
        assert!(db.contacts().delete(&c.ghost_id).unwrap());
        assert!(db.contacts().get(&c.ghost_id).unwrap().is_none());
    }

    #[test]
    fn insert_duplicate_errors() {
        let db = fresh_db();
        let c = fake_contact(9, "Once");
        db.contacts().insert(&c).unwrap();
        let err = db.contacts().insert(&c).unwrap_err();
        assert!(matches!(err, StorageError::Sqlite(_)));
    }
}
```

- [ ] **Step 3: Modify `lib.rs`**

```rust
pub mod database;
pub mod error;
pub mod master_key;
pub mod migrations;
pub mod repos;

pub use database::Database;
pub use error::{Result, StorageError};
pub use master_key::{derive_master_key, master_key_pragma, MASTER_KEY_LEN};
pub use migrations::APP_SCHEMA_VERSION;
pub use repos::{Contact, ContactsRepo, Verification};

#[cfg(test)]
mod smoke_tests { /* unchanged */ }
```

- [ ] **Step 4: Test + commit**

Expected: 20 tests pass (13 prior + 7 new).

```bash
git add crates/ghost-storage/
git commit -m "feat(ghost-storage): ContactsRepo (insert/get/list/update/delete)"
```

---

## Task 7: MlsGroupsRepo

**Files:**
- Create: `crates/ghost-storage/src/repos/mls_groups.rs`
- Modify: `crates/ghost-storage/src/repos/mod.rs`
- Modify: `crates/ghost-storage/src/lib.rs`

This repo treats MLS state as opaque bytes — `state_blob: Vec<u8>`. Plan 03 deliverable test (Task 13) verifies that ghost-protocol produces serialized state that this repo can persist, then ghost-protocol can rehydrate it on the other side.

- [ ] **Step 1: Create `crates/ghost-storage/src/repos/mls_groups.rs`**

```rust
//! MLS group state repository. Stores TLS-serialized MlsGroup blobs keyed by group_id.

use crate::{Database, Result, StorageError};
use ghost_core::GhostId;
use rusqlite::params;

#[derive(Clone, Debug)]
pub struct MlsGroupRow {
    pub group_id: [u8; 32],
    pub contact_id: GhostId,
    pub state_blob: Vec<u8>,
    pub current_epoch: u64,
    pub created_at: i64,
    pub last_updated: i64,
}

pub struct MlsGroupsRepo<'a> {
    db: &'a Database,
}

impl<'a> MlsGroupsRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Insert or replace the MLS state for a contact (one group per contact in MVP-1).
    pub fn upsert(&self, row: &MlsGroupRow) -> Result<()> {
        self.db.with_tx(|tx| {
            tx.execute(
                "INSERT OR REPLACE INTO mls_groups (
                    group_id, contact_id, state_blob, current_epoch, created_at, last_updated
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    &row.group_id[..],
                    row.contact_id.as_bytes(),
                    row.state_blob,
                    row.current_epoch as i64,
                    row.created_at,
                    row.last_updated,
                ],
            )?;
            Ok(())
        })
    }

    /// Load the MLS state for a contact. Returns `Ok(None)` if there is no group.
    pub fn load_for_contact(&self, contact: &GhostId) -> Result<Option<MlsGroupRow>> {
        self.db.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT group_id, contact_id, state_blob, current_epoch, created_at, last_updated
                   FROM mls_groups WHERE contact_id = ?1",
            )?;
            let mut rows = stmt.query(params![contact.as_bytes()])?;
            match rows.next()? {
                Some(row) => Ok(Some(Self::row_to_struct(row)?)),
                None => Ok(None),
            }
        })
    }

    /// Delete the MLS state for a contact (e.g., contact removed from address book).
    pub fn delete_for_contact(&self, contact: &GhostId) -> Result<bool> {
        self.db.with_tx(|tx| {
            let n = tx.execute(
                "DELETE FROM mls_groups WHERE contact_id = ?1",
                params![contact.as_bytes()],
            )?;
            Ok(n > 0)
        })
    }

    fn row_to_struct(row: &rusqlite::Row<'_>) -> Result<MlsGroupRow> {
        let group_id_bytes: Vec<u8> = row.get(0)?;
        if group_id_bytes.len() != 32 {
            return Err(StorageError::InvalidBlob {
                table: "mls_groups",
                column: "group_id",
                detail: format!("expected 32 bytes, got {}", group_id_bytes.len()),
            });
        }
        let mut group_id = [0u8; 32];
        group_id.copy_from_slice(&group_id_bytes);

        let contact_bytes: Vec<u8> = row.get(1)?;
        if contact_bytes.len() != 32 {
            return Err(StorageError::InvalidBlob {
                table: "mls_groups",
                column: "contact_id",
                detail: format!("expected 32 bytes, got {}", contact_bytes.len()),
            });
        }
        let mut contact_arr = [0u8; 32];
        contact_arr.copy_from_slice(&contact_bytes);

        let epoch_i64: i64 = row.get(3)?;
        Ok(MlsGroupRow {
            group_id,
            contact_id: GhostId::from_bytes(contact_arr),
            state_blob: row.get(2)?,
            current_epoch: epoch_i64 as u64,
            created_at: row.get(4)?,
            last_updated: row.get(5)?,
        })
    }
}

impl Database {
    pub fn mls_groups(&self) -> MlsGroupsRepo<'_> {
        MlsGroupsRepo::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master_key::derive_master_key;
    use crate::repos::contacts::{Contact, Verification};
    use ghost_core::Fingerprint;
    use ghost_identity::IdentityKey;

    fn fresh_db() -> Database {
        let db = Database::open_in_memory(&derive_master_key(&IdentityKey::generate())).unwrap();
        db.migrate().unwrap();
        db
    }

    fn insert_contact(db: &Database, seed: u8) -> GhostId {
        let id = GhostId::from_bytes([seed; 32]);
        let fp = Fingerprint::of(&id).to_string();
        db.contacts()
            .insert(&Contact {
                ghost_id: id,
                display_name: None,
                local_alias: None,
                fingerprint: fp,
                added_at: 0,
                last_endpoint: None,
                verification: Verification::Unverified,
                notes: None,
                blocked: false,
            })
            .unwrap();
        id
    }

    #[test]
    fn upsert_then_load_roundtrips() {
        let db = fresh_db();
        let contact = insert_contact(&db, 1);
        let row = MlsGroupRow {
            group_id: [42u8; 32],
            contact_id: contact,
            state_blob: vec![1, 2, 3, 4, 5],
            current_epoch: 7,
            created_at: 1700000000,
            last_updated: 1700000060,
        };
        db.mls_groups().upsert(&row).unwrap();
        let loaded = db.mls_groups().load_for_contact(&contact).unwrap().unwrap();
        assert_eq!(loaded.group_id, row.group_id);
        assert_eq!(loaded.state_blob, vec![1, 2, 3, 4, 5]);
        assert_eq!(loaded.current_epoch, 7);
    }

    #[test]
    fn upsert_replaces_existing_state() {
        let db = fresh_db();
        let contact = insert_contact(&db, 2);
        let v1 = MlsGroupRow {
            group_id: [10u8; 32],
            contact_id: contact,
            state_blob: vec![0x11],
            current_epoch: 1,
            created_at: 0,
            last_updated: 0,
        };
        db.mls_groups().upsert(&v1).unwrap();
        let v2 = MlsGroupRow {
            group_id: [10u8; 32],
            contact_id: contact,
            state_blob: vec![0x22, 0x33],
            current_epoch: 2,
            created_at: 0,
            last_updated: 1,
        };
        db.mls_groups().upsert(&v2).unwrap();
        let loaded = db.mls_groups().load_for_contact(&contact).unwrap().unwrap();
        assert_eq!(loaded.state_blob, vec![0x22, 0x33]);
        assert_eq!(loaded.current_epoch, 2);
        assert_eq!(loaded.last_updated, 1);
    }

    #[test]
    fn load_for_unknown_contact_returns_none() {
        let db = fresh_db();
        let contact = GhostId::from_bytes([0xff; 32]);
        assert!(db.mls_groups().load_for_contact(&contact).unwrap().is_none());
    }

    #[test]
    fn upsert_with_unknown_contact_violates_fk() {
        let db = fresh_db();
        // contact NOT inserted — FK should fail.
        let row = MlsGroupRow {
            group_id: [0u8; 32],
            contact_id: GhostId::from_bytes([0xfe; 32]),
            state_blob: vec![],
            current_epoch: 0,
            created_at: 0,
            last_updated: 0,
        };
        let err = db.mls_groups().upsert(&row).unwrap_err();
        assert!(matches!(err, StorageError::Sqlite(_)));
    }

    #[test]
    fn delete_for_contact_removes_state() {
        let db = fresh_db();
        let contact = insert_contact(&db, 3);
        db.mls_groups()
            .upsert(&MlsGroupRow {
                group_id: [0u8; 32],
                contact_id: contact,
                state_blob: vec![],
                current_epoch: 0,
                created_at: 0,
                last_updated: 0,
            })
            .unwrap();
        assert!(db.mls_groups().delete_for_contact(&contact).unwrap());
        assert!(db.mls_groups().load_for_contact(&contact).unwrap().is_none());
    }
}
```

- [ ] **Step 2: Modify `repos/mod.rs`**

```rust
pub mod contacts;
pub mod mls_groups;

pub use contacts::{Contact, ContactsRepo, Verification};
pub use mls_groups::{MlsGroupRow, MlsGroupsRepo};
```

- [ ] **Step 3: Modify `lib.rs` re-exports**

```rust
pub use repos::{Contact, ContactsRepo, MlsGroupRow, MlsGroupsRepo, Verification};
```

- [ ] **Step 4: Test + commit**

Expected: 25 tests pass.

```bash
git add crates/ghost-storage/
git commit -m "feat(ghost-storage): MlsGroupsRepo with state-blob persistence"
```

---

## Task 8: MyKeyPackagesRepo

**Files:**
- Create: `crates/ghost-storage/src/repos/my_keypackages.rs`
- Modify: `crates/ghost-storage/src/repos/mod.rs`
- Modify: `crates/ghost-storage/src/lib.rs`

- [ ] **Step 1: Create `crates/ghost-storage/src/repos/my_keypackages.rs`**

```rust
//! Our own published KeyPackages.

use crate::{Database, Result, StorageError};
use rusqlite::params;

#[derive(Clone, Debug)]
pub struct MyKeyPackageRow {
    /// 32-byte hash of the KeyPackage (e.g., BLAKE3 of TLS-serialized bytes).
    pub package_id: [u8; 32],
    /// TLS-serialized KeyPackage.
    pub package_blob: Vec<u8>,
    /// Private init key (HPKE) — stored alongside so we can process incoming Welcomes
    /// in Plan 04+ (network) when the openmls provider's in-memory storage isn't enough.
    /// In Plan 03 this is opaque bytes only; ghost-protocol decides format.
    pub private_key: Vec<u8>,
    pub created_at: i64,
    /// `Some(t)` once consumed by an incoming Welcome; `None` while still available.
    pub consumed_at: Option<i64>,
    pub is_last_resort: bool,
}

pub struct MyKeyPackagesRepo<'a> {
    db: &'a Database,
}

impl<'a> MyKeyPackagesRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn insert(&self, row: &MyKeyPackageRow) -> Result<()> {
        self.db.with_tx(|tx| {
            tx.execute(
                "INSERT INTO my_keypackages (
                    package_id, package_blob, private_key, created_at, consumed_at, is_last_resort
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    &row.package_id[..],
                    row.package_blob,
                    row.private_key,
                    row.created_at,
                    row.consumed_at,
                    row.is_last_resort as i64,
                ],
            )?;
            Ok(())
        })
    }

    pub fn mark_consumed(&self, package_id: &[u8; 32], when: i64) -> Result<()> {
        self.db.with_tx(|tx| {
            let n = tx.execute(
                "UPDATE my_keypackages SET consumed_at = ?2
                  WHERE package_id = ?1 AND consumed_at IS NULL",
                params![&package_id[..], when],
            )?;
            if n == 0 {
                return Err(StorageError::NotFound(format!(
                    "keypackage {} (or already consumed)",
                    hex::encode(package_id)
                )));
            }
            Ok(())
        })
    }

    /// List unconsumed (available) one-time KeyPackages. Returns oldest first so we
    /// rotate through them.
    pub fn list_available_one_time(&self) -> Result<Vec<MyKeyPackageRow>> {
        self.db.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT package_id, package_blob, private_key, created_at, consumed_at, is_last_resort
                   FROM my_keypackages
                  WHERE consumed_at IS NULL AND is_last_resort = 0
                  ORDER BY created_at ASC",
            )?;
            let rows = stmt
                .query_map([], |row| Ok(Self::row_to_struct(row)))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows.into_iter().collect()
        })
    }

    /// Get the last-resort KeyPackage if one exists.
    pub fn last_resort(&self) -> Result<Option<MyKeyPackageRow>> {
        self.db.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT package_id, package_blob, private_key, created_at, consumed_at, is_last_resort
                   FROM my_keypackages
                  WHERE is_last_resort = 1
                  LIMIT 1",
            )?;
            let mut rows = stmt.query([])?;
            match rows.next()? {
                Some(row) => Ok(Some(Self::row_to_struct(row)?)),
                None => Ok(None),
            }
        })
    }

    fn row_to_struct(row: &rusqlite::Row<'_>) -> Result<MyKeyPackageRow> {
        let pkg_id_bytes: Vec<u8> = row.get(0)?;
        if pkg_id_bytes.len() != 32 {
            return Err(StorageError::InvalidBlob {
                table: "my_keypackages",
                column: "package_id",
                detail: format!("expected 32 bytes, got {}", pkg_id_bytes.len()),
            });
        }
        let mut package_id = [0u8; 32];
        package_id.copy_from_slice(&pkg_id_bytes);
        let is_last_resort: i64 = row.get(5)?;
        Ok(MyKeyPackageRow {
            package_id,
            package_blob: row.get(1)?,
            private_key: row.get(2)?,
            created_at: row.get(3)?,
            consumed_at: row.get(4)?,
            is_last_resort: is_last_resort != 0,
        })
    }
}

impl Database {
    pub fn my_keypackages(&self) -> MyKeyPackagesRepo<'_> {
        MyKeyPackagesRepo::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master_key::derive_master_key;
    use ghost_identity::IdentityKey;

    fn fresh_db() -> Database {
        let db = Database::open_in_memory(&derive_master_key(&IdentityKey::generate())).unwrap();
        db.migrate().unwrap();
        db
    }

    fn fake_kp(seed: u8, last_resort: bool, created_at: i64) -> MyKeyPackageRow {
        MyKeyPackageRow {
            package_id: [seed; 32],
            package_blob: vec![seed; 16],
            private_key: vec![seed; 8],
            created_at,
            consumed_at: None,
            is_last_resort: last_resort,
        }
    }

    #[test]
    fn insert_and_list_available() {
        let db = fresh_db();
        db.my_keypackages().insert(&fake_kp(1, false, 100)).unwrap();
        db.my_keypackages().insert(&fake_kp(2, false, 200)).unwrap();
        let avail = db.my_keypackages().list_available_one_time().unwrap();
        assert_eq!(avail.len(), 2);
        assert_eq!(avail[0].package_id, [1u8; 32]); // oldest first
        assert_eq!(avail[1].package_id, [2u8; 32]);
    }

    #[test]
    fn last_resort_separated_from_one_time_list() {
        let db = fresh_db();
        db.my_keypackages().insert(&fake_kp(1, false, 0)).unwrap();
        db.my_keypackages().insert(&fake_kp(2, true, 0)).unwrap();
        let avail = db.my_keypackages().list_available_one_time().unwrap();
        assert_eq!(avail.len(), 1);
        assert_eq!(avail[0].package_id, [1u8; 32]);
        let lr = db.my_keypackages().last_resort().unwrap().unwrap();
        assert_eq!(lr.package_id, [2u8; 32]);
    }

    #[test]
    fn mark_consumed_excludes_from_list() {
        let db = fresh_db();
        db.my_keypackages().insert(&fake_kp(1, false, 0)).unwrap();
        db.my_keypackages().mark_consumed(&[1u8; 32], 12345).unwrap();
        let avail = db.my_keypackages().list_available_one_time().unwrap();
        assert!(avail.is_empty());
    }

    #[test]
    fn mark_consumed_twice_returns_not_found() {
        let db = fresh_db();
        db.my_keypackages().insert(&fake_kp(1, false, 0)).unwrap();
        db.my_keypackages().mark_consumed(&[1u8; 32], 1).unwrap();
        let err = db.my_keypackages().mark_consumed(&[1u8; 32], 2).unwrap_err();
        assert!(matches!(err, StorageError::NotFound(_)));
    }
}
```

- [ ] **Step 2: Update `repos/mod.rs` and `lib.rs`** — add `pub mod my_keypackages;` and re-exports.

- [ ] **Step 3: Test + commit**

Expected: 29 tests pass.

```bash
git add crates/ghost-storage/
git commit -m "feat(ghost-storage): MyKeyPackagesRepo (insert, mark_consumed, list available)"
```

---

## Task 9: MessagesRepo

**Files:**
- Create: `crates/ghost-storage/src/repos/messages.rs`
- Modify: `crates/ghost-storage/src/repos/mod.rs` and `lib.rs`

- [ ] **Step 1: Create `crates/ghost-storage/src/repos/messages.rs`**

```rust
//! Messages repository.

use crate::{Database, Result, StorageError};
use ghost_core::GhostId;
use rusqlite::params;

#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Outgoing = 0,
    Incoming = 1,
}

impl Direction {
    pub fn from_i64(v: i64) -> Result<Self> {
        match v {
            0 => Ok(Self::Outgoing),
            1 => Ok(Self::Incoming),
            other => Err(StorageError::Invalid(format!("direction {other}"))),
        }
    }
}

#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageStatus {
    Pending = 0,
    Sent = 1,
    Delivered = 2,
    Read = 3,
    Failed = 4,
}

impl MessageStatus {
    pub fn from_i64(v: i64) -> Result<Self> {
        match v {
            0 => Ok(Self::Pending),
            1 => Ok(Self::Sent),
            2 => Ok(Self::Delivered),
            3 => Ok(Self::Read),
            4 => Ok(Self::Failed),
            other => Err(StorageError::Invalid(format!("message status {other}"))),
        }
    }
}

#[derive(Clone, Debug)]
pub struct MessageRow {
    pub msg_uuid: [u8; 16],
    pub contact_id: GhostId,
    pub direction: Direction,
    pub content_type: i64,
    pub content: String,
    pub sent_at: i64,
    pub received_at: Option<i64>,
    pub status: MessageStatus,
    pub reply_to: Option<[u8; 16]>,
    pub expires_at: Option<i64>,
}

pub struct MessagesRepo<'a> {
    db: &'a Database,
}

impl<'a> MessagesRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn insert(&self, msg: &MessageRow) -> Result<()> {
        self.db.with_tx(|tx| {
            tx.execute(
                "INSERT INTO messages (
                    msg_uuid, contact_id, direction, content_type, content,
                    sent_at, received_at, status, reply_to, expires_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    &msg.msg_uuid[..],
                    msg.contact_id.as_bytes(),
                    msg.direction as i64,
                    msg.content_type,
                    msg.content,
                    msg.sent_at,
                    msg.received_at,
                    msg.status as i64,
                    msg.reply_to.as_ref().map(|b| &b[..]),
                    msg.expires_at,
                ],
            )?;
            Ok(())
        })
    }

    /// Page through messages for a contact, ordered by sent_at ascending.
    pub fn list_for_contact(
        &self,
        contact: &GhostId,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<MessageRow>> {
        self.db.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT msg_uuid, contact_id, direction, content_type, content,
                        sent_at, received_at, status, reply_to, expires_at
                   FROM messages
                  WHERE contact_id = ?1
                  ORDER BY sent_at ASC
                  LIMIT ?2 OFFSET ?3",
            )?;
            let rows = stmt
                .query_map(
                    params![contact.as_bytes(), limit as i64, offset as i64],
                    |row| Ok(Self::row_to_struct(row)),
                )?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows.into_iter().collect()
        })
    }

    pub fn update_status(&self, msg_uuid: &[u8; 16], status: MessageStatus) -> Result<()> {
        self.db.with_tx(|tx| {
            let n = tx.execute(
                "UPDATE messages SET status = ?2 WHERE msg_uuid = ?1",
                params![&msg_uuid[..], status as i64],
            )?;
            if n == 0 {
                return Err(StorageError::NotFound(format!(
                    "message {}",
                    hex::encode(msg_uuid)
                )));
            }
            Ok(())
        })
    }

    /// Delete messages whose `expires_at` is past `now`. Returns the number of deletions.
    pub fn purge_expired(&self, now: i64) -> Result<usize> {
        self.db.with_tx(|tx| {
            let n = tx.execute(
                "DELETE FROM messages WHERE expires_at IS NOT NULL AND expires_at < ?1",
                params![now],
            )?;
            Ok(n)
        })
    }

    fn row_to_struct(row: &rusqlite::Row<'_>) -> Result<MessageRow> {
        let uuid_bytes: Vec<u8> = row.get(0)?;
        if uuid_bytes.len() != 16 {
            return Err(StorageError::InvalidBlob {
                table: "messages",
                column: "msg_uuid",
                detail: format!("expected 16 bytes, got {}", uuid_bytes.len()),
            });
        }
        let mut msg_uuid = [0u8; 16];
        msg_uuid.copy_from_slice(&uuid_bytes);

        let contact_bytes: Vec<u8> = row.get(1)?;
        if contact_bytes.len() != 32 {
            return Err(StorageError::InvalidBlob {
                table: "messages",
                column: "contact_id",
                detail: format!("expected 32 bytes, got {}", contact_bytes.len()),
            });
        }
        let mut contact_arr = [0u8; 32];
        contact_arr.copy_from_slice(&contact_bytes);

        let reply_to_bytes: Option<Vec<u8>> = row.get(8)?;
        let reply_to = match reply_to_bytes {
            None => None,
            Some(b) => {
                if b.len() != 16 {
                    return Err(StorageError::InvalidBlob {
                        table: "messages",
                        column: "reply_to",
                        detail: format!("expected 16 bytes, got {}", b.len()),
                    });
                }
                let mut arr = [0u8; 16];
                arr.copy_from_slice(&b);
                Some(arr)
            }
        };

        let direction: i64 = row.get(2)?;
        let status: i64 = row.get(7)?;

        Ok(MessageRow {
            msg_uuid,
            contact_id: GhostId::from_bytes(contact_arr),
            direction: Direction::from_i64(direction)?,
            content_type: row.get(3)?,
            content: row.get(4)?,
            sent_at: row.get(5)?,
            received_at: row.get(6)?,
            status: MessageStatus::from_i64(status)?,
            reply_to,
            expires_at: row.get(9)?,
        })
    }
}

impl Database {
    pub fn messages(&self) -> MessagesRepo<'_> {
        MessagesRepo::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master_key::derive_master_key;
    use crate::repos::contacts::{Contact, Verification};
    use ghost_core::Fingerprint;
    use ghost_identity::IdentityKey;

    fn fresh_db_with_contact(seed: u8) -> (Database, GhostId) {
        let db = Database::open_in_memory(&derive_master_key(&IdentityKey::generate())).unwrap();
        db.migrate().unwrap();
        let id = GhostId::from_bytes([seed; 32]);
        let fp = Fingerprint::of(&id).to_string();
        db.contacts()
            .insert(&Contact {
                ghost_id: id,
                display_name: None,
                local_alias: None,
                fingerprint: fp,
                added_at: 0,
                last_endpoint: None,
                verification: Verification::Unverified,
                notes: None,
                blocked: false,
            })
            .unwrap();
        (db, id)
    }

    fn msg(uuid_seed: u8, contact: GhostId, direction: Direction, sent_at: i64) -> MessageRow {
        MessageRow {
            msg_uuid: [uuid_seed; 16],
            contact_id: contact,
            direction,
            content_type: 0,
            content: format!("hello-{uuid_seed}"),
            sent_at,
            received_at: if direction == Direction::Incoming { Some(sent_at + 1) } else { None },
            status: MessageStatus::Pending,
            reply_to: None,
            expires_at: None,
        }
    }

    #[test]
    fn insert_then_list() {
        let (db, contact) = fresh_db_with_contact(1);
        db.messages().insert(&msg(1, contact, Direction::Outgoing, 100)).unwrap();
        db.messages().insert(&msg(2, contact, Direction::Incoming, 200)).unwrap();
        let list = db.messages().list_for_contact(&contact, 100, 0).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].content, "hello-1");
        assert_eq!(list[1].content, "hello-2");
    }

    #[test]
    fn list_paginates() {
        let (db, contact) = fresh_db_with_contact(2);
        for i in 0..10u8 {
            db.messages().insert(&msg(i + 1, contact, Direction::Outgoing, i as i64)).unwrap();
        }
        let page = db.messages().list_for_contact(&contact, 3, 5).unwrap();
        assert_eq!(page.len(), 3);
        assert_eq!(page[0].content, "hello-6");
        assert_eq!(page[2].content, "hello-8");
    }

    #[test]
    fn update_status_changes_field() {
        let (db, contact) = fresh_db_with_contact(3);
        db.messages().insert(&msg(1, contact, Direction::Outgoing, 0)).unwrap();
        db.messages().update_status(&[1u8; 16], MessageStatus::Delivered).unwrap();
        let list = db.messages().list_for_contact(&contact, 10, 0).unwrap();
        assert_eq!(list[0].status, MessageStatus::Delivered);
    }

    #[test]
    fn purge_expired_removes_only_expired() {
        let (db, contact) = fresh_db_with_contact(4);
        let mut keep = msg(1, contact, Direction::Outgoing, 0);
        keep.expires_at = Some(2_000_000_000);
        let mut go = msg(2, contact, Direction::Outgoing, 0);
        go.expires_at = Some(1_700_000_000);
        db.messages().insert(&keep).unwrap();
        db.messages().insert(&go).unwrap();
        let removed = db.messages().purge_expired(1_800_000_000).unwrap();
        assert_eq!(removed, 1);
        let remaining = db.messages().list_for_contact(&contact, 10, 0).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].msg_uuid, [1u8; 16]);
    }
}
```

- [ ] **Step 2: Update `repos/mod.rs` and `lib.rs`** with the new module + re-exports.

- [ ] **Step 3: Test + commit**

Expected: 33 tests pass.

```bash
git add crates/ghost-storage/
git commit -m "feat(ghost-storage): MessagesRepo (insert, paginate, status, purge expired)"
```

---

## Task 10: Outbox + InboxDedup + Settings repos (bunched)

**Files:**
- Create: `crates/ghost-storage/src/repos/outbox.rs`
- Create: `crates/ghost-storage/src/repos/inbox_dedup.rs`
- Create: `crates/ghost-storage/src/repos/settings.rs`
- Modify: `crates/ghost-storage/src/repos/mod.rs` and `lib.rs`

These are small, similar repositories. Implement all three together with one commit.

- [ ] **Step 1: Create `crates/ghost-storage/src/repos/outbox.rs`**

```rust
//! Outbox repository: outgoing envelopes awaiting send.

use crate::{Database, Result, StorageError};
use ghost_core::GhostId;
use rusqlite::params;

#[derive(Clone, Debug)]
pub struct OutboxRow {
    pub msg_uuid: [u8; 16],
    pub recipient_id: GhostId,
    pub envelope_blob: Vec<u8>,
    pub attempts: u32,
    pub next_retry_at: i64,
    pub last_error: Option<String>,
}

pub struct OutboxRepo<'a> {
    db: &'a Database,
}

impl<'a> OutboxRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn enqueue(&self, row: &OutboxRow) -> Result<()> {
        self.db.with_tx(|tx| {
            tx.execute(
                "INSERT INTO outbox (msg_uuid, recipient_id, envelope_blob, attempts, next_retry_at, last_error)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    &row.msg_uuid[..],
                    row.recipient_id.as_bytes(),
                    row.envelope_blob,
                    row.attempts as i64,
                    row.next_retry_at,
                    row.last_error,
                ],
            )?;
            Ok(())
        })
    }

    /// List rows whose `next_retry_at <= now`, oldest first. Limit caps the batch.
    pub fn due(&self, now: i64, limit: u32) -> Result<Vec<OutboxRow>> {
        self.db.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT msg_uuid, recipient_id, envelope_blob, attempts, next_retry_at, last_error
                   FROM outbox
                  WHERE next_retry_at <= ?1
                  ORDER BY next_retry_at ASC
                  LIMIT ?2",
            )?;
            let rows = stmt
                .query_map(params![now, limit as i64], |row| Ok(Self::row_to_struct(row)))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows.into_iter().collect()
        })
    }

    /// Remove a row after successful send.
    pub fn remove(&self, msg_uuid: &[u8; 16]) -> Result<()> {
        self.db.with_tx(|tx| {
            tx.execute("DELETE FROM outbox WHERE msg_uuid = ?1", params![&msg_uuid[..]])?;
            Ok(())
        })
    }

    /// Increment attempts, set next_retry_at, optionally record error string.
    pub fn record_failure(
        &self,
        msg_uuid: &[u8; 16],
        next_retry_at: i64,
        error: Option<&str>,
    ) -> Result<()> {
        self.db.with_tx(|tx| {
            let n = tx.execute(
                "UPDATE outbox
                    SET attempts = attempts + 1,
                        next_retry_at = ?2,
                        last_error = ?3
                  WHERE msg_uuid = ?1",
                params![&msg_uuid[..], next_retry_at, error],
            )?;
            if n == 0 {
                return Err(StorageError::NotFound(format!(
                    "outbox row {}",
                    hex::encode(msg_uuid)
                )));
            }
            Ok(())
        })
    }

    fn row_to_struct(row: &rusqlite::Row<'_>) -> Result<OutboxRow> {
        let uuid_bytes: Vec<u8> = row.get(0)?;
        if uuid_bytes.len() != 16 {
            return Err(StorageError::InvalidBlob {
                table: "outbox",
                column: "msg_uuid",
                detail: format!("expected 16, got {}", uuid_bytes.len()),
            });
        }
        let mut msg_uuid = [0u8; 16];
        msg_uuid.copy_from_slice(&uuid_bytes);
        let recipient_bytes: Vec<u8> = row.get(1)?;
        if recipient_bytes.len() != 32 {
            return Err(StorageError::InvalidBlob {
                table: "outbox",
                column: "recipient_id",
                detail: format!("expected 32, got {}", recipient_bytes.len()),
            });
        }
        let mut rid = [0u8; 32];
        rid.copy_from_slice(&recipient_bytes);
        let attempts_i64: i64 = row.get(3)?;
        Ok(OutboxRow {
            msg_uuid,
            recipient_id: GhostId::from_bytes(rid),
            envelope_blob: row.get(2)?,
            attempts: attempts_i64 as u32,
            next_retry_at: row.get(4)?,
            last_error: row.get(5)?,
        })
    }
}

impl Database {
    pub fn outbox(&self) -> OutboxRepo<'_> {
        OutboxRepo::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master_key::derive_master_key;
    use ghost_identity::IdentityKey;

    fn fresh_db() -> Database {
        let db = Database::open_in_memory(&derive_master_key(&IdentityKey::generate())).unwrap();
        db.migrate().unwrap();
        db
    }

    fn row(seed: u8, next: i64) -> OutboxRow {
        OutboxRow {
            msg_uuid: [seed; 16],
            recipient_id: GhostId::from_bytes([seed; 32]),
            envelope_blob: vec![seed],
            attempts: 0,
            next_retry_at: next,
            last_error: None,
        }
    }

    #[test]
    fn enqueue_and_due() {
        let db = fresh_db();
        db.outbox().enqueue(&row(1, 100)).unwrap();
        db.outbox().enqueue(&row(2, 200)).unwrap();
        let due = db.outbox().due(150, 10).unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].msg_uuid, [1u8; 16]);
    }

    #[test]
    fn remove_dequeues() {
        let db = fresh_db();
        db.outbox().enqueue(&row(1, 0)).unwrap();
        db.outbox().remove(&[1u8; 16]).unwrap();
        let due = db.outbox().due(1000, 10).unwrap();
        assert!(due.is_empty());
    }

    #[test]
    fn record_failure_increments_attempts() {
        let db = fresh_db();
        db.outbox().enqueue(&row(1, 0)).unwrap();
        db.outbox().record_failure(&[1u8; 16], 500, Some("network down")).unwrap();
        let due = db.outbox().due(1000, 10).unwrap();
        assert_eq!(due[0].attempts, 1);
        assert_eq!(due[0].next_retry_at, 500);
        assert_eq!(due[0].last_error.as_deref(), Some("network down"));
    }
}
```

- [ ] **Step 2: Create `crates/ghost-storage/src/repos/inbox_dedup.rs`**

```rust
//! Inbox dedup: short-lived "we already saw this msg_uuid" set.

use crate::{Database, Result};
use rusqlite::params;

pub struct InboxDedupRepo<'a> {
    db: &'a Database,
}

impl<'a> InboxDedupRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Insert an entry. Returns `true` if it was new (insert succeeded), `false`
    /// if the msg_uuid is already present (duplicate).
    pub fn try_insert(&self, msg_uuid: &[u8; 16], received_at: i64) -> Result<bool> {
        self.db.with_tx(|tx| {
            let n = tx.execute(
                "INSERT OR IGNORE INTO inbox_dedup (msg_uuid, received_at) VALUES (?1, ?2)",
                params![&msg_uuid[..], received_at],
            )?;
            Ok(n == 1)
        })
    }

    /// Drop entries older than `before`.
    pub fn purge_older_than(&self, before: i64) -> Result<usize> {
        self.db.with_tx(|tx| {
            let n = tx.execute(
                "DELETE FROM inbox_dedup WHERE received_at < ?1",
                params![before],
            )?;
            Ok(n)
        })
    }
}

impl Database {
    pub fn inbox_dedup(&self) -> InboxDedupRepo<'_> {
        InboxDedupRepo::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master_key::derive_master_key;
    use ghost_identity::IdentityKey;

    fn fresh_db() -> Database {
        let db = Database::open_in_memory(&derive_master_key(&IdentityKey::generate())).unwrap();
        db.migrate().unwrap();
        db
    }

    #[test]
    fn first_insert_returns_true_duplicate_returns_false() {
        let db = fresh_db();
        assert!(db.inbox_dedup().try_insert(&[1u8; 16], 100).unwrap());
        assert!(!db.inbox_dedup().try_insert(&[1u8; 16], 200).unwrap());
    }

    #[test]
    fn purge_removes_old_entries() {
        let db = fresh_db();
        db.inbox_dedup().try_insert(&[1u8; 16], 100).unwrap();
        db.inbox_dedup().try_insert(&[2u8; 16], 200).unwrap();
        let removed = db.inbox_dedup().purge_older_than(150).unwrap();
        assert_eq!(removed, 1);
        // [1] is gone, [2] remains
        assert!(db.inbox_dedup().try_insert(&[1u8; 16], 999).unwrap()); // fresh insert succeeds
        assert!(!db.inbox_dedup().try_insert(&[2u8; 16], 999).unwrap()); // still a dup
    }
}
```

- [ ] **Step 3: Create `crates/ghost-storage/src/repos/settings.rs`**

```rust
//! Settings: simple key-value strings.

use crate::{Database, Result};
use rusqlite::params;

pub struct SettingsRepo<'a> {
    db: &'a Database,
}

impl<'a> SettingsRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        self.db.with_tx(|tx| {
            tx.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )?;
            Ok(())
        })
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        self.db.with_conn(|c| {
            let mut stmt = c.prepare("SELECT value FROM settings WHERE key = ?1")?;
            let mut rows = stmt.query(params![key])?;
            match rows.next()? {
                Some(row) => Ok(Some(row.get::<_, String>(0)?)),
                None => Ok(None),
            }
        })
    }

    pub fn delete(&self, key: &str) -> Result<bool> {
        self.db.with_tx(|tx| {
            let n = tx.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
            Ok(n > 0)
        })
    }
}

impl Database {
    pub fn settings(&self) -> SettingsRepo<'_> {
        SettingsRepo::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master_key::derive_master_key;
    use ghost_identity::IdentityKey;

    fn fresh_db() -> Database {
        let db = Database::open_in_memory(&derive_master_key(&IdentityKey::generate())).unwrap();
        db.migrate().unwrap();
        db
    }

    #[test]
    fn set_then_get() {
        let db = fresh_db();
        db.settings().set("retention", "30d").unwrap();
        assert_eq!(db.settings().get("retention").unwrap().as_deref(), Some("30d"));
    }

    #[test]
    fn set_overwrites() {
        let db = fresh_db();
        db.settings().set("k", "v1").unwrap();
        db.settings().set("k", "v2").unwrap();
        assert_eq!(db.settings().get("k").unwrap().as_deref(), Some("v2"));
    }

    #[test]
    fn delete_removes_entry() {
        let db = fresh_db();
        db.settings().set("k", "v").unwrap();
        assert!(db.settings().delete("k").unwrap());
        assert!(db.settings().get("k").unwrap().is_none());
    }

    #[test]
    fn get_missing_returns_none() {
        let db = fresh_db();
        assert!(db.settings().get("nope").unwrap().is_none());
    }
}
```

- [ ] **Step 4: Update `crates/ghost-storage/src/repos/mod.rs`**

```rust
pub mod contacts;
pub mod inbox_dedup;
pub mod messages;
pub mod mls_groups;
pub mod my_keypackages;
pub mod outbox;
pub mod settings;

pub use contacts::{Contact, ContactsRepo, Verification};
pub use inbox_dedup::InboxDedupRepo;
pub use messages::{Direction, MessageRow, MessageStatus, MessagesRepo};
pub use mls_groups::{MlsGroupRow, MlsGroupsRepo};
pub use my_keypackages::{MyKeyPackageRow, MyKeyPackagesRepo};
pub use outbox::{OutboxRepo, OutboxRow};
pub use settings::SettingsRepo;
```

- [ ] **Step 5: Update `crates/ghost-storage/src/lib.rs` re-exports**

Add the new repo types to the `pub use repos::{...}` block.

- [ ] **Step 6: Test + commit**

Expected: 43 tests pass (33 prior + 3 outbox + 2 inbox_dedup + 4 settings + bumps for messages tests).

Actually count: 33 (after Task 9) + 3 (outbox) + 2 (inbox_dedup) + 4 (settings) = 42. The exact number may differ by ±1 depending on exact test additions. Don't gate on the exact count — gate on "all pass, no failures".

```bash
git add crates/ghost-storage/
git commit -m "feat(ghost-storage): OutboxRepo + InboxDedupRepo + SettingsRepo"
```

---

## Task 11: Add MlsSession::serialize_state / deserialize_state in ghost-protocol

**Files:**
- Modify: `crates/ghost-protocol/src/mls_session.rs`

This is a **companion change to ghost-protocol**. We add two methods so ghost-storage can persist MLS state without depending on openmls internals.

**Implementer note:** consult openmls 0.8 docs for the actual serialization API. Likely candidates:
- `MlsGroup::tls_serialize_detached()` — if MlsGroup implements `TlsSerialize`
- `MlsGroup::save(provider)` and `MlsGroup::load(provider, &group_id)` — provider-backed pathway
- A `GroupContext` or `MlsGroupSnapshot` type with explicit serialization

If openmls 0.8 only supports provider-backed persistence (no detached serialization), this task becomes:
1. Provide `MlsSession::group_id() -> Vec<u8>` so callers can save/load via provider+group_id
2. Update Plan 03 Task 13's deliverable test to use openmls's provider for persistence (not our state_blob in mls_groups table)

Report DONE_WITH_CONCERNS in that case so the controller adjusts Task 13's test.

- [ ] **Step 1: Append serialize/deserialize to `MlsSession`**

```rust
impl MlsSession {
    /// Serialize the entire MLS group state to bytes for persistence.
    /// Round-trips with `deserialize_state`.
    pub fn serialize_state(&self) -> Result<Vec<u8>> {
        use openmls::prelude::tls_codec::Serialize as _;
        // Implementer: confirm openmls 0.8's MlsGroup serialization API.
        // Likely: self.group.tls_serialize_detached().map_err(...)
        self.group
            .tls_serialize_detached()
            .map_err(|e| ProtoError::Mls(format!("serialize group state: {e}")))
    }

    /// Restore an MlsSession from previously-serialized state.
    pub fn deserialize_state(
        provider: &GhostMlsProvider,
        bytes: &[u8],
    ) -> Result<Self> {
        use openmls::prelude::tls_codec::Deserialize as _;
        // Implementer: confirm openmls 0.8's deserialize signature.
        // Likely: MlsGroup::tls_deserialize(&mut &bytes[..])
        let group = openmls::group::MlsGroup::tls_deserialize(&mut &bytes[..])
            .map_err(|e| ProtoError::Mls(format!("deserialize group state: {e}")))?;
        // Some openmls versions require linking the group back to the provider's storage —
        // if there's a `group.set_provider(provider)` or similar, call it. If not, the
        // group state is self-contained.
        let _ = provider; // suppress unused warning if provider isn't needed
        Ok(Self { group })
    }

    /// Group ID as raw bytes — useful for keying persisted state.
    pub fn group_id_bytes(&self) -> Vec<u8> {
        self.group.group_id().as_slice().to_vec()
    }

    pub fn current_epoch(&self) -> u64 {
        self.epoch()
    }
}

#[cfg(test)]
mod state_persist_tests {
    use super::*;
    use crate::key_package::generate_key_package;
    use crate::mls_provider::new_provider;
    use openmls::prelude::tls_codec::{Deserialize as TlsDeserialize, Serialize as TlsSerialize};
    use openmls::prelude::MlsMessageIn;

    /// Build alice+bob in an established MLS group (alice creator, bob joined).
    /// Returns a tuple ((alice_provider, alice_session), (bob_provider, bob_session)).
    fn alice_and_bob_in_group() -> (
        (GhostMlsProvider, MlsSession, openmls_basic_credential::SignatureKeyPair, ghost_identity::IdentityKey, ghost_identity::DeviceKey),
        (GhostMlsProvider, MlsSession, openmls_basic_credential::SignatureKeyPair, ghost_identity::IdentityKey, ghost_identity::DeviceKey),
    ) {
        let alice_provider = new_provider();
        let alice_ik = ghost_identity::IdentityKey::generate();
        let alice_dk = ghost_identity::DeviceKey::generate(&alice_ik);
        let mut alice = MlsSession::create(&alice_provider, &alice_ik, &alice_dk).unwrap();
        let alice_signer = MlsSession::signer_from_dk(&alice_dk);

        let bob_provider = new_provider();
        let bob_ik = ghost_identity::IdentityKey::generate();
        let bob_dk = ghost_identity::DeviceKey::generate(&bob_ik);
        let bob_kp = generate_key_package(&bob_provider, &bob_ik, &bob_dk).unwrap();
        let bob_signer = MlsSession::signer_from_dk(&bob_dk);

        let invite = alice.add_member(&alice_provider, &alice_signer, bob_kp).unwrap();
        let welcome_bytes = invite.welcome.tls_serialize_detached().unwrap();
        let welcome_in = MlsMessageIn::tls_deserialize(&mut welcome_bytes.as_slice()).unwrap();
        let bob = MlsSession::join_via_welcome(&bob_provider, welcome_in).unwrap();

        ((alice_provider, alice, alice_signer, alice_ik, alice_dk),
         (bob_provider, bob, bob_signer, bob_ik, bob_dk))
    }

    #[test]
    fn serialize_then_deserialize_preserves_epoch_and_messaging() {
        let ((alice_p, mut alice, alice_s, _, _), (bob_p, mut bob, bob_s, _, _)) =
            alice_and_bob_in_group();

        // Send one message before snapshot to advance state.
        let wire = alice.encrypt_app_message(&alice_p, &alice_s, b"warmup").unwrap();
        let _ = bob.decrypt_app_message(&bob_p, &wire).unwrap();

        // Snapshot Alice's state.
        let alice_state_bytes = alice.serialize_state().unwrap();
        let pre_epoch = alice.current_epoch();

        // Drop alice (simulate process restart) and rebuild.
        drop(alice);
        let mut alice_restored = MlsSession::deserialize_state(&alice_p, &alice_state_bytes).unwrap();
        assert_eq!(alice_restored.current_epoch(), pre_epoch);

        // Continue messaging — Alice (restored) -> Bob.
        let wire2 = alice_restored.encrypt_app_message(&alice_p, &alice_s, b"after restart").unwrap();
        let recovered = bob.decrypt_app_message(&bob_p, &wire2).unwrap();
        assert_eq!(recovered, b"after restart");
    }
}
```

- [ ] **Step 2: Adjust if openmls 0.8 API differs**

If the test fails with "TlsSerialize is not implemented for MlsGroup", consult docs for the actual persistence API. Likely fallbacks:
1. `MlsGroup::save(provider)` (provider-backed) + `MlsGroup::load(provider, &group_id)` for restore. In that case, `serialize_state` returns the group_id bytes only, and `deserialize_state` calls `MlsGroup::load`. This couples persistence to the provider — fine for our use (one provider per Identity).
2. Manual snapshot via `GroupContext` + `RatchetTree` separately serialized — much more work.

If you have to switch to (1), adjust the function bodies and the deliverable test in Task 13 accordingly. Document the decision in the commit message.

- [ ] **Step 3: Run tests**

```bash
cargo test -p ghost-protocol
```

Expected: count climbs by 1 (added `serialize_then_deserialize_preserves_epoch_and_messaging`).

- [ ] **Step 4: Commit**

```bash
git add crates/ghost-protocol/src/mls_session.rs
git commit -m "feat(ghost-protocol): MlsSession::serialize_state / deserialize_state for persistence"
```

---

## Task 12: Add `db_master_key` accessor to ghost-identity::Identity

**Files:**
- Modify: `crates/ghost-identity/src/identity.rs`
- Modify: `crates/ghost-identity/src/lib.rs`

This is a tiny companion change. The CLI and orchestration layers will need to derive the DB master key from a loaded Identity. Rather than expose `secret_bytes()` widely, we add a focused method that derives just the DB key.

The actual derivation lives in ghost-storage (`derive_master_key`). To avoid a cross-crate cycle (ghost-identity → ghost-storage), we DON'T move the derivation. Instead, ghost-identity exposes `secret_bytes()` (already exists) and ghost-storage takes an `&IdentityKey` to derive.

**This task is informational** — verify there's nothing to change. The plumbing is already in place from Plan 02 Task 6 (where `secret_bytes` was added to `IdentityKey`).

- [ ] **Step 1: Verify**

```bash
cargo test -p ghost-storage
cargo test -p ghost-identity -- --test-threads=1
```

Both should pass without changes. If they don't, debug — the existing `secret_bytes()` accessor must already work.

If everything passes: this task is complete with no code changes. Skip to Step 2.

- [ ] **Step 2: Empty commit to mark milestone**

We could skip the commit entirely, but a no-op marker keeps the plan's commit cadence consistent.

```bash
git commit --allow-empty -m "chore(plan-03): Identity already exposes secret_bytes() for DB key derivation"
```

(If you'd rather skip the empty commit, just move on to Task 13.)

---

## Task 13: End-to-end persistence integration test

**Files:**
- Create: `crates/ghost-storage/tests/e2e_persistence.rs`

**This is the main deliverable of Plan 03.** Alice and Bob complete the Plan 02 messaging flow, persist their state to disk, drop the in-memory MlsSessions, reopen the encrypted DBs, restore the sessions, and continue messaging successfully.

- [ ] **Step 1: Create `crates/ghost-storage/tests/e2e_persistence.rs`**

```rust
//! Plan 03 deliverable: messaging round-trips across a simulated process restart.
//!
//! Flow:
//! 1. Alice + Bob create identities, populate KeyPackages.
//! 2. Open per-user encrypted DB.
//! 3. Run the Plan 02 first-contact + bidirectional exchange.
//! 4. Each side persists MLS group state to disk.
//! 5. Drop in-memory sessions.
//! 6. Reopen DB, restore MlsSession.
//! 7. Continue messaging (Alice -> Bob), verify the new message is received.

use ghost_core::{Fingerprint, GhostId};
use ghost_identity::Identity;
use ghost_protocol::{
    delivery_public, new_provider, populate_initial_keypackages, unwrap_message,
    wrap_message, MlsSession, MsgType, PayloadType,
};
use ghost_storage::{
    derive_master_key, Contact, Database, MlsGroupRow, Verification,
};
use openmls::prelude::tls_codec::{Deserialize as TlsDeserialize, Serialize as TlsSerialize};
use openmls::prelude::{KeyPackage, KeyPackageIn, MlsMessageIn};
use openmls_traits::OpenMlsProvider;
use tempfile::tempdir;

#[test]
fn alice_and_bob_persist_and_continue_after_restart() {
    let dir = tempdir().unwrap();
    let alice_db_path = dir.path().join("alice.db");
    let bob_db_path = dir.path().join("bob.db");

    // ===== Identity setup =====
    let mut alice_id = Identity::generate(Some("Alice".into()), 1700000000);
    let mut bob_id = Identity::generate(Some("Bob".into()), 1700000000);

    let alice_key = derive_master_key(&alice_id.identity_key);
    let bob_key = derive_master_key(&bob_id.identity_key);

    // ===== First session: complete Plan 02 exchange and persist state =====
    let alice_state_bytes_v1: Vec<u8>;
    let bob_state_bytes_v1: Vec<u8>;
    let alice_group_id: Vec<u8>;
    let bob_group_id: Vec<u8>;

    {
        let alice_db = Database::open_encrypted(&alice_db_path, &alice_key).unwrap();
        alice_db.migrate().unwrap();
        let bob_db = Database::open_encrypted(&bob_db_path, &bob_key).unwrap();
        bob_db.migrate().unwrap();

        // Add each other as contacts.
        let alice_fp = Fingerprint::of(&alice_id.ghost_id()).to_string();
        let bob_fp = Fingerprint::of(&bob_id.ghost_id()).to_string();
        alice_db.contacts().insert(&Contact {
            ghost_id: bob_id.ghost_id(),
            display_name: Some("Bob".into()),
            local_alias: None,
            fingerprint: bob_fp.clone(),
            added_at: 1700000000,
            last_endpoint: None,
            verification: Verification::Unverified,
            notes: None,
            blocked: false,
        }).unwrap();
        bob_db.contacts().insert(&Contact {
            ghost_id: alice_id.ghost_id(),
            display_name: Some("Alice".into()),
            local_alias: None,
            fingerprint: alice_fp.clone(),
            added_at: 1700000000,
            last_endpoint: None,
            verification: Verification::Unverified,
            notes: None,
            blocked: false,
        }).unwrap();

        // MLS providers + KeyPackages
        let alice_provider = new_provider();
        let bob_provider = new_provider();
        populate_initial_keypackages(&mut alice_id, &alice_provider, 2).unwrap();
        populate_initial_keypackages(&mut bob_id, &bob_provider, 2).unwrap();

        // Alice fetches Bob's KP.
        let bob_kp_bytes = bob_id.mls_keypackages.first().unwrap().clone();
        let bob_kp_in = KeyPackageIn::tls_deserialize(&mut bob_kp_bytes.as_slice()).unwrap();
        let bob_kp = bob_kp_in
            .validate(alice_provider.crypto(), openmls::versions::ProtocolVersion::Mls10)
            .expect("validate bob KP");

        // Alice creates group + invites Bob.
        let mut alice_session = MlsSession::create(&alice_provider, &alice_id.identity_key, &alice_id.device_key).unwrap();
        let alice_signer = MlsSession::signer_from_dk(&alice_id.device_key);
        let invite = alice_session.add_member(&alice_provider, &alice_signer, bob_kp).unwrap();

        let welcome_bytes = invite.welcome.tls_serialize_detached().unwrap();
        let welcome_in = MlsMessageIn::tls_deserialize(&mut welcome_bytes.as_slice()).unwrap();
        let mut bob_session = MlsSession::join_via_welcome(&bob_provider, welcome_in).unwrap();

        // Round 1 exchange: alice -> bob, bob -> alice.
        let mls_ct = alice_session.encrypt_app_message(&alice_provider, &alice_signer, b"hello bob").unwrap();
        let bob_delivery = delivery_public(&bob_id.identity_key);
        let wire = wrap_message(
            &alice_id.identity_key, &alice_id.device_key, bob_id.ghost_id(),
            &bob_delivery, MsgType::AppMessage, PayloadType::AppText,
            mls_ct, 1700000060,
        ).unwrap();
        let alice_dk_pub = alice_id.device_key.public();
        let unwrapped = unwrap_message(&wire, &bob_id.identity_key, |id|
            (id == &alice_id.ghost_id()).then_some(alice_dk_pub)).unwrap();
        let plaintext = bob_session.decrypt_app_message(&bob_provider, &unwrapped.payload).unwrap();
        assert_eq!(plaintext, b"hello bob");

        // ===== Persist MLS state =====
        alice_state_bytes_v1 = alice_session.serialize_state().unwrap();
        bob_state_bytes_v1 = bob_session.serialize_state().unwrap();
        alice_group_id = alice_session.group_id_bytes();
        bob_group_id = bob_session.group_id_bytes();

        let now = 1700000100;
        alice_db.mls_groups().upsert(&MlsGroupRow {
            group_id: bytes_to_array_32(&alice_group_id),
            contact_id: bob_id.ghost_id(),
            state_blob: alice_state_bytes_v1.clone(),
            current_epoch: alice_session.current_epoch(),
            created_at: now,
            last_updated: now,
        }).unwrap();
        bob_db.mls_groups().upsert(&MlsGroupRow {
            group_id: bytes_to_array_32(&bob_group_id),
            contact_id: alice_id.ghost_id(),
            state_blob: bob_state_bytes_v1.clone(),
            current_epoch: bob_session.current_epoch(),
            created_at: now,
            last_updated: now,
        }).unwrap();

        // Drop everything (sessions, DBs, providers) — simulate process restart.
        drop(alice_session);
        drop(bob_session);
        drop(alice_provider);
        drop(bob_provider);
        drop(alice_db);
        drop(bob_db);
    }

    // ===== "Restart": reopen DBs, restore sessions, continue messaging =====
    let alice_db = Database::open_encrypted(&alice_db_path, &alice_key).unwrap();
    let bob_db = Database::open_encrypted(&bob_db_path, &bob_key).unwrap();

    let alice_state_loaded = alice_db.mls_groups().load_for_contact(&bob_id.ghost_id()).unwrap()
        .expect("alice state present");
    let bob_state_loaded = bob_db.mls_groups().load_for_contact(&alice_id.ghost_id()).unwrap()
        .expect("bob state present");
    assert_eq!(alice_state_loaded.state_blob, alice_state_bytes_v1);
    assert_eq!(bob_state_loaded.state_blob, bob_state_bytes_v1);

    // Need fresh providers after restart. (Plan 06 will explore whether the providers
    // also need to be persisted; for Plan 03 the test demonstrates state-only restore.)
    let alice_provider = new_provider();
    let bob_provider = new_provider();

    let mut alice_session = MlsSession::deserialize_state(&alice_provider, &alice_state_loaded.state_blob).unwrap();
    let mut bob_session = MlsSession::deserialize_state(&bob_provider, &bob_state_loaded.state_blob).unwrap();

    let alice_signer = MlsSession::signer_from_dk(&alice_id.device_key);

    // Round 2 exchange: alice (restored) -> bob (restored).
    let mls_ct = alice_session.encrypt_app_message(&alice_provider, &alice_signer, b"after restart").unwrap();
    let bob_delivery = delivery_public(&bob_id.identity_key);
    let wire = wrap_message(
        &alice_id.identity_key, &alice_id.device_key, bob_id.ghost_id(),
        &bob_delivery, MsgType::AppMessage, PayloadType::AppText,
        mls_ct, 1700000200,
    ).unwrap();
    let alice_dk_pub = alice_id.device_key.public();
    let unwrapped = unwrap_message(&wire, &bob_id.identity_key, |id|
        (id == &alice_id.ghost_id()).then_some(alice_dk_pub)).unwrap();
    let plaintext = bob_session.decrypt_app_message(&bob_provider, &unwrapped.payload).unwrap();
    assert_eq!(plaintext, b"after restart");
}

fn bytes_to_array_32(b: &[u8]) -> [u8; 32] {
    // openmls group IDs are arbitrary bytes; we hash to 32 if needed, or pad.
    // For Plan 03 simplicity, we BLAKE3-hash whatever the group ID is to a 32-byte key.
    *blake3::hash(b).as_bytes()
}
```

- [ ] **Step 2: Run the integration test**

```bash
cargo test -p ghost-storage --test e2e_persistence
```

Expected: `alice_and_bob_persist_and_continue_after_restart` passes.

If the MLS state restore step fails (likely places: `MlsSession::deserialize_state` due to openmls API mismatch, or signer resolution after restart), STOP and report DONE_WITH_CONCERNS with details. The deliverable is this test passing.

If the failure is specifically about openmls's signer state being lost after restart (because the SignatureKeyPair was registered with the original provider's storage), the fix is to **re-register** the signer in the new provider before deserializing. Add `let alice_signer = MlsSession::signer_from_dk(&alice_id.device_key); alice_signer.store(alice_provider.storage()).expect(...)` before the deserialize call. Document this in the test as a comment.

- [ ] **Step 3: Run the full test suite (no regressions)**

```bash
cargo test --workspace -- --test-threads=1
```

Expected: ghost-core 16, ghost-identity 40, ghost-protocol N (Plan 02's count + 1 from Task 11), ghost-storage M (~42 unit + 1 integration).

- [ ] **Step 4: Commit**

```bash
git add crates/ghost-storage/tests/
git commit -m "test(ghost-storage): end-to-end persistence test (alice/bob across restart)"
```

---

## Task 14: Final verification + tag plan-03-complete

**Files:** none (verification + tag only)

- [ ] **Step 1: Run the full battery**

```bash
cargo +1.87-x86_64-pc-windows-msvc fmt --all -- --check
cargo +1.87-x86_64-pc-windows-msvc clippy --all-targets --workspace -- -D warnings
cargo +1.87-x86_64-pc-windows-msvc test --workspace -- --test-threads=1
bash scripts/smoke-test-plan-01.sh
```

ALL FOUR must pass:
- `cargo fmt` clean
- `cargo clippy` zero warnings
- `cargo test --workspace` — sums to ~140+ tests including ghost-storage's ~42 unit + 1 integration
- Plan 01 smoke test still passes (Identity CLI lifecycle unchanged by Plan 03 — no schema bump)

If anything fails, STOP and report DONE_WITH_CONCERNS.

- [ ] **Step 2: Tag the milestone**

```bash
git tag -a plan-03-complete -m "Plan 03 (Storage) complete

Deliverable: ghost-storage crate providing SQLCipher-encrypted SQLite
with repository APIs for contacts, MLS group state, my KeyPackages,
messages, outbox, inbox dedup, and settings. Master DB key derived
from IdentityKey via HKDF — same IK unlocks identity.encrypted AND
ghost.db.

Validated end-to-end by integration test
'alice_and_bob_persist_and_continue_after_restart' which:
  1. Completes Plan 02 first-contact + bidirectional exchange.
  2. Persists each side's MLS group state to its encrypted DB.
  3. Drops all in-memory state (simulated process restart).
  4. Reopens DBs, restores MlsSessions from persisted state.
  5. Continues messaging — round-trips successfully across the restart.

Coverage: ~42 unit tests in ghost-storage + 1 integration test, plus
unchanged Plan 01/02 suites. cargo fmt and cargo clippy clean.

Notable choices:
  - rusqlite 0.32 with bundled-sqlcipher-vendored-openssl (no system
    deps, ~3-6min first build).
  - MLS state persisted as serialized blob (not full StorageProvider
    impl) — adequate for MVP-1 1-on-1 sessions; deferred to Plan 04+
    if granular persistence is needed.
  - Identity schema unchanged from Plan 02 v2 (no migration needed).

Next: Plan 04 — Network + Discovery (QUIC, libp2p DHT, NAT traversal,
custom TLS bound to GhostId)."
```

- [ ] **Step 3: Verify the tag**

```bash
git tag -l
git show plan-03-complete --stat | head -15
```

Expected: `plan-03-complete` listed; tag points to the most recent commit.

---

## Risks & Open Questions for Plan 03

| Risk | Mitigation |
|---|---|
| `rusqlite` + `bundled-sqlcipher-vendored-openssl` first build is 3-6 minutes | Documented; subsequent builds cached. Acceptable one-time cost. |
| openmls 0.8 may not expose direct MlsGroup serialization | Task 11 explicitly notes this; falls back to `MlsGroup::save/load` via provider if needed. Adjust Task 13 test accordingly. |
| MLS provider state (signature keys, init keys) is lost across restart in our blob-only approach | Task 13 comment documents that fresh providers are created; signers are re-registered from DK secret bytes (which DO persist via Identity). For 1-on-1 MVP-1 this is sufficient. Plan 06 will revisit if multi-device adds complexity. |
| SQLCipher KDF iter (256000) makes initial open ~50ms slower | Acceptable for desktop. Don't lower below 100k (security floor). |
| C compiler required to build rusqlite | Already required for openmls 0.8 build (Plan 02). No new dependency. |

## Self-Review Checklist (after writing this plan)

**1. Spec coverage** — every requirement in spec section 6 (Local storage) implemented:
- ✓ SQLite + SQLCipher encryption at rest (Task 4)
- ✓ Master DB key derived from IK via HKDF (Task 3)
- ✓ Schema with all 7 tables + schema_version + indexes (Task 5)
- ✓ Migrations runner with idempotency + downgrade detection (Task 5)
- ✓ ContactsRepo (Task 6)
- ✓ MlsGroupsRepo with state-blob persistence (Task 7)
- ✓ MyKeyPackagesRepo (Task 8)
- ✓ MessagesRepo with pagination + status update + purge (Task 9)
- ✓ OutboxRepo + InboxDedupRepo + SettingsRepo (Task 10)
- ✓ MLS state round-trip persistence (Tasks 11, 13)

Items NOT in Plan 03 (correctly deferred):
- FTS5 full-text search → MVP-2
- Storage layer for openmls's full StorageProvider trait → defer; current blob approach is sufficient
- Background task for inbox_dedup cleanup → ghost-client (Plan 06) orchestration
- DB integration into Identity::create CLI flow → ghost-client (Plan 06)

**2. Placeholder scan** — no "TBD" / "TODO". Phrases like "Implementer:" mark openmls-API consultation hints, not unfilled requirements.

**3. Type consistency** — `Database::open_encrypted` / `open_in_memory` / `migrate` / repository accessors (`db.contacts()`, `db.mls_groups()`, etc.) consistent. `derive_master_key`, `master_key_pragma`, `MASTER_KEY_LEN` consistent. `MlsSession::serialize_state` / `deserialize_state` / `group_id_bytes` / `current_epoch` consistent.

---

**Plan 03 complete and ready for execution.**
