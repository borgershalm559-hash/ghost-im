# Ghost Plan 09 — Sidebar Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Ghost v0.0.4 with the new V2 · Folder rail UI (persistent rail + chat list + main pane) replacing the current single-column home, plus Tier-1 backend additions (per-contact pin/mute/verified/retention/last_read_at + settings + scrubber task) and the auto-update path proven against installed v0.0.3.

**Architecture:** SvelteKit (Svelte 5 runes) consumes a Tauri IPC surface backed by a Rust workspace (`ghost-storage`, `ghost-client`, `ghost-app`). Migration 0003 extends the `contacts` table with four columns (`last_read_at`, `pinned`, `muted`, `retention_seconds`); new `ContactsRepo` setter methods are surface-level updates. `MessagesRepo::unread_count` is a single `COUNT(*)` query. A 60-second tokio interval task in `Client::open()` calls the existing `MessagesRepo::purge_expired`. New Tauri commands group into `commands/contact_actions.rs` and `commands/settings.rs`. The frontend gains 19 new Svelte components organized as Rail / ChatList / ChatPane / EmptyState / Modals + shared primitives (Avatar, SearchBar, Modal). Theme is CSS variables flipped via `data-theme` on the root element and persisted in the `settings` table.

**Tech Stack:** Rust 1.87 (workspace), `rusqlite` 0.32 with SQLCipher, `tokio` for the scrubber, Tauri 2 commands. Frontend: Svelte 5 (`$state` runes), SvelteKit 2, TypeScript 5, Vite 5, `@fontsource/inter`, `@fontsource/jetbrains-mono`.

**Reference spec:** [docs/superpowers/specs/2026-05-07-ghost-sidebar-design.md](../specs/2026-05-07-ghost-sidebar-design.md). Visual reference: design package files at `C:\Users\david\AppData\Local\Temp\design-pkg\ghost\project\components\` (`variants.jsx`, `sidebar-parts.jsx`, `chat-panes.jsx`, `theme.jsx`, `icons.jsx`, `avatar.jsx`, `data.jsx`).

---

## Phase 1 — Backend foundation (migration + storage layer)

### Task 1: Migration 0003 — add pinned / muted / retention / last_read_at columns

**Files:**
- Create: `crates/ghost-storage/migrations/0003_contacts_extras.sql`
- Modify: `crates/ghost-storage/src/migrations.rs`

- [ ] **Step 1.1: Create migration SQL file**

Create `crates/ghost-storage/migrations/0003_contacts_extras.sql`:

```sql
-- Per-contact UI state for the sidebar redesign:
--   last_read_at      — UNIX seconds; messages with received_at > this count as unread
--   pinned            — 0/1; pinned chats sort first in the list
--   muted             — 0/1; suppresses notifications (UI-only until OS notifications land)
--   retention_seconds — NULL = forever; otherwise expires_at on new messages = now + seconds
--
-- All columns get safe defaults so existing rows from migration 0001/0002 remain valid.

ALTER TABLE contacts ADD COLUMN last_read_at INTEGER NOT NULL DEFAULT 0;
ALTER TABLE contacts ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
ALTER TABLE contacts ADD COLUMN muted INTEGER NOT NULL DEFAULT 0;
ALTER TABLE contacts ADD COLUMN retention_seconds INTEGER;
```

- [ ] **Step 1.2: Register migration and bump APP_SCHEMA_VERSION**

Edit `crates/ghost-storage/src/migrations.rs`. Replace the `APP_SCHEMA_VERSION` line and `MIGRATIONS` array:

```rust
pub const APP_SCHEMA_VERSION: u32 = 3;

const MIGRATIONS: &[(u32, &str)] = &[
    (1, include_str!("../migrations/0001_init.sql")),
    (2, include_str!("../migrations/0002_add_contact_dk_pub.sql")),
    (3, include_str!("../migrations/0003_contacts_extras.sql")),
];
```

- [ ] **Step 1.3: Run existing migration tests to verify v3 reaches APP_SCHEMA_VERSION**

Run: `cargo test -p ghost-storage --lib migrations -- --test-threads=1`

Expected: `migrate_brings_fresh_db_to_app_version`, `migrate_is_idempotent`, `after_migrate_all_app_tables_exist`, `schema_too_new_returns_error` all PASS. The `after_migrate_all_app_tables_exist` test still works because the migration only adds columns, not tables.

- [ ] **Step 1.4: Commit**

```bash
git add crates/ghost-storage/migrations/0003_contacts_extras.sql crates/ghost-storage/src/migrations.rs
git commit -m "feat(storage): migration 0003 — pinned/muted/retention/last_read_at on contacts"
```

---

### Task 2: Extend `Contact` struct + INSERT/SELECT to include the new columns

**Files:**
- Modify: `crates/ghost-storage/src/repos/contacts.rs`

- [ ] **Step 2.1: Add new fields to `Contact` struct**

In `crates/ghost-storage/src/repos/contacts.rs`, replace the `Contact` struct (around line 26):

```rust
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
    pub dk_pub: Option<[u8; 32]>,
    /// UNIX seconds — messages with received_at > this are unread (default 0).
    pub last_read_at: i64,
    /// Pinned chats sort first in the UI list.
    pub pinned: bool,
    /// Mutes notifications (UI-only until OS notifications land).
    pub muted: bool,
    /// `None` = forever. Otherwise: new messages get `expires_at = now + seconds`.
    pub retention_seconds: Option<i64>,
}
```

- [ ] **Step 2.2: Update `insert` to include the new columns**

Replace the `insert` method body to use 14 columns:

```rust
pub fn insert(&self, contact: &Contact) -> Result<()> {
    self.db.with_tx(|tx| {
        tx.execute(
            "INSERT INTO contacts (
                ghost_id, display_name, local_alias, fingerprint, added_at,
                last_endpoint, verification, notes, blocked, dk_pub,
                last_read_at, pinned, muted, retention_seconds
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
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
                contact.last_read_at,
                contact.pinned as i64,
                contact.muted as i64,
                contact.retention_seconds,
            ],
        )?;
        Ok(())
    })
}
```

- [ ] **Step 2.3: Update `get` SELECT to include the new columns**

Replace the SELECT in `get`:

```rust
pub fn get(&self, id: &GhostId) -> Result<Option<Contact>> {
    self.db.with_conn(|c| {
        let mut stmt = c.prepare(
            "SELECT ghost_id, display_name, local_alias, fingerprint, added_at,
                    last_endpoint, verification, notes, blocked, dk_pub,
                    last_read_at, pinned, muted, retention_seconds
               FROM contacts WHERE ghost_id = ?1",
        )?;
        let mut rows = stmt.query(params![id.as_bytes()])?;
        match rows.next()? {
            Some(row) => Ok(Some(Self::row_to_contact(row)?)),
            None => Ok(None),
        }
    })
}
```

- [ ] **Step 2.4: Update `list` SELECT to include the new columns**

Replace the SELECT in `list`:

```rust
pub fn list(&self) -> Result<Vec<Contact>> {
    self.db.with_conn(|c| {
        let mut stmt = c.prepare(
            "SELECT ghost_id, display_name, local_alias, fingerprint, added_at,
                    last_endpoint, verification, notes, blocked, dk_pub,
                    last_read_at, pinned, muted, retention_seconds
               FROM contacts ORDER BY added_at ASC",
        )?;
        let rows = stmt
            .query_map([], |row| Ok(Self::row_to_contact(row)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter().collect()
    })
}
```

- [ ] **Step 2.5: Update `row_to_contact` to read the new columns**

Replace `row_to_contact`:

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
    let verification: i64 = row.get(6)?;
    let blocked: i64 = row.get(8)?;
    let dk_pub: Option<[u8; 32]> = match row.get::<_, Option<Vec<u8>>>(9)? {
        None => None,
        Some(bytes) => {
            if bytes.len() != 32 {
                return Err(StorageError::InvalidBlob {
                    table: "contacts",
                    column: "dk_pub",
                    detail: format!("expected 32 bytes, got {}", bytes.len()),
                });
            }
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            Some(arr)
        }
    };
    let pinned: i64 = row.get(11)?;
    let muted: i64 = row.get(12)?;
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
        last_read_at: row.get(10)?,
        pinned: pinned != 0,
        muted: muted != 0,
        retention_seconds: row.get(13)?,
    })
}
```

- [ ] **Step 2.6: Update `fake_contact` test helper to populate the new fields**

In the `tests` module of the same file, replace `fake_contact`:

```rust
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
        dk_pub: None,
        last_read_at: 0,
        pinned: false,
        muted: false,
        retention_seconds: None,
    }
}
```

Also update any other test that constructs a `Contact` literal in `crates/ghost-storage/`. Run `cargo build -p ghost-storage --tests` and fix every "missing field" compile error by adding the four new defaults (`last_read_at: 0, pinned: false, muted: false, retention_seconds: None`).

- [ ] **Step 2.7: Run all storage tests**

Run: `cargo test -p ghost-storage -- --test-threads=1`

Expected: every test passes.

- [ ] **Step 2.8: Commit**

```bash
git add crates/ghost-storage/src/repos/contacts.rs
git commit -m "feat(storage): extend Contact with last_read_at/pinned/muted/retention_seconds"
```

---

### Task 3: Add per-field setter methods on `ContactsRepo`

**Files:**
- Modify: `crates/ghost-storage/src/repos/contacts.rs`

- [ ] **Step 3.1: Write failing tests for the new methods**

In the `tests` module (bottom of `contacts.rs`), append:

```rust
#[test]
fn set_pinned_toggles_field() {
    let db = fresh_db();
    let c = fake_contact(20, "Pin");
    db.contacts().insert(&c).unwrap();
    db.contacts().set_pinned(&c.ghost_id, true).unwrap();
    assert!(db.contacts().get(&c.ghost_id).unwrap().unwrap().pinned);
    db.contacts().set_pinned(&c.ghost_id, false).unwrap();
    assert!(!db.contacts().get(&c.ghost_id).unwrap().unwrap().pinned);
}

#[test]
fn set_muted_toggles_field() {
    let db = fresh_db();
    let c = fake_contact(21, "Mute");
    db.contacts().insert(&c).unwrap();
    db.contacts().set_muted(&c.ghost_id, true).unwrap();
    assert!(db.contacts().get(&c.ghost_id).unwrap().unwrap().muted);
}

#[test]
fn set_verified_writes_correct_enum() {
    let db = fresh_db();
    let c = fake_contact(22, "Verify");
    db.contacts().insert(&c).unwrap();
    db.contacts().set_verified(&c.ghost_id, true).unwrap();
    assert_eq!(
        db.contacts().get(&c.ghost_id).unwrap().unwrap().verification,
        Verification::Verified
    );
    db.contacts().set_verified(&c.ghost_id, false).unwrap();
    assert_eq!(
        db.contacts().get(&c.ghost_id).unwrap().unwrap().verification,
        Verification::Unverified
    );
}

#[test]
fn set_retention_writes_seconds_or_null() {
    let db = fresh_db();
    let c = fake_contact(23, "Retain");
    db.contacts().insert(&c).unwrap();
    db.contacts().set_retention(&c.ghost_id, Some(86400)).unwrap();
    assert_eq!(
        db.contacts().get(&c.ghost_id).unwrap().unwrap().retention_seconds,
        Some(86400)
    );
    db.contacts().set_retention(&c.ghost_id, None).unwrap();
    assert_eq!(
        db.contacts().get(&c.ghost_id).unwrap().unwrap().retention_seconds,
        None
    );
}

#[test]
fn set_last_read_at_writes_value() {
    let db = fresh_db();
    let c = fake_contact(24, "Read");
    db.contacts().insert(&c).unwrap();
    db.contacts().set_last_read_at(&c.ghost_id, 1_700_000_000).unwrap();
    assert_eq!(
        db.contacts().get(&c.ghost_id).unwrap().unwrap().last_read_at,
        1_700_000_000
    );
}

#[test]
fn setter_on_missing_returns_not_found() {
    let db = fresh_db();
    let id = GhostId::from_bytes([99; 32]);
    let err = db.contacts().set_pinned(&id, true).unwrap_err();
    assert!(matches!(err, StorageError::NotFound(_)));
}
```

- [ ] **Step 3.2: Run failing tests**

Run: `cargo test -p ghost-storage --lib repos::contacts::tests::set_ -- --test-threads=1`

Expected: FAIL — `set_pinned`, `set_muted`, `set_verified`, `set_retention`, `set_last_read_at` not defined.

- [ ] **Step 3.3: Implement the setter methods**

In `impl<'a> ContactsRepo<'a>`, before the `fn row_to_contact` line, add:

```rust
pub fn set_pinned(&self, id: &GhostId, pinned: bool) -> Result<()> {
    self.exec_setter(
        "UPDATE contacts SET pinned = ?2 WHERE ghost_id = ?1",
        params![id.as_bytes(), pinned as i64],
        id,
    )
}

pub fn set_muted(&self, id: &GhostId, muted: bool) -> Result<()> {
    self.exec_setter(
        "UPDATE contacts SET muted = ?2 WHERE ghost_id = ?1",
        params![id.as_bytes(), muted as i64],
        id,
    )
}

pub fn set_verified(&self, id: &GhostId, verified: bool) -> Result<()> {
    let v = if verified { Verification::Verified } else { Verification::Unverified };
    self.exec_setter(
        "UPDATE contacts SET verification = ?2 WHERE ghost_id = ?1",
        params![id.as_bytes(), v as i64],
        id,
    )
}

pub fn set_retention(&self, id: &GhostId, seconds: Option<i64>) -> Result<()> {
    self.exec_setter(
        "UPDATE contacts SET retention_seconds = ?2 WHERE ghost_id = ?1",
        params![id.as_bytes(), seconds],
        id,
    )
}

pub fn set_last_read_at(&self, id: &GhostId, at: i64) -> Result<()> {
    self.exec_setter(
        "UPDATE contacts SET last_read_at = ?2 WHERE ghost_id = ?1",
        params![id.as_bytes(), at],
        id,
    )
}

fn exec_setter(
    &self,
    sql: &str,
    params: impl rusqlite::Params,
    id: &GhostId,
) -> Result<()> {
    self.db.with_tx(|tx| {
        let n = tx.execute(sql, params)?;
        if n == 0 {
            return Err(StorageError::NotFound(format!("contact {id}")));
        }
        Ok(())
    })
}
```

- [ ] **Step 3.4: Run tests pass**

Run: `cargo test -p ghost-storage --lib repos::contacts -- --test-threads=1`

Expected: PASS.

- [ ] **Step 3.5: Commit**

```bash
git add crates/ghost-storage/src/repos/contacts.rs
git commit -m "feat(storage): per-field setters on ContactsRepo (pin/mute/verify/retention/last_read_at)"
```

---

### Task 4: Add `unread_count` on `MessagesRepo`

**Files:**
- Modify: `crates/ghost-storage/src/repos/messages.rs`

- [ ] **Step 4.1: Write failing test**

In `tests` module of `messages.rs`, append:

```rust
#[test]
fn unread_count_returns_only_incoming_after_since() {
    let (db, contact) = fresh_db_with_contact(50);
    // 1 outgoing at received_at irrelevant
    db.messages().insert(&msg(1, contact, Direction::Outgoing, 100)).unwrap();
    // 2 incoming with received_at = 200, 300
    let mut a = msg(2, contact, Direction::Incoming, 200);
    a.received_at = Some(200);
    db.messages().insert(&a).unwrap();
    let mut b = msg(3, contact, Direction::Incoming, 300);
    b.received_at = Some(300);
    db.messages().insert(&b).unwrap();

    // since=199 → both incoming counted
    assert_eq!(db.messages().unread_count(&contact, 199).unwrap(), 2);
    // since=250 → only one
    assert_eq!(db.messages().unread_count(&contact, 250).unwrap(), 1);
    // since=999 → none
    assert_eq!(db.messages().unread_count(&contact, 999).unwrap(), 0);
}
```

- [ ] **Step 4.2: Run failing test**

Run: `cargo test -p ghost-storage --lib repos::messages::tests::unread_count -- --test-threads=1`

Expected: FAIL — `unread_count` not defined.

- [ ] **Step 4.3: Implement `unread_count`**

In `impl<'a> MessagesRepo<'a>`, after the `purge_expired` method, add:

```rust
/// Number of incoming messages for `contact` whose `received_at` is strictly
/// greater than `since`. Used by the UI to compute the unread badge.
pub fn unread_count(&self, contact: &GhostId, since: i64) -> Result<i64> {
    self.db.with_conn(|c| {
        let mut stmt = c.prepare(
            "SELECT COUNT(*) FROM messages
              WHERE contact_id = ?1
                AND direction = ?2
                AND received_at IS NOT NULL
                AND received_at > ?3",
        )?;
        let n: i64 = stmt.query_row(
            params![contact.as_bytes(), Direction::Incoming as i64, since],
            |r| r.get(0),
        )?;
        Ok(n)
    })
}
```

- [ ] **Step 4.4: Run test passes**

Run: `cargo test -p ghost-storage --lib repos::messages -- --test-threads=1`

Expected: PASS.

- [ ] **Step 4.5: Commit**

```bash
git add crates/ghost-storage/src/repos/messages.rs
git commit -m "feat(storage): MessagesRepo::unread_count for unread-badge computation"
```

---

## Phase 2 — Backend wiring (Client + Tauri commands)

### Task 5: Wire retention into `Client::send_message`

**Files:**
- Modify: `crates/ghost-client/src/client.rs`

- [ ] **Step 5.1: Update `send_message` to compute `expires_at` from contact retention**

In `crates/ghost-client/src/client.rs`, around line 444 there is `expires_at: None`. The fix uses the `contact` already loaded (line 396-399). Replace the `expires_at: None,` line on line ~444 with:

```rust
            expires_at: contact.retention_seconds.map(|s| now as i64 + s),
```

- [ ] **Step 5.2: Verify it compiles**

Run: `cargo build -p ghost-client`

Expected: success.

- [ ] **Step 5.3: Commit**

```bash
git add crates/ghost-client/src/client.rs
git commit -m "feat(client): send_message stamps expires_at from contact.retention_seconds"
```

---

### Task 6: Background scrubber task in `Client::open`

**Files:**
- Modify: `crates/ghost-client/src/client.rs`

- [ ] **Step 6.1: Spawn 60-second tokio interval that calls `purge_expired`**

In `crates/ghost-client/src/client.rs`, find `client.ensure_keypackages().await?;` (around line 119, end of `Client::open`). Just before that line, insert:

```rust
        // Disappearing-messages scrubber. Runs every 60s; each tick deletes any
        // message whose expires_at < now. Lives for the lifetime of the Client
        // (the JoinHandle is dropped when Client drops, which aborts it).
        {
            let db = client.db.clone();
            tokio::spawn(async move {
                let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
                loop {
                    tick.tick().await;
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);
                    if let Err(e) = db.messages().purge_expired(now) {
                        tracing::warn!(target: "ghost-client", "purge_expired failed: {e}");
                    }
                }
            });
        }
```

Apply the same insertion in `Client::open_with_in_memory_identity` (line ~178), just before `client.ensure_keypackages().await?;`.

- [ ] **Step 6.2: Add `tracing` import if missing at top of file**

If `crates/ghost-client/src/client.rs` doesn't already `use tracing::...`, the call uses fully-qualified `tracing::warn!`. Verify the crate's Cargo.toml has `tracing = { workspace = true }` — if not, add it under `[dependencies]`.

Run: `cargo build -p ghost-client`

If compile fails for missing `tracing`, edit `crates/ghost-client/Cargo.toml` to add:

```toml
tracing = { workspace = true }
```

- [ ] **Step 6.3: Commit**

```bash
git add crates/ghost-client/src/client.rs crates/ghost-client/Cargo.toml
git commit -m "feat(client): 60s background scrubber calls purge_expired"
```

---

### Task 7: Settings commands module (`get_setting` / `set_setting`)

**Files:**
- Create: `crates/ghost-app/src/commands/settings.rs`
- Modify: `crates/ghost-app/src/commands/mod.rs`
- Modify: `crates/ghost-client/src/client.rs` (expose settings access)

- [ ] **Step 7.1: Add settings accessor on `Client`**

In `crates/ghost-client/src/client.rs`, find `pub fn list_contacts` (around line 361). After that method, add:

```rust
/// Read a setting by key. Returns `None` if absent.
pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
    Ok(self.db.settings().get(key)?)
}

/// Write a setting (insert or update).
pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
    Ok(self.db.settings().set(key, value)?)
}
```

- [ ] **Step 7.2: Create `commands/settings.rs`**

Create `crates/ghost-app/src/commands/settings.rs`:

```rust
//! Generic settings get/set: simple key-value strings persisted in the
//! `settings` table. The keys we expect today: `theme` (`"dark"` | `"light"`),
//! `ghost_mode` (`"0"` | `"1"`).

use crate::error::CommandResult;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_setting(
    key: String,
    state: State<'_, AppState>,
) -> CommandResult<Option<String>> {
    let client = state.require_client().await?;
    Ok(client.get_setting(&key)?)
}

#[tauri::command]
pub async fn set_setting(
    key: String,
    value: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let client = state.require_client().await?;
    client.set_setting(&key, &value)?;
    Ok(())
}
```

- [ ] **Step 7.3: Register module in `commands/mod.rs`**

Edit `crates/ghost-app/src/commands/mod.rs` — append `pub mod settings;`:

```rust
//! Tauri command implementations.

pub mod identity;
pub mod lifecycle;
pub mod read;
pub mod settings;
pub mod updater;
pub mod write;
```

- [ ] **Step 7.4: Build to verify compilation**

Run: `cargo build -p ghost-app`

Expected: success.

- [ ] **Step 7.5: Commit**

```bash
git add crates/ghost-app/src/commands/settings.rs crates/ghost-app/src/commands/mod.rs crates/ghost-client/src/client.rs
git commit -m "feat(app): get_setting/set_setting Tauri commands + Client accessors"
```

---

### Task 8: Contact actions module (pin/mute/verify/retention/mark_read)

**Files:**
- Create: `crates/ghost-app/src/commands/contact_actions.rs`
- Modify: `crates/ghost-app/src/commands/mod.rs`
- Modify: `crates/ghost-client/src/client.rs` (expose contact-action wrappers)

- [ ] **Step 8.1: Add wrapper methods on `Client`**

In `crates/ghost-client/src/client.rs`, after the `set_setting` method added in Task 7, append:

```rust
pub fn set_pinned(&self, id: &ghost_core::GhostId, pinned: bool) -> Result<()> {
    Ok(self.db.contacts().set_pinned(id, pinned)?)
}

pub fn set_muted(&self, id: &ghost_core::GhostId, muted: bool) -> Result<()> {
    Ok(self.db.contacts().set_muted(id, muted)?)
}

pub fn set_verified(&self, id: &ghost_core::GhostId, verified: bool) -> Result<()> {
    Ok(self.db.contacts().set_verified(id, verified)?)
}

pub fn set_retention(&self, id: &ghost_core::GhostId, seconds: Option<i64>) -> Result<()> {
    Ok(self.db.contacts().set_retention(id, seconds)?)
}

pub fn mark_chat_read(&self, id: &ghost_core::GhostId) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Ok(self.db.contacts().set_last_read_at(id, now)?)
}
```

- [ ] **Step 8.2: Create `commands/contact_actions.rs`**

Create `crates/ghost-app/src/commands/contact_actions.rs`:

```rust
//! Per-contact action commands: pin, mute, verify, retention, mark-read.

use crate::error::{CommandError, CommandResult};
use crate::AppState;
use ghost_core::GhostId;
use tauri::State;

fn parse(s: &str) -> CommandResult<GhostId> {
    GhostId::from_bech32(s).map_err(|e| CommandError(format!("ghost id: {e}")))
}

#[tauri::command]
pub async fn set_pinned(
    contact_ghost_id: String,
    pinned: bool,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let client = state.require_client().await?;
    client.set_pinned(&parse(&contact_ghost_id)?, pinned)?;
    Ok(())
}

#[tauri::command]
pub async fn set_muted(
    contact_ghost_id: String,
    muted: bool,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let client = state.require_client().await?;
    client.set_muted(&parse(&contact_ghost_id)?, muted)?;
    Ok(())
}

#[tauri::command]
pub async fn set_verified(
    contact_ghost_id: String,
    verified: bool,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let client = state.require_client().await?;
    client.set_verified(&parse(&contact_ghost_id)?, verified)?;
    Ok(())
}

#[tauri::command]
pub async fn set_retention(
    contact_ghost_id: String,
    seconds: Option<i64>,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let client = state.require_client().await?;
    client.set_retention(&parse(&contact_ghost_id)?, seconds)?;
    Ok(())
}

#[tauri::command]
pub async fn mark_chat_read(
    contact_ghost_id: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let client = state.require_client().await?;
    client.mark_chat_read(&parse(&contact_ghost_id)?)?;
    Ok(())
}
```

- [ ] **Step 8.3: Register module**

In `crates/ghost-app/src/commands/mod.rs`, add `pub mod contact_actions;` (alphabetically):

```rust
//! Tauri command implementations.

pub mod contact_actions;
pub mod identity;
pub mod lifecycle;
pub mod read;
pub mod settings;
pub mod updater;
pub mod write;
```

- [ ] **Step 8.4: Build to verify**

Run: `cargo build -p ghost-app`

Expected: success.

- [ ] **Step 8.5: Commit**

```bash
git add crates/ghost-app/src/commands/contact_actions.rs crates/ghost-app/src/commands/mod.rs crates/ghost-client/src/client.rs
git commit -m "feat(app): contact_actions commands (pin/mute/verify/retention/mark_read)"
```

---

### Task 9: Extend `ContactDto` with new fields + last-message preview + unread

**Files:**
- Modify: `crates/ghost-app/src/dto.rs`
- Modify: `crates/ghost-app/src/commands/read.rs`
- Modify: `crates/ghost-client/src/client.rs` (add `list_messages_descending` helper)

- [ ] **Step 9.1: Extend `ContactDto`**

In `crates/ghost-app/src/dto.rs`, replace the `ContactDto` struct:

```rust
#[derive(Debug, Serialize)]
pub struct ContactDto {
    pub ghost_id: String,
    pub fingerprint: String,
    pub display_name: Option<String>,
    pub local_alias: Option<String>,
    pub added_at: i64,
    pub verified: bool,
    pub pinned: bool,
    pub muted: bool,
    pub retention_seconds: Option<i64>,

    /// Last message text (truncated server-side to 200 chars), `None` if no messages.
    pub last_message: Option<String>,
    /// `sent_at` of the last message.
    pub last_message_at: Option<i64>,
    /// `"in"` | `"out"` | `null`.
    pub last_message_direction: Option<String>,

    /// Count of incoming messages with `received_at > last_read_at`.
    pub unread_count: i64,
}
```

- [ ] **Step 9.2: Add `last_message_for_contact` helper on `Client`**

In `crates/ghost-client/src/client.rs`, after `pub fn list_messages` (around line 464), add:

```rust
/// Return the most recent message for a contact (highest `sent_at`), if any.
pub fn last_message_for_contact(
    &self,
    contact_id: &ghost_core::GhostId,
) -> Result<Option<MessageRow>> {
    // list_for_contact orders by sent_at ASC. Pull the tail by selecting all
    // and grabbing the last; for very long histories this is O(n) but the
    // sidebar refreshes only on inbound events / boot, not per render.
    let mut all = self.db.messages().list_for_contact(contact_id, 100_000, 0)?;
    Ok(all.pop())
}

/// Count unread messages for a contact (`received_at > contact.last_read_at`).
pub fn unread_count(&self, contact_id: &ghost_core::GhostId) -> Result<i64> {
    let last_read_at = self
        .db
        .contacts()
        .get(contact_id)?
        .map(|c| c.last_read_at)
        .unwrap_or(0);
    Ok(self.db.messages().unread_count(contact_id, last_read_at)?)
}
```

- [ ] **Step 9.3: Update `commands/read.rs::list_contacts` to populate new fields**

Replace the body of `list_contacts` in `crates/ghost-app/src/commands/read.rs`:

```rust
#[tauri::command]
pub async fn list_contacts(state: State<'_, AppState>) -> CommandResult<Vec<ContactDto>> {
    let client = state.require_client().await?;
    let rows = client.list_contacts()?;
    let mut out = Vec::with_capacity(rows.len());
    for c in rows {
        let last = client.last_message_for_contact(&c.ghost_id)?;
        let (last_message, last_message_at, last_message_direction) = match last {
            Some(m) => (
                Some(truncate(&m.content, 200)),
                Some(m.sent_at),
                Some(match m.direction {
                    ghost_storage::Direction::Incoming => "in".to_string(),
                    ghost_storage::Direction::Outgoing => "out".to_string(),
                }),
            ),
            None => (None, None, None),
        };
        let unread_count = client.unread_count(&c.ghost_id)?;
        out.push(ContactDto {
            ghost_id: c.ghost_id.to_string(),
            fingerprint: c.fingerprint,
            display_name: c.display_name,
            local_alias: c.local_alias,
            added_at: c.added_at,
            verified: matches!(c.verification, Verification::Verified),
            pinned: c.pinned,
            muted: c.muted,
            retention_seconds: c.retention_seconds,
            last_message,
            last_message_at,
            last_message_direction,
            unread_count,
        });
    }
    Ok(out)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max - 1).collect();
        t.push('…');
        t
    }
}
```

- [ ] **Step 9.4: Build**

Run: `cargo build -p ghost-app`

Expected: success.

- [ ] **Step 9.5: Commit**

```bash
git add crates/ghost-app/src/dto.rs crates/ghost-app/src/commands/read.rs crates/ghost-client/src/client.rs
git commit -m "feat(app): list_contacts returns last_message preview + unread_count"
```

---

### Task 10: Register new commands and update capabilities

**Files:**
- Modify: `apps/ghost-desktop/src/main.rs`
- Modify: `apps/ghost-desktop/capabilities/default.json`

- [ ] **Step 10.1: Register all new commands in `tauri::generate_handler!`**

In `apps/ghost-desktop/src/main.rs`, replace the `use ghost_app::commands::{...};` and `invoke_handler` block:

```rust
use ghost_app::commands::{
    contact_actions, identity, lifecycle, read, settings as settings_cmd, updater, write,
};
```

```rust
        .invoke_handler(tauri::generate_handler![
            identity::identity_status,
            identity::create_identity,
            lifecycle::open_client,
            lifecycle::close_client,
            read::client_info,
            read::list_contacts,
            read::list_messages,
            read::create_invite,
            updater::check_for_update,
            updater::download_and_install_update,
            write::add_contact,
            write::send_message,
            contact_actions::set_pinned,
            contact_actions::set_muted,
            contact_actions::set_verified,
            contact_actions::set_retention,
            contact_actions::mark_chat_read,
            settings_cmd::get_setting,
            settings_cmd::set_setting,
        ])
```

(Note: `settings` is renamed to `settings_cmd` because Tauri's `tauri::Manager::manage` namespace might collide; the `as` alias avoids any ambiguity.)

- [ ] **Step 10.2: Update capabilities/default.json**

Replace `apps/ghost-desktop/capabilities/default.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "identifier": "default",
  "description": "Allow main window to invoke ghost-app commands and receive inbox events.",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "core:event:default",
    "core:event:allow-listen",
    "core:event:allow-unlisten",
    "updater:default",
    "updater:allow-check",
    "updater:allow-download-and-install"
  ]
}
```

(The default `core:default` already grants `invoke` on app commands; updater has explicit grants. New commands inherit the invoke permission.)

- [ ] **Step 10.3: Build the desktop binary to verify**

Run: `cargo build -p ghost-desktop`

Expected: success.

- [ ] **Step 10.4: Commit**

```bash
git add apps/ghost-desktop/src/main.rs apps/ghost-desktop/capabilities/default.json
git commit -m "feat(desktop): register contact_actions + settings Tauri commands"
```

---

### Task 11: Add an end-to-end Rust test exercising the new path

**Files:**
- Modify: `crates/ghost-storage/tests/e2e_persistence.rs`

- [ ] **Step 11.1: Append a retention-roundtrip test**

In `crates/ghost-storage/tests/e2e_persistence.rs`, append at the end of the file:

```rust
#[test]
fn retention_purges_expired_messages() {
    use ghost_core::{Fingerprint, GhostId};
    use ghost_storage::{Contact, Direction, MessageRow, MessageStatus, Verification};
    use ghost_storage::{Database, derive_master_key};
    use ghost_identity::IdentityKey;

    let db = Database::open_in_memory(&derive_master_key(&IdentityKey::generate())).unwrap();
    db.migrate().unwrap();

    let id = GhostId::from_bytes([7u8; 32]);
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
            dk_pub: None,
            last_read_at: 0,
            pinned: false,
            muted: false,
            retention_seconds: Some(60),
        })
        .unwrap();

    let mut keep = MessageRow {
        msg_uuid: [1u8; 16],
        contact_id: id,
        direction: Direction::Outgoing,
        content_type: 0,
        content: "kept".into(),
        sent_at: 0,
        received_at: None,
        status: MessageStatus::Sent,
        reply_to: None,
        expires_at: Some(2_000_000_000),
    };
    let mut go = keep.clone();
    go.msg_uuid = [2u8; 16];
    go.content = "expired".into();
    go.expires_at = Some(1_000_000_000);
    db.messages().insert(&keep).unwrap();
    db.messages().insert(&go).unwrap();

    let purged = db.messages().purge_expired(1_500_000_000).unwrap();
    assert_eq!(purged, 1);
    let remaining = db.messages().list_for_contact(&id, 100, 0).unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].content, "kept");
}
```

If the existing tests in this file construct `Contact` literals, they too will fail to compile until you add the four new fields (`last_read_at: 0, pinned: false, muted: false, retention_seconds: None`).

- [ ] **Step 11.2: Run the e2e test**

Run: `cargo test -p ghost-storage --test e2e_persistence -- --test-threads=1`

Expected: all tests pass, including the new `retention_purges_expired_messages`.

- [ ] **Step 11.3: Commit**

```bash
git add crates/ghost-storage/tests/e2e_persistence.rs
git commit -m "test(storage): retention scrubber roundtrip e2e"
```

---

## Phase 3 — Frontend foundation (theme, icons, types, state)

### Task 12: Install Inter + JetBrains Mono via fontsource

**Files:**
- Modify: `frontend/package.json` (via pnpm)

- [ ] **Step 12.1: Add the font packages**

Run from repo root:

```bash
pnpm --dir frontend add @fontsource/inter @fontsource/jetbrains-mono
```

Expected: `frontend/package.json` gains the two new dependencies; `frontend/pnpm-lock.yaml` updates.

- [ ] **Step 12.2: Commit**

```bash
git add frontend/package.json frontend/pnpm-lock.yaml
git commit -m "feat(frontend): add Inter + JetBrains Mono via @fontsource"
```

---

### Task 13: Global CSS with theme tokens

**Files:**
- Create: `frontend/src/app.css`
- Modify: `frontend/src/routes/+layout.svelte` (import the CSS)

- [ ] **Step 13.1: Create `app.css`**

Create `frontend/src/app.css`:

```css
/* Font imports via @fontsource. */
@import '@fontsource/inter/400.css';
@import '@fontsource/inter/500.css';
@import '@fontsource/inter/600.css';
@import '@fontsource/inter/700.css';
@import '@fontsource/jetbrains-mono/400.css';
@import '@fontsource/jetbrains-mono/500.css';

/* Theme tokens — ported 1:1 from the design package's theme.jsx. */
:root,
:root[data-theme='dark'] {
  --bg: #0a0a10;
  --surface: #101019;
  --elevated: #171722;
  --sidebar: #0d0d14;
  --rail: #080810;
  --border: rgba(255, 255, 255, 0.06);
  --border-strong: rgba(255, 255, 255, 0.10);
  --text: #e8e8f0;
  --text-dim: rgba(232, 232, 240, 0.62);
  --text-muted: rgba(232, 232, 240, 0.40);
  --accent: #9b8cff;
  --accent-dim: rgba(155, 140, 255, 0.14);
  --accent-soft: rgba(155, 140, 255, 0.22);
  --success: #3ddc97;
  --danger: #ff6b7a;
  --bubble: #1a1a26;
  --bubble-mine: linear-gradient(135deg, #6c5ce7 0%, #9b8cff 100%);
  --hover: rgba(255, 255, 255, 0.04);
  --selected: rgba(155, 140, 255, 0.10);
}

:root[data-theme='light'] {
  --bg: #f7f6f3;
  --surface: #ffffff;
  --elevated: #ffffff;
  --sidebar: #f1efeb;
  --rail: #ebe9e4;
  --border: rgba(0, 0, 0, 0.06);
  --border-strong: rgba(0, 0, 0, 0.10);
  --text: #1a1a24;
  --text-dim: rgba(26, 26, 36, 0.62);
  --text-muted: rgba(26, 26, 36, 0.40);
  --accent: #6c5ce7;
  --accent-dim: rgba(108, 92, 231, 0.10);
  --accent-soft: rgba(108, 92, 231, 0.18);
  --success: #1a9968;
  --danger: #d83a4a;
  --bubble: #ffffff;
  --bubble-mine: linear-gradient(135deg, #6c5ce7 0%, #9b8cff 100%);
  --hover: rgba(0, 0, 0, 0.04);
  --selected: rgba(108, 92, 231, 0.08);
}

html,
body {
  margin: 0;
  padding: 0;
  height: 100%;
  background: var(--bg);
  color: var(--text);
  font-family:
    'Inter',
    -apple-system,
    BlinkMacSystemFont,
    'Segoe UI',
    sans-serif;
  font-size: 14px;
  letter-spacing: -0.05px;
}

* {
  box-sizing: border-box;
}

::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}
::-webkit-scrollbar-thumb {
  background: var(--border-strong);
  border-radius: 4px;
}
::-webkit-scrollbar-track {
  background: transparent;
}

button {
  font: inherit;
}
```

- [ ] **Step 13.2: Import `app.css` from layout**

Edit `frontend/src/routes/+layout.svelte` — replace the entire file content with:

```svelte
<script lang="ts">
  import '../app.css';
  import UpdateBanner from '$lib/components/UpdateBanner.svelte';

  let { children } = $props();
</script>

<UpdateBanner />
{@render children()}
```

(`ShellLayout` is wired in Task 38; for now layout is minimal.)

- [ ] **Step 13.3: Smoke build**

Run: `pnpm --dir frontend build`

Expected: success. Output mentions `app-*.css` bundle.

- [ ] **Step 13.4: Commit**

```bash
git add frontend/src/app.css frontend/src/routes/+layout.svelte
git commit -m "feat(frontend): theme tokens + font imports"
```

---

### Task 14: `lib/theme.ts` — apply/persist theme

**Files:**
- Create: `frontend/src/lib/theme.ts`

- [ ] **Step 14.1: Create `theme.ts`**

Create `frontend/src/lib/theme.ts`:

```ts
import { getSetting, setSetting } from './tauri';

export type Theme = 'dark' | 'light';

const KEY = 'theme';

/** Apply theme by writing `data-theme` to <html>. Pure DOM, no I/O. */
export function applyTheme(t: Theme) {
  document.documentElement.dataset.theme = t;
}

/** Load theme from settings (default `dark`) and apply. Idempotent. */
export async function bootTheme(): Promise<Theme> {
  const stored = (await getSetting(KEY)) as Theme | null;
  const t: Theme = stored === 'light' ? 'light' : 'dark';
  applyTheme(t);
  return t;
}

/** Set theme: persist + apply. */
export async function persistTheme(t: Theme): Promise<void> {
  await setSetting(KEY, t);
  applyTheme(t);
}
```

- [ ] **Step 14.2: Commit**

```bash
git add frontend/src/lib/theme.ts
git commit -m "feat(frontend): theme apply/persist module"
```

(Note: `getSetting`/`setSetting` are added in Task 17. Build will fail until then. That's OK — the commit chain forms a TDD sequence; this file just declares the dep.)

---

### Task 15: `lib/icons.ts` — SVG icon set

**Files:**
- Create: `frontend/src/lib/icons.ts`

- [ ] **Step 15.1: Create `icons.ts` with the icons used by the design**

Create `frontend/src/lib/icons.ts`:

```ts
/**
 * SVG path data for icons used across the sidebar redesign.
 * Each icon is a `d` string (or array of strings for multi-path) plus default
 * stroke-width. Render with `<svg>` element in the consuming Svelte component.
 *
 * Stroke-based, currentColor, viewBox 0 0 24 24.
 */

export interface IconDef {
  /** Either a single `d` (one <path>) or array of `d` strings (multiple paths). */
  d: string | string[];
  /** Default stroke-width (caller can override). */
  sw: number;
  /** If `true`, render with fill=currentColor instead of stroke. */
  fill?: boolean;
}

export const I: Record<string, IconDef> = {
  search: { d: 'M11 19a8 8 0 1 1 0-16 8 8 0 0 1 0 16ZM21 21l-4.3-4.3', sw: 1.6 },
  plus: { d: 'M12 5v14M5 12h14', sw: 1.6 },
  settings: {
    d: 'M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3h.1a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8v.1a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1Z',
    sw: 1.6,
  },
  pin: { d: 'M12 17v5 M9 3h6l-1 5 3 3v2H7v-2l3-3-1-5Z', sw: 1.6 },
  archive: { d: 'M3 7h18 M5 7v12a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V7 M9 12h6 M3 4h18v3H3z', sw: 1.6 },
  shield: { d: 'M12 3 4 6v6c0 5 3.5 8 8 9 4.5-1 8-4 8-9V6l-8-3Z', sw: 1.6 },
  lock: { d: 'M5 11h14v10H5z M8 11V8a4 4 0 0 1 8 0v3', sw: 1.6 },
  send: { d: 'M22 2 11 13 M22 2l-7 20-4-9-9-4 20-7Z', sw: 1.6 },
  mic: { d: 'M12 2a3 3 0 0 0-3 3v6a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z M19 10a7 7 0 0 1-14 0 M12 17v4 M8 21h8', sw: 1.6 },
  paperclip: { d: 'M21 11.5 12.5 20a5.5 5.5 0 0 1-7.8-7.8l8.5-8.5a3.7 3.7 0 0 1 5.2 5.2L9.9 17.4a1.8 1.8 0 0 1-2.6-2.6l7.4-7.4', sw: 1.6 },
  hash: { d: 'M4 9h16 M4 15h16 M10 3 8 21 M16 3l-2 18', sw: 1.6 },
  bell: { d: 'M6 8a6 6 0 1 1 12 0c0 7 3 9 3 9H3s3-2 3-9 M10 21a2 2 0 0 0 4 0', sw: 1.6 },
  bellOff: { d: 'M13.7 21a2 2 0 0 1-3.4 0 M18 8a6 6 0 0 0-9.3-5 M6 8c0 7-3 9-3 9h12 M2 2l20 20', sw: 1.6 },
  ghost: { d: 'M12 2a8 8 0 0 0-8 8v11l3-2 3 2 2-2 2 2 3-2 3 2V10a8 8 0 0 0-8-8Z M9 11h.01 M15 11h.01', sw: 1.6 },
  check: { d: 'M20 6 9 17l-5-5', sw: 1.6 },
  checkDouble: { d: 'M2 12l5 5L18 6 M9 17l1.5 1.5L22 7', sw: 1.6 },
  clock: { d: 'M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20Z M12 6v6l4 2', sw: 1.6 },
  fire: { d: 'M14 3c0 4-4 5-4 9a4 4 0 0 0 8 0c0-2-1-3-2-4 0 0 1 5-2 5 0-3-3-4 0-10Z M6 14a4 4 0 0 0 4 6', sw: 1.6 },
  user: { d: 'M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2 M12 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z', sw: 1.6 },
  users: { d: 'M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2 M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z M23 21v-2a4 4 0 0 0-3-3.9 M16 3.1a4 4 0 0 1 0 7.8', sw: 1.6 },
  chevDown: { d: 'm6 9 6 6 6-6', sw: 1.6 },
  chevRight: { d: 'm9 6 6 6-6 6', sw: 1.6 },
  inbox: { d: 'M22 12h-6l-2 3h-4l-2-3H2 M5.5 5h13L22 12v6a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2v-6L5.5 5Z', sw: 1.6 },
  star: { d: 'm12 2 3.1 6.3 6.9 1-5 4.9 1.2 6.8L12 17.8 5.8 21 7 14.2 2 9.3l6.9-1Z', sw: 1.6 },
  key: { d: 'm21 2-2 2m-7.6 7.6a5.5 5.5 0 1 1-7.8 7.8 5.5 5.5 0 0 1 7.8-7.8Zm0 0L15 8m0 0 4 4m-4-4 3-3', sw: 1.6 },
  smile: { d: 'M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20Z M8 14s1.5 2 4 2 4-2 4-2 M9 9h.01 M15 9h.01', sw: 1.6 },
  more: { d: 'M12 13a1 1 0 1 0 0-2 1 1 0 0 0 0 2Z M19 13a1 1 0 1 0 0-2 1 1 0 0 0 0 2Z M5 13a1 1 0 1 0 0-2 1 1 0 0 0 0 2Z', sw: 1.6, fill: true },
  edit: { d: 'M12 20h9 M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z', sw: 1.6 },
  filter: { d: 'M22 3H2l8 9.5V19l4 2v-8.5L22 3Z', sw: 1.6 },
};
```

- [ ] **Step 15.2: Commit**

```bash
git add frontend/src/lib/icons.ts
git commit -m "feat(frontend): SVG icon set ported from design package"
```

---

### Task 16: Extend `lib/types.ts` with the new fields

**Files:**
- Modify: `frontend/src/lib/types.ts`

- [ ] **Step 16.1: Replace `ContactDto` and add the small auxiliary types**

Replace the entire `frontend/src/lib/types.ts`:

```ts
export interface IdentityStatusDto {
  exists: boolean;
  client_open: boolean;
}

export interface CreatedIdentityDto {
  ghost_id: string;
  fingerprint: string;
  display_name: string | null;
}

export interface ClientInfoDto {
  ghost_id: string;
  fingerprint: string;
  display_name: string | null;
  local_addrs: string[];
}

export interface ContactDto {
  ghost_id: string;
  fingerprint: string;
  display_name: string | null;
  local_alias: string | null;
  added_at: number;
  verified: boolean;
  pinned: boolean;
  muted: boolean;
  retention_seconds: number | null;
  last_message: string | null;
  last_message_at: number | null;
  last_message_direction: 'in' | 'out' | null;
  unread_count: number;
}

export interface MessageDto {
  uuid: string;
  direction: 'in' | 'out';
  content: string;
  sent_at: number;
  received_at: number | null;
}

export interface UpdateAvailableDto {
  version: string;
  notes: string | null;
  release_date: string | null;
}

export interface InviteDto {
  bech32: string;
  expires_at: number;
}

export interface InboundMessageEvent {
  from_ghost_id: string;
  content: string;
  received_at: number;
}

/** Retention preset values (seconds) shown in the dropdown. `null` = forever. */
export const RETENTION_PRESETS: { label: string; seconds: number | null }[] = [
  { label: 'Хранить всегда', seconds: null },
  { label: '30 дней', seconds: 30 * 24 * 3600 },
  { label: '7 дней', seconds: 7 * 24 * 3600 },
  { label: '24 часа', seconds: 24 * 3600 },
  { label: '1 час', seconds: 3600 },
  { label: '5 минут', seconds: 5 * 60 },
];
```

- [ ] **Step 16.2: Run typecheck**

Run: `pnpm --dir frontend check`

Expected: errors will appear in components that consume `ContactDto` (e.g., the old `ContactList.svelte`). These will be fixed in later tasks (those components are deleted/replaced).

- [ ] **Step 16.3: Commit**

```bash
git add frontend/src/lib/types.ts
git commit -m "feat(frontend): extend ContactDto with sidebar-redesign fields"
```

---

### Task 17: Extend `lib/tauri.ts` with new command wrappers

**Files:**
- Modify: `frontend/src/lib/tauri.ts`

- [ ] **Step 17.1: Append wrappers for new commands**

At the end of `frontend/src/lib/tauri.ts`, append:

```ts
// ─── Per-contact actions (Task 8) ───────────────────────────────────────────

export async function setPinned(contact_ghost_id: string, pinned: boolean): Promise<void> {
  return invoke('set_pinned', { contactGhostId: contact_ghost_id, pinned });
}

export async function setMuted(contact_ghost_id: string, muted: boolean): Promise<void> {
  return invoke('set_muted', { contactGhostId: contact_ghost_id, muted });
}

export async function setVerified(contact_ghost_id: string, verified: boolean): Promise<void> {
  return invoke('set_verified', { contactGhostId: contact_ghost_id, verified });
}

export async function setRetention(
  contact_ghost_id: string,
  seconds: number | null
): Promise<void> {
  return invoke('set_retention', { contactGhostId: contact_ghost_id, seconds });
}

export async function markChatRead(contact_ghost_id: string): Promise<void> {
  return invoke('mark_chat_read', { contactGhostId: contact_ghost_id });
}

// ─── Settings (Task 7) ──────────────────────────────────────────────────────

export async function getSetting(key: string): Promise<string | null> {
  return invoke('get_setting', { key });
}

export async function setSetting(key: string, value: string): Promise<void> {
  return invoke('set_setting', { key, value });
}
```

- [ ] **Step 17.2: Verify with `pnpm --dir frontend check`**

Run: `pnpm --dir frontend check`

The errors should now only be in components that haven't been replaced yet (`ContactList.svelte`, `+page.svelte`). Backend type imports should resolve.

- [ ] **Step 17.3: Commit**

```bash
git add frontend/src/lib/tauri.ts
git commit -m "feat(frontend): tauri.ts wrappers for contact_actions + settings"
```

---

### Task 18: Extend `lib/state.svelte.ts` with theme + ghost mode + selection

**Files:**
- Modify: `frontend/src/lib/state.svelte.ts`

- [ ] **Step 18.1: Replace the store with the extended version**

Replace `frontend/src/lib/state.svelte.ts`:

```ts
import type { ClientInfoDto, ContactDto, MessageDto } from './types';
import type { Theme } from './theme';

class AppStore {
  info = $state<ClientInfoDto | null>(null);
  contacts = $state<ContactDto[]>([]);
  threads = $state<Record<string, MessageDto[]>>({});

  /** Currently visible theme; bootTheme() sets it on app start. */
  theme = $state<Theme>('dark');
  /** Persisted ghost-mode flag (visual-only in MVP-1). */
  ghostMode = $state(false);

  /** Sidebar search filter (local; not persisted). */
  searchQuery = $state('');

  setInfo(info: ClientInfoDto | null) {
    this.info = info;
  }

  setContacts(list: ContactDto[]) {
    this.contacts = list;
  }

  /** Replace a single contact's row (after pin/mute/verify/retention edits). */
  patchContact(ghost_id: string, patch: Partial<ContactDto>) {
    this.contacts = this.contacts.map((c) =>
      c.ghost_id === ghost_id ? { ...c, ...patch } : c
    );
  }

  setThread(ghost_id: string, msgs: MessageDto[]) {
    this.threads = { ...this.threads, [ghost_id]: msgs };
  }

  pushIncoming(ghost_id: string, msg: MessageDto) {
    const existing = this.threads[ghost_id] ?? [];
    this.threads = { ...this.threads, [ghost_id]: [...existing, msg] };
  }

  setTheme(t: Theme) {
    this.theme = t;
  }

  setGhostMode(on: boolean) {
    this.ghostMode = on;
  }

  setSearchQuery(q: string) {
    this.searchQuery = q;
  }
}

export const store = new AppStore();
```

- [ ] **Step 18.2: Commit**

```bash
git add frontend/src/lib/state.svelte.ts
git commit -m "feat(frontend): store gains theme/ghostMode/searchQuery + patchContact"
```

---

## Phase 4 — Frontend primitives (Avatar, Modal, SearchBar, Icon)

### Task 19: `Icon.svelte` shared component

**Files:**
- Create: `frontend/src/lib/components/Icon.svelte`

- [ ] **Step 19.1: Create**

Create `frontend/src/lib/components/Icon.svelte`:

```svelte
<script lang="ts">
  import { I, type IconDef } from '$lib/icons';

  type Props = {
    /** Key from `$lib/icons`'s `I` table. */
    name: keyof typeof I;
    size?: number;
    sw?: number;
    color?: string;
  };

  let { name, size = 16, sw, color = 'currentColor' }: Props = $props();
  let def = $derived(I[name] as IconDef);
  let strokeWidth = $derived(sw ?? def.sw);
  let paths = $derived(Array.isArray(def.d) ? def.d : [def.d]);
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill={def.fill ? color : 'none'}
  stroke={def.fill ? 'none' : color}
  stroke-width={strokeWidth}
  stroke-linecap="round"
  stroke-linejoin="round"
>
  {#each paths as d}
    <path {d} />
  {/each}
</svg>
```

- [ ] **Step 19.2: Commit**

```bash
git add frontend/src/lib/components/Icon.svelte
git commit -m "feat(frontend): Icon.svelte primitive"
```

---

### Task 20: `Avatar.svelte`

**Files:**
- Create: `frontend/src/lib/components/Avatar.svelte`

- [ ] **Step 20.1: Create**

Create `frontend/src/lib/components/Avatar.svelte`:

```svelte
<script lang="ts">
  type Props = {
    /** Display name or fingerprint — used to derive initials and hue. */
    name: string;
    size?: number;
    online?: boolean;
    ghost?: boolean;
    square?: boolean;
  };

  let { name, size = 40, online = false, ghost = false, square = false }: Props = $props();

  let seed = $derived(name.charCodeAt(0) + (name.charCodeAt(1) || 0));
  let hue = $derived((seed * 37) % 360);
  let grad = $derived(
    `linear-gradient(135deg, oklch(0.65 0.18 ${hue}) 0%, oklch(0.55 0.20 ${(hue + 40) % 360}) 100%)`
  );
  let initials = $derived(
    name
      .split(' ')
      .map((s) => s[0] ?? '')
      .slice(0, 2)
      .join('')
      .toUpperCase() || '?'
  );
</script>

<div class="root" style:width="{size}px" style:height="{size}px">
  <div
    class="bubble"
    style:width="{size}px"
    style:height="{size}px"
    style:border-radius={square ? `${size * 0.28}px` : '50%'}
    style:background={grad}
    style:font-size="{size * 0.4}px"
    style:box-shadow={ghost
      ? `0 0 0 2px var(--bg), 0 0 0 3.5px var(--accent)`
      : 'none'}
  >
    {initials}
  </div>
  {#if online}
    <div
      class="dot online"
      style:width="{size * 0.28}px"
      style:height="{size * 0.28}px"
    ></div>
  {/if}
  {#if ghost}
    <div
      class="dot ghost"
      style:width="{size * 0.32}px"
      style:height="{size * 0.32}px"
    >
      <svg width={size * 0.18} height={size * 0.18} viewBox="0 0 24 24" fill="#fff">
        <path d="M12 2a8 8 0 0 0-8 8v11l3-2 3 2 2-2 2 2 3-2 3 2V10a8 8 0 0 0-8-8Z" />
      </svg>
    </div>
  {/if}
</div>

<style>
  .root {
    position: relative;
    flex-shrink: 0;
  }
  .bubble {
    color: #fff;
    font-weight: 600;
    display: flex;
    align-items: center;
    justify-content: center;
    letter-spacing: -0.3px;
  }
  .dot {
    position: absolute;
    border-radius: 50%;
  }
  .dot.online {
    bottom: 0;
    right: 0;
    background: var(--success);
    border: 2px solid var(--sidebar);
  }
  .dot.ghost {
    bottom: -1px;
    right: -1px;
    background: var(--accent);
    border: 2px solid var(--bg);
    display: flex;
    align-items: center;
    justify-content: center;
  }
</style>
```

- [ ] **Step 20.2: Commit**

```bash
git add frontend/src/lib/components/Avatar.svelte
git commit -m "feat(frontend): Avatar component (gradient initials)"
```

---

### Task 21: `Modal.svelte` generic backdrop+pane

**Files:**
- Create: `frontend/src/lib/components/Modal.svelte`

- [ ] **Step 21.1: Create**

Create `frontend/src/lib/components/Modal.svelte`:

```svelte
<script lang="ts">
  import type { Snippet } from 'svelte';

  type Props = {
    open: boolean;
    onClose: () => void;
    title: string;
    children: Snippet;
  };

  let { open, onClose, title, children }: Props = $props();

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose();
  }
</script>

<svelte:window onkeydown={onKey} />

{#if open}
  <div
    class="backdrop"
    role="dialog"
    aria-modal="true"
    aria-label={title}
    onclick={(e) => {
      if (e.currentTarget === e.target) onClose();
    }}
  >
    <div class="pane">
      <header>
        <h2>{title}</h2>
        <button type="button" class="close" onclick={onClose} aria-label="Закрыть">×</button>
      </header>
      <div class="body">
        {@render children()}
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    backdrop-filter: blur(4px);
  }
  .pane {
    background: var(--surface);
    border: 0.5px solid var(--border);
    border-radius: 14px;
    width: min(560px, 90vw);
    max-height: 80vh;
    overflow: auto;
    box-shadow: 0 24px 80px rgba(0, 0, 0, 0.5);
  }
  header {
    display: flex;
    align-items: center;
    padding: 14px 18px;
    border-bottom: 0.5px solid var(--border);
  }
  h2 {
    margin: 0;
    flex: 1;
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.2px;
    color: var(--text);
  }
  .close {
    width: 28px;
    height: 28px;
    border: none;
    background: transparent;
    color: var(--text-dim);
    cursor: pointer;
    font-size: 22px;
    line-height: 1;
    border-radius: 6px;
  }
  .close:hover {
    background: var(--hover);
    color: var(--text);
  }
  .body {
    padding: 18px;
    color: var(--text);
  }
</style>
```

- [ ] **Step 21.2: Commit**

```bash
git add frontend/src/lib/components/Modal.svelte
git commit -m "feat(frontend): Modal primitive (backdrop+pane+esc/click-out)"
```

---

### Task 22: `SearchBar.svelte`

**Files:**
- Create: `frontend/src/lib/components/SearchBar.svelte`

- [ ] **Step 22.1: Create**

Create `frontend/src/lib/components/SearchBar.svelte`:

```svelte
<script lang="ts">
  import Icon from './Icon.svelte';

  type Props = {
    value: string;
    placeholder?: string;
    onInput: (v: string) => void;
  };

  let { value, placeholder = 'Поиск · ⌘K', onInput }: Props = $props();
</script>

<div class="wrap">
  <div class="box">
    <span class="ic"><Icon name="search" size={14} color="var(--text-muted)" /></span>
    <input
      type="text"
      {value}
      {placeholder}
      oninput={(e) => onInput((e.currentTarget as HTMLInputElement).value)}
    />
  </div>
</div>

<style>
  .wrap {
    padding: 0 12px 10px;
  }
  .box {
    height: 36px;
    border-radius: 10px;
    background: var(--surface);
    border: 0.5px solid var(--border);
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
  }
  .ic {
    display: flex;
  }
  input {
    flex: 1;
    border: none;
    background: transparent;
    color: var(--text);
    font-size: 13px;
    outline: none;
  }
  input::placeholder {
    color: var(--text-muted);
  }
</style>
```

- [ ] **Step 22.2: Commit**

```bash
git add frontend/src/lib/components/SearchBar.svelte
git commit -m "feat(frontend): SearchBar primitive"
```

---

## Phase 5 — Frontend shell (Rail / ChatList / ChatRow / ContactMenu / ProfileFooter)

### Task 23: `Rail.svelte`

**Files:**
- Create: `frontend/src/lib/components/Rail.svelte`

- [ ] **Step 23.1: Create**

The rail has one functional item ("All") plus decorative folder labels. Click on decorative items does nothing (no toast — fake-feature dishonesty is forbidden in MVP-1).

Create `frontend/src/lib/components/Rail.svelte`:

```svelte
<script lang="ts">
  import Icon from './Icon.svelte';
  import Avatar from './Avatar.svelte';
  import { store } from '$lib/state.svelte';

  type Folder = {
    id: string;
    icon: 'inbox' | 'user' | 'users' | 'key' | 'hash' | 'ghost' | 'archive';
    label: string;
    /** When the only-functional flag, click is a no-op. */
    decorative?: boolean;
  };

  // First entry "All" is the only active/functional one in MVP-1.
  const FOLDERS: Folder[] = [
    { id: 'all', icon: 'inbox', label: 'Все' },
    { id: 'personal', icon: 'user', label: 'Личные', decorative: true },
    { id: 'work', icon: 'users', label: 'Работа', decorative: true },
    { id: 'crypto', icon: 'key', label: 'Crypto', decorative: true },
    { id: 'channels', icon: 'hash', label: 'Каналы', decorative: true },
    { id: 'burner', icon: 'ghost', label: 'Burner', decorative: true },
    { id: 'archive', icon: 'archive', label: 'Архив', decorative: true },
  ];

  type Props = {
    onProfileClick: () => void;
  };
  let { onProfileClick }: Props = $props();

  let avatarName = $derived(store.info?.fingerprint ?? '?');
</script>

<aside class="rail">
  {#each FOLDERS as f (f.id)}
    <button
      type="button"
      class="cell"
      class:active={f.id === 'all'}
      class:decorative={f.decorative}
      disabled={f.decorative}
    >
      <span class="ic">
        <Icon
          name={f.icon}
          size={22}
          sw={1.8}
          color={f.id === 'all' ? 'var(--accent)' : 'var(--text-dim)'}
        />
      </span>
      <span class="label">{f.label}</span>
    </button>
  {/each}

  <div class="spacer"></div>

  <button type="button" class="profile" onclick={onProfileClick} aria-label="Профиль">
    <Avatar name={avatarName} size={40} ghost={store.ghostMode} />
  </button>
</aside>

<style>
  .rail {
    width: 80px;
    background: var(--rail);
    border-right: 0.5px solid var(--border);
    display: flex;
    flex-direction: column;
    align-items: stretch;
    padding: 12px 6px;
    gap: 4px;
    flex-shrink: 0;
  }
  .cell {
    padding: 10px 6px;
    border-radius: 10px;
    background: transparent;
    border: 0;
    cursor: pointer;
    color: var(--text-dim);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    font: inherit;
  }
  .cell.active {
    background: var(--accent-dim);
    color: var(--accent);
  }
  .cell.decorative {
    cursor: default;
    opacity: 0.5;
  }
  .ic {
    display: flex;
  }
  .label {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.1px;
  }
  .spacer {
    flex: 1;
  }
  .profile {
    background: transparent;
    border: 0;
    padding: 0;
    cursor: pointer;
    align-self: center;
  }
</style>
```

- [ ] **Step 23.2: Commit**

```bash
git add frontend/src/lib/components/Rail.svelte
git commit -m "feat(frontend): Rail component (functional 'All' + decorative folders)"
```

---

### Task 24: `ProfilePopover.svelte`

**Files:**
- Create: `frontend/src/lib/components/ProfilePopover.svelte`

- [ ] **Step 24.1: Create**

Create `frontend/src/lib/components/ProfilePopover.svelte`:

```svelte
<script lang="ts">
  import { store } from '$lib/state.svelte';
  import { persistTheme } from '$lib/theme';
  import { setSetting } from '$lib/tauri';

  type Props = {
    open: boolean;
    onClose: () => void;
    onShowIdentity: () => void;
  };
  let { open, onClose, onShowIdentity }: Props = $props();

  async function pickTheme(t: 'dark' | 'light') {
    await persistTheme(t);
    store.setTheme(t);
  }

  async function toggleGhostMode() {
    const next = !store.ghostMode;
    store.setGhostMode(next);
    await setSetting('ghost_mode', next ? '1' : '0');
  }
</script>

{#if open}
  <div
    class="popover"
    role="dialog"
    aria-label="Настройки профиля"
    onmouseleave={onClose}
  >
    <div class="row">
      <div class="label">Тема</div>
      <div class="seg">
        <button
          type="button"
          class:active={store.theme === 'dark'}
          onclick={() => pickTheme('dark')}>Тёмная</button
        >
        <button
          type="button"
          class:active={store.theme === 'light'}
          onclick={() => pickTheme('light')}>Светлая</button
        >
      </div>
    </div>

    <div class="row">
      <div class="label">Ghost mode</div>
      <button type="button" class="toggle" class:on={store.ghostMode} onclick={toggleGhostMode}>
        <span class="knob"></span>
      </button>
    </div>

    <div class="divider"></div>

    <button type="button" class="action" onclick={onShowIdentity}>Показать мой Ghost ID</button>
  </div>
{/if}

<style>
  .popover {
    position: fixed;
    bottom: 76px;
    left: 88px;
    width: 240px;
    background: var(--elevated);
    border: 0.5px solid var(--border);
    border-radius: 12px;
    padding: 10px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
    z-index: 100;
    color: var(--text);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 6px;
  }
  .label {
    font-size: 12px;
    color: var(--text-dim);
    font-weight: 500;
  }
  .seg {
    display: flex;
    background: var(--surface);
    border-radius: 6px;
    padding: 2px;
    border: 0.5px solid var(--border);
  }
  .seg button {
    border: 0;
    background: transparent;
    color: var(--text-dim);
    padding: 4px 10px;
    border-radius: 4px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
  }
  .seg button.active {
    background: var(--accent-dim);
    color: var(--accent);
  }
  .toggle {
    width: 36px;
    height: 20px;
    border-radius: 999px;
    background: var(--border-strong);
    border: 0;
    position: relative;
    cursor: pointer;
  }
  .toggle.on {
    background: var(--accent);
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    transition: transform 0.15s;
  }
  .toggle.on .knob {
    transform: translateX(16px);
  }
  .divider {
    height: 0.5px;
    background: var(--border);
    margin: 6px 0;
  }
  .action {
    width: 100%;
    padding: 8px;
    border: 0;
    background: transparent;
    color: var(--text);
    text-align: left;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13px;
  }
  .action:hover {
    background: var(--hover);
  }
</style>
```

- [ ] **Step 24.2: Commit**

```bash
git add frontend/src/lib/components/ProfilePopover.svelte
git commit -m "feat(frontend): ProfilePopover (theme switch + ghost-mode toggle + show ID)"
```

---

### Task 25: `ChatRow.svelte`

**Files:**
- Create: `frontend/src/lib/components/ChatRow.svelte`

- [ ] **Step 25.1: Create**

Create `frontend/src/lib/components/ChatRow.svelte`:

```svelte
<script lang="ts">
  import Avatar from './Avatar.svelte';
  import Icon from './Icon.svelte';
  import type { ContactDto } from '$lib/types';

  type Props = {
    contact: ContactDto;
    selected: boolean;
    onClick: () => void;
    onContextMenu: (x: number, y: number) => void;
  };
  let { contact, selected, onClick, onContextMenu }: Props = $props();

  let displayName = $derived(
    contact.local_alias ?? contact.display_name ?? contact.fingerprint
  );
  let timeText = $derived(formatTime(contact.last_message_at));

  function formatTime(ts: number | null): string {
    if (ts == null) return '';
    const d = new Date(ts * 1000);
    const now = new Date();
    if (d.toDateString() === now.toDateString()) {
      return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    }
    const yesterday = new Date(now);
    yesterday.setDate(now.getDate() - 1);
    if (d.toDateString() === yesterday.toDateString()) return 'Вчера';
    const diff = (now.getTime() - d.getTime()) / 86_400_000;
    if (diff < 7) {
      return d.toLocaleDateString([], { weekday: 'short' });
    }
    return d.toLocaleDateString([], { day: '2-digit', month: '2-digit' });
  }
</script>

<button
  type="button"
  class="row"
  class:selected
  onclick={onClick}
  oncontextmenu={(e) => {
    e.preventDefault();
    onContextMenu(e.clientX, e.clientY);
  }}
>
  {#if selected}
    <span class="bar"></span>
  {/if}
  <Avatar name={displayName} size={36} />
  <div class="body">
    <div class="line1">
      <span class="lock"><Icon name="lock" size={11} sw={2} color="var(--success)" /></span>
      <span class="name">{displayName}</span>
      {#if contact.verified}
        <span class="badge"><Icon name="shield" size={12} sw={2} color="var(--accent)" /></span>
      {/if}
      {#if contact.muted}
        <span class="badge"><Icon name="bellOff" size={12} color="var(--text-muted)" /></span>
      {/if}
      {#if contact.pinned}
        <span class="badge"><Icon name="pin" size={11} sw={2} color="var(--text-muted)" /></span>
      {/if}
      <span class="time">{timeText}</span>
    </div>
    <div class="line2">
      <span class="last">
        {#if contact.last_message_direction === 'out'}<span class="me">Вы:&nbsp;</span>{/if}
        {contact.last_message ?? 'Нет сообщений'}
      </span>
      {#if contact.unread_count > 0}
        <span class="unread" class:muted={contact.muted}>
          {contact.unread_count > 99 ? '99+' : contact.unread_count}
        </span>
      {/if}
    </div>
  </div>
</button>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    border-radius: 10px;
    margin: 0 8px;
    background: transparent;
    border: 0;
    cursor: pointer;
    text-align: left;
    color: var(--text);
    font: inherit;
    width: calc(100% - 16px);
    position: relative;
  }
  .row:hover {
    background: var(--hover);
  }
  .row.selected {
    background: var(--selected);
  }
  .bar {
    position: absolute;
    left: 0;
    top: 8px;
    bottom: 8px;
    width: 3px;
    border-radius: 2px;
    background: var(--accent);
  }
  .body {
    flex: 1;
    min-width: 0;
  }
  .line1 {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 2px;
  }
  .name {
    font-size: 14px;
    font-weight: 600;
    letter-spacing: -0.1px;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .lock,
  .badge {
    display: flex;
  }
  .time {
    font-size: 12px;
    color: var(--text-muted);
    flex-shrink: 0;
  }
  .line2 {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .last {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .me {
    color: var(--text-muted);
  }
  .unread {
    min-width: 20px;
    height: 20px;
    padding: 0 6px;
    border-radius: 10px;
    background: var(--accent);
    color: #fff;
    font-size: 11px;
    font-weight: 700;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .unread.muted {
    background: var(--text-muted);
    color: var(--bg);
  }
</style>
```

- [ ] **Step 25.2: Commit**

```bash
git add frontend/src/lib/components/ChatRow.svelte
git commit -m "feat(frontend): ChatRow (avatar + name + indicators + last/unread)"
```

---

### Task 26: `ContactMenu.svelte`

**Files:**
- Create: `frontend/src/lib/components/ContactMenu.svelte`

- [ ] **Step 26.1: Create**

Create `frontend/src/lib/components/ContactMenu.svelte`:

```svelte
<script lang="ts">
  import {
    setPinned as cmdSetPinned,
    setMuted as cmdSetMuted,
    setVerified as cmdSetVerified,
    setRetention as cmdSetRetention,
    listContacts,
  } from '$lib/tauri';
  import { store } from '$lib/state.svelte';
  import { RETENTION_PRESETS, type ContactDto } from '$lib/types';

  type Props = {
    contact: ContactDto;
    x: number;
    y: number;
    onClose: () => void;
  };
  let { contact, x, y, onClose }: Props = $props();

  let busy = $state(false);

  async function refresh() {
    const cs = await listContacts();
    store.setContacts(cs);
  }

  async function togglePin() {
    busy = true;
    try {
      await cmdSetPinned(contact.ghost_id, !contact.pinned);
      await refresh();
      onClose();
    } finally {
      busy = false;
    }
  }
  async function toggleMute() {
    busy = true;
    try {
      await cmdSetMuted(contact.ghost_id, !contact.muted);
      await refresh();
      onClose();
    } finally {
      busy = false;
    }
  }
  async function toggleVerify() {
    busy = true;
    try {
      await cmdSetVerified(contact.ghost_id, !contact.verified);
      await refresh();
      onClose();
    } finally {
      busy = false;
    }
  }
  async function pickRetention(seconds: number | null) {
    busy = true;
    try {
      await cmdSetRetention(contact.ghost_id, seconds);
      await refresh();
      onClose();
    } finally {
      busy = false;
    }
  }
</script>

<svelte:window onclick={onClose} oncontextmenu={onClose} />

<div
  class="menu"
  style:left="{x}px"
  style:top="{y}px"
  role="menu"
  onclick={(e) => e.stopPropagation()}
  oncontextmenu={(e) => e.preventDefault()}
>
  <button type="button" disabled={busy} onclick={togglePin}>
    {contact.pinned ? 'Открепить' : 'Закрепить'}
  </button>
  <button type="button" disabled={busy} onclick={toggleMute}>
    {contact.muted ? 'Включить уведомления' : 'Выключить уведомления'}
  </button>
  <button type="button" disabled={busy} onclick={toggleVerify}>
    {contact.verified ? 'Снять отметку «проверен»' : 'Отметить как проверенного'}
  </button>
  <div class="divider"></div>
  <div class="label">Исчезающие сообщения</div>
  {#each RETENTION_PRESETS as p}
    <button
      type="button"
      class="preset"
      class:active={contact.retention_seconds === p.seconds}
      disabled={busy}
      onclick={() => pickRetention(p.seconds)}
    >
      {p.label}
    </button>
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 200;
    background: var(--elevated);
    border: 0.5px solid var(--border);
    border-radius: 10px;
    padding: 6px;
    min-width: 220px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5);
    color: var(--text);
  }
  .menu button {
    width: 100%;
    text-align: left;
    border: 0;
    background: transparent;
    color: var(--text);
    padding: 7px 10px;
    border-radius: 6px;
    font-size: 13px;
    cursor: pointer;
  }
  .menu button:hover {
    background: var(--hover);
  }
  .menu .preset.active {
    background: var(--accent-dim);
    color: var(--accent);
  }
  .divider {
    height: 0.5px;
    background: var(--border);
    margin: 4px 0;
  }
  .label {
    font-size: 10px;
    font-weight: 700;
    color: var(--text-muted);
    letter-spacing: 0.6px;
    text-transform: uppercase;
    padding: 6px 10px 2px;
  }
</style>
```

- [ ] **Step 26.2: Commit**

```bash
git add frontend/src/lib/components/ContactMenu.svelte
git commit -m "feat(frontend): ContactMenu (pin/mute/verify/retention popover)"
```

---

### Task 27: `ChatList.svelte`

**Files:**
- Create: `frontend/src/lib/components/ChatList.svelte`

- [ ] **Step 27.1: Create**

Create `frontend/src/lib/components/ChatList.svelte`:

```svelte
<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { store } from '$lib/state.svelte';
  import SearchBar from './SearchBar.svelte';
  import ChatRow from './ChatRow.svelte';
  import ContactMenu from './ContactMenu.svelte';
  import type { ContactDto } from '$lib/types';

  let menu = $state<{ contact: ContactDto; x: number; y: number } | null>(null);

  let selectedId = $derived(decodeURIComponent(page.params.ghost_id ?? ''));

  let filtered = $derived(
    store.contacts
      .filter((c) => {
        const q = store.searchQuery.trim().toLowerCase();
        if (q === '') return true;
        const name = (c.local_alias ?? c.display_name ?? '').toLowerCase();
        return (
          name.includes(q) ||
          c.fingerprint.toLowerCase().includes(q) ||
          c.ghost_id.toLowerCase().includes(q)
        );
      })
      .slice()
      .sort((a, b) => {
        if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
        const at = a.last_message_at ?? a.added_at;
        const bt = b.last_message_at ?? b.added_at;
        return bt - at;
      })
  );

  let pinned = $derived(filtered.filter((c) => c.pinned));
  let rest = $derived(filtered.filter((c) => !c.pinned));

  let totalUnread = $derived(store.contacts.reduce((s, c) => s + c.unread_count, 0));

  function open(c: ContactDto) {
    goto(`/chat/${encodeURIComponent(c.ghost_id)}`);
  }
</script>

<aside class="list">
  <header>
    <div>
      <div class="title">Все</div>
      <div class="meta">
        {store.contacts.length} {pluralize(store.contacts.length)}
        {#if totalUnread > 0} · {totalUnread} непрочит.{/if}
      </div>
    </div>
  </header>

  <SearchBar
    value={store.searchQuery}
    placeholder="Поиск чатов"
    onInput={(v) => store.setSearchQuery(v)}
  />

  <div class="scroll">
    {#if pinned.length > 0}
      <div class="section-label">Закреплённые</div>
      {#each pinned as c (c.ghost_id)}
        <ChatRow
          contact={c}
          selected={c.ghost_id === selectedId}
          onClick={() => open(c)}
          onContextMenu={(x, y) => (menu = { contact: c, x, y })}
        />
      {/each}
    {/if}

    {#if rest.length > 0}
      <div class="section-label">Все чаты</div>
      {#each rest as c (c.ghost_id)}
        <ChatRow
          contact={c}
          selected={c.ghost_id === selectedId}
          onClick={() => open(c)}
          onContextMenu={(x, y) => (menu = { contact: c, x, y })}
        />
      {/each}
    {/if}

    {#if store.contacts.length === 0}
      <div class="empty">Контактов пока нет.</div>
    {/if}
  </div>
</aside>

{#if menu}
  <ContactMenu
    contact={menu.contact}
    x={menu.x}
    y={menu.y}
    onClose={() => (menu = null)}
  />
{/if}

<script context="module" lang="ts">
  function pluralize(n: number): string {
    const last = n % 10;
    const tens = Math.floor((n % 100) / 10);
    if (tens === 1) return 'чатов';
    if (last === 1) return 'чат';
    if (last >= 2 && last <= 4) return 'чата';
    return 'чатов';
  }
</script>

<style>
  .list {
    width: 360px;
    background: var(--sidebar);
    border-right: 0.5px solid var(--border);
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
  }
  header {
    padding: 14px 14px 12px;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .title {
    font-size: 16px;
    font-weight: 700;
    color: var(--text);
    letter-spacing: -0.3px;
  }
  .meta {
    font-size: 11px;
    color: var(--text-dim);
    margin-top: 2px;
  }
  .scroll {
    flex: 1;
    overflow-y: auto;
  }
  .section-label {
    padding: 10px 18px 4px;
    font-size: 10px;
    font-weight: 700;
    color: var(--text-muted);
    letter-spacing: 0.6px;
    text-transform: uppercase;
  }
  .empty {
    padding: 40px 24px;
    text-align: center;
    color: var(--text-muted);
    font-size: 13px;
  }
</style>
```

- [ ] **Step 27.2: Commit**

```bash
git add frontend/src/lib/components/ChatList.svelte
git commit -m "feat(frontend): ChatList (search + pinned/all sections + sort)"
```

---

### Task 28: `ShellLayout.svelte`

**Files:**
- Create: `frontend/src/lib/components/ShellLayout.svelte`

- [ ] **Step 28.1: Create**

Create `frontend/src/lib/components/ShellLayout.svelte`:

```svelte
<script lang="ts">
  import type { Snippet } from 'svelte';
  import Rail from './Rail.svelte';
  import ChatList from './ChatList.svelte';
  import ProfilePopover from './ProfilePopover.svelte';
  import IdentityModal from './IdentityModal.svelte';

  type Props = {
    children: Snippet;
  };
  let { children }: Props = $props();

  let popoverOpen = $state(false);
  let identityOpen = $state(false);
</script>

<div class="shell">
  <Rail onProfileClick={() => (popoverOpen = !popoverOpen)} />
  <ChatList />
  <main class="main">{@render children()}</main>
</div>

<ProfilePopover
  open={popoverOpen}
  onClose={() => (popoverOpen = false)}
  onShowIdentity={() => {
    popoverOpen = false;
    identityOpen = true;
  }}
/>

<IdentityModal open={identityOpen} onClose={() => (identityOpen = false)} />

<style>
  .shell {
    display: flex;
    height: calc(100vh - var(--banner-h, 0px));
    background: var(--bg);
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
</style>
```

- [ ] **Step 28.2: Commit**

```bash
git add frontend/src/lib/components/ShellLayout.svelte
git commit -m "feat(frontend): ShellLayout (Rail + ChatList + slot main)"
```

(`IdentityModal` is created in Task 35; build will fail until then.)

---

## Phase 6 — Chat pane (header + bubbles + composer)

### Task 29: `EncryptionBanner.svelte`

**Files:**
- Create: `frontend/src/lib/components/EncryptionBanner.svelte`

- [ ] **Step 29.1: Create**

Create `frontend/src/lib/components/EncryptionBanner.svelte`:

```svelte
<script lang="ts">
  import Icon from './Icon.svelte';
</script>

<div class="banner">
  <Icon name="lock" size={13} sw={2.2} color="var(--accent)" />
  Сообщения end-to-end зашифрованы. Только вы и собеседник можете их прочитать.
</div>

<style>
  .banner {
    margin: 20px auto;
    padding: 8px 14px;
    max-width: fit-content;
    background: var(--accent-dim);
    border-radius: 999px;
    font-size: 12px;
    color: var(--accent);
    font-weight: 500;
    display: flex;
    align-items: center;
    gap: 8px;
  }
</style>
```

- [ ] **Step 29.2: Commit**

```bash
git add frontend/src/lib/components/EncryptionBanner.svelte
git commit -m "feat(frontend): EncryptionBanner pill"
```

---

### Task 30: `MessageBubble.svelte`

**Files:**
- Create: `frontend/src/lib/components/MessageBubble.svelte`

- [ ] **Step 30.1: Create**

Create `frontend/src/lib/components/MessageBubble.svelte`:

```svelte
<script lang="ts">
  import Avatar from './Avatar.svelte';
  import Icon from './Icon.svelte';
  import type { MessageDto } from '$lib/types';

  type Props = {
    msg: MessageDto;
    senderName: string;
  };
  let { msg, senderName }: Props = $props();

  let mine = $derived(msg.direction === 'out');
  let timeText = $derived(
    new Date(msg.sent_at * 1000).toLocaleTimeString([], {
      hour: '2-digit',
      minute: '2-digit',
    })
  );
</script>

<div class="row" class:mine>
  {#if !mine}
    <Avatar name={senderName} size={32} />
  {/if}
  <div class="col" class:mine>
    <div class="bubble" class:mine>{msg.content}</div>
    <div class="meta">
      {timeText}
      {#if mine}
        <span class="check"><Icon name="checkDouble" size={12} sw={2.2} color="var(--accent)" /></span>
      {/if}
    </div>
  </div>
</div>

<style>
  .row {
    display: flex;
    justify-content: flex-start;
    gap: 10px;
    margin: 4px 0;
    padding: 0 24px;
  }
  .row.mine {
    justify-content: flex-end;
  }
  .col {
    max-width: 60%;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
  }
  .col.mine {
    align-items: flex-end;
  }
  .bubble {
    padding: 10px 14px;
    background: var(--bubble);
    border: 0.5px solid var(--border);
    border-radius: 16px;
    border-top-left-radius: 4px;
    color: var(--text);
    font-size: 14px;
    line-height: 1.5;
    letter-spacing: -0.05px;
    white-space: pre-wrap;
    word-wrap: break-word;
  }
  .bubble.mine {
    background: var(--bubble-mine);
    border: none;
    color: #fff;
    border-top-left-radius: 16px;
    border-top-right-radius: 4px;
  }
  .meta {
    font-size: 11px;
    color: var(--text-muted);
    margin-top: 4px;
    padding: 0 6px;
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .check {
    display: flex;
  }
</style>
```

- [ ] **Step 30.2: Commit**

```bash
git add frontend/src/lib/components/MessageBubble.svelte
git commit -m "feat(frontend): MessageBubble (mine/their variants)"
```

---

### Task 31: `Composer.svelte`

**Files:**
- Create: `frontend/src/lib/components/Composer.svelte`

- [ ] **Step 31.1: Create**

Create `frontend/src/lib/components/Composer.svelte`:

```svelte
<script lang="ts">
  import Icon from './Icon.svelte';

  type Props = {
    onSend: (text: string) => Promise<void>;
    disabled?: boolean;
  };
  let { onSend, disabled = false }: Props = $props();

  let text = $state('');
  let busy = $state(false);
  let errorMsg = $state<string | null>(null);
  let textarea: HTMLTextAreaElement | null = $state(null);

  async function send() {
    const t = text.trim();
    if (t === '' || busy) return;
    busy = true;
    errorMsg = null;
    try {
      await onSend(t);
      text = '';
      // Reset height
      if (textarea) textarea.style.height = 'auto';
    } catch (e) {
      errorMsg = String(e);
    } finally {
      busy = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      void send();
    }
  }

  function autosize() {
    if (!textarea) return;
    textarea.style.height = 'auto';
    textarea.style.height = Math.min(textarea.scrollHeight, 160) + 'px';
  }
</script>

<div class="wrap">
  <div class="bar">
    <textarea
      bind:this={textarea}
      bind:value={text}
      onkeydown={onKey}
      oninput={autosize}
      disabled={disabled || busy}
      rows="1"
      placeholder="Напишите сообщение…"
    ></textarea>
    <button
      type="button"
      class="send"
      onclick={send}
      disabled={disabled || busy || text.trim() === ''}
      aria-label="Отправить"
    >
      <Icon name="send" size={16} sw={2} color="#fff" />
    </button>
  </div>
  {#if errorMsg}<p class="error">{errorMsg}</p>{/if}
</div>

<style>
  .wrap {
    padding: 12px 20px 16px;
    border-top: 0.5px solid var(--border);
    background: var(--bg);
  }
  .bar {
    background: var(--surface);
    border: 0.5px solid var(--border);
    border-radius: 14px;
    padding: 4px 6px 4px 14px;
    display: flex;
    align-items: flex-end;
    gap: 8px;
  }
  textarea {
    flex: 1;
    border: 0;
    background: transparent;
    color: var(--text);
    font-size: 14px;
    line-height: 1.5;
    padding: 9px 0;
    resize: none;
    outline: none;
    font-family: inherit;
    max-height: 160px;
  }
  textarea::placeholder {
    color: var(--text-muted);
  }
  .send {
    width: 36px;
    height: 36px;
    border-radius: 10px;
    border: 0;
    cursor: pointer;
    background: linear-gradient(135deg, #6c5ce7, var(--accent));
    color: #fff;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 4px 16px var(--accent-soft);
  }
  .send:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .error {
    margin: 8px 0 0 0;
    color: var(--danger);
    font-size: 12px;
  }
</style>
```

- [ ] **Step 31.2: Commit**

```bash
git add frontend/src/lib/components/Composer.svelte
git commit -m "feat(frontend): Composer (textarea + Enter-to-send + send button)"
```

---

### Task 32: `ChatHeader.svelte`

**Files:**
- Create: `frontend/src/lib/components/ChatHeader.svelte`

- [ ] **Step 32.1: Create**

Create `frontend/src/lib/components/ChatHeader.svelte`:

```svelte
<script lang="ts">
  import Avatar from './Avatar.svelte';
  import Icon from './Icon.svelte';
  import ContactMenu from './ContactMenu.svelte';
  import type { ContactDto } from '$lib/types';

  type Props = {
    contact: ContactDto;
  };
  let { contact }: Props = $props();

  let menu = $state<{ x: number; y: number } | null>(null);

  let displayName = $derived(
    contact.local_alias ?? contact.display_name ?? contact.fingerprint
  );
</script>

<header class="hdr">
  <Avatar name={displayName} size={40} />
  <div class="meta">
    <div class="name">
      <span>{displayName}</span>
      {#if contact.verified}
        <Icon name="shield" size={13} sw={2.2} color="var(--accent)" />
      {/if}
      <span class="e2e-pill">E2E</span>
    </div>
    <div class="sub">
      {contact.fingerprint}
      {#if contact.retention_seconds}
        · авто-удаление {formatSeconds(contact.retention_seconds)}
      {/if}
    </div>
  </div>
  <button
    type="button"
    class="more"
    aria-label="Действия"
    onclick={(e) => (menu = { x: e.clientX, y: e.clientY })}
  >
    <Icon name="more" size={18} color="var(--text-dim)" />
  </button>
</header>

{#if menu}
  <ContactMenu {contact} x={menu.x} y={menu.y} onClose={() => (menu = null)} />
{/if}

<script context="module" lang="ts">
  function formatSeconds(s: number): string {
    if (s >= 86400) return `${Math.round(s / 86400)}d`;
    if (s >= 3600) return `${Math.round(s / 3600)}h`;
    if (s >= 60) return `${Math.round(s / 60)}m`;
    return `${s}s`;
  }
</script>

<style>
  .hdr {
    height: 64px;
    padding: 0 20px;
    border-bottom: 0.5px solid var(--border);
    display: flex;
    align-items: center;
    gap: 12px;
    background: var(--bg);
    flex-shrink: 0;
  }
  .meta {
    flex: 1;
    min-width: 0;
  }
  .name {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
    letter-spacing: -0.1px;
  }
  .e2e-pill {
    font-size: 10px;
    font-weight: 600;
    padding: 2px 6px;
    border-radius: 4px;
    background: var(--accent-dim);
    color: var(--accent);
    letter-spacing: 0.4px;
  }
  .sub {
    font-size: 12px;
    color: var(--text-dim);
    margin-top: 2px;
    font-family: 'JetBrains Mono', monospace;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .more {
    width: 36px;
    height: 36px;
    border-radius: 9px;
    border: 0;
    cursor: pointer;
    background: transparent;
    color: var(--text-dim);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .more:hover {
    background: var(--hover);
  }
</style>
```

- [ ] **Step 32.2: Commit**

```bash
git add frontend/src/lib/components/ChatHeader.svelte
git commit -m "feat(frontend): ChatHeader (name + e2e + fingerprint + actions menu)"
```

---

### Task 33: `ChatPane.svelte`

**Files:**
- Create: `frontend/src/lib/components/ChatPane.svelte`

- [ ] **Step 33.1: Create**

Create `frontend/src/lib/components/ChatPane.svelte`:

```svelte
<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { listMessages, sendMessage, onInbound, markChatRead, listContacts } from '$lib/tauri';
  import { store } from '$lib/state.svelte';
  import ChatHeader from './ChatHeader.svelte';
  import Composer from './Composer.svelte';
  import EncryptionBanner from './EncryptionBanner.svelte';
  import MessageBubble from './MessageBubble.svelte';
  import type { ContactDto, MessageDto } from '$lib/types';

  type Props = {
    contactGhostId: string;
  };
  let { contactGhostId }: Props = $props();

  let contact = $derived(
    store.contacts.find((c) => c.ghost_id === contactGhostId) as ContactDto | undefined
  );
  let messages = $derived<MessageDto[]>(store.threads[contactGhostId] ?? []);

  let scrollRef: HTMLDivElement | null = $state(null);
  let errorMsg = $state<string | null>(null);
  let unlisten: (() => void) | null = null;

  async function refreshContacts() {
    const cs = await listContacts();
    store.setContacts(cs);
  }

  async function loadInitial() {
    try {
      const msgs = await listMessages(contactGhostId);
      store.setThread(contactGhostId, msgs);
      // Mark as read after the load completes; refresh contact list to clear badge.
      await markChatRead(contactGhostId);
      await refreshContacts();
    } catch (e) {
      errorMsg = String(e);
    }
  }

  async function send(text: string) {
    await sendMessage(contactGhostId, text);
    const msgs = await listMessages(contactGhostId);
    store.setThread(contactGhostId, msgs);
    await refreshContacts();
  }

  // Effect: when contactGhostId changes, reload.
  $effect(() => {
    void contactGhostId;
    untrack(() => {
      void loadInitial();
    });
  });

  // Effect: scroll to bottom when messages change.
  $effect(() => {
    void messages;
    if (scrollRef) {
      scrollRef.scrollTop = scrollRef.scrollHeight;
    }
  });

  onMount(() => {
    void onInbound(async (ev) => {
      if (ev.from_ghost_id === contactGhostId) {
        const msgs = await listMessages(contactGhostId);
        store.setThread(contactGhostId, msgs);
        await markChatRead(contactGhostId);
        await refreshContacts();
      } else {
        // Unread badge for OTHER contact bumps automatically when refreshContacts() runs;
        // do a refresh so the sidebar count is up to date.
        await refreshContacts();
      }
    }).then((u) => {
      unlisten = u;
    });

    return () => {
      unlisten?.();
    };
  });
</script>

{#if !contact}
  <div class="loading">Загрузка контакта…</div>
{:else}
  <ChatHeader {contact} />
  <div bind:this={scrollRef} class="scroll">
    <EncryptionBanner />
    {#each messages as m, i (m.uuid || `${i}-${m.sent_at}`)}
      <MessageBubble msg={m} senderName={contact.local_alias ?? contact.display_name ?? contact.fingerprint} />
    {/each}
    {#if messages.length === 0}
      <div class="empty">Сообщений пока нет.</div>
    {/if}
  </div>
  <Composer onSend={send} />
  {#if errorMsg}<p class="err">{errorMsg}</p>{/if}
{/if}

<style>
  .scroll {
    flex: 1;
    overflow-y: auto;
    padding: 8px 0 16px;
    background: var(--bg);
  }
  .empty {
    text-align: center;
    color: var(--text-muted);
    margin-top: 80px;
    font-size: 13px;
  }
  .loading {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-dim);
  }
  .err {
    color: var(--danger);
    padding: 8px 20px;
    margin: 0;
    font-size: 12px;
  }
</style>
```

- [ ] **Step 33.2: Commit**

```bash
git add frontend/src/lib/components/ChatPane.svelte
git commit -m "feat(frontend): ChatPane (header + scroll + composer + mark-read)"
```

---

## Phase 7 — Modals + EmptyState

### Task 34: `InviteModal.svelte`

**Files:**
- Create: `frontend/src/lib/components/InviteModal.svelte`

- [ ] **Step 34.1: Create**

Create `frontend/src/lib/components/InviteModal.svelte`:

```svelte
<script lang="ts">
  import Modal from './Modal.svelte';
  import { createInvite } from '$lib/tauri';

  type Props = {
    open: boolean;
    onClose: () => void;
  };
  let { open, onClose }: Props = $props();

  let invite = $state<string | null>(null);
  let busy = $state(false);
  let errorMsg = $state<string | null>(null);
  let copied = $state(false);

  async function generate() {
    busy = true;
    errorMsg = null;
    copied = false;
    try {
      const r = await createInvite();
      invite = r.bech32;
    } catch (e) {
      errorMsg = String(e);
    } finally {
      busy = false;
    }
  }

  async function copy() {
    if (!invite) return;
    try {
      await navigator.clipboard.writeText(invite);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch (e) {
      errorMsg = String(e);
    }
  }

  // Reset state when modal reopens.
  $effect(() => {
    if (!open) {
      invite = null;
      copied = false;
      errorMsg = null;
    }
  });
</script>

<Modal {open} {onClose} title="Создать инвайт">
  <p class="desc">
    Поделитесь этой строкой с одним человеком. Срок действия — 7 дней.
  </p>

  {#if !invite}
    <button type="button" class="primary" onclick={generate} disabled={busy}>
      {busy ? 'Генерация…' : 'Создать инвайт'}
    </button>
  {:else}
    <textarea readonly rows="3" class="bech">{invite}</textarea>
    <button type="button" class="ghost" onclick={copy}>
      {copied ? 'Скопировано!' : 'Копировать'}
    </button>
  {/if}

  {#if errorMsg}
    <p class="error">{errorMsg}</p>
  {/if}
</Modal>

<style>
  .desc {
    margin: 0 0 12px 0;
    color: var(--text-dim);
    font-size: 13px;
  }
  .primary {
    padding: 10px 18px;
    background: var(--accent);
    color: #fff;
    border: 0;
    border-radius: 8px;
    cursor: pointer;
    font-weight: 600;
  }
  .ghost {
    margin-top: 8px;
    padding: 8px 14px;
    background: transparent;
    color: var(--text);
    border: 0.5px solid var(--border);
    border-radius: 8px;
    cursor: pointer;
  }
  .bech {
    width: 100%;
    padding: 10px;
    background: var(--bg);
    color: var(--text);
    border: 0.5px solid var(--border);
    border-radius: 8px;
    font-family: 'JetBrains Mono', monospace;
    font-size: 12px;
    resize: none;
  }
  .error {
    color: var(--danger);
    font-size: 12px;
    margin-top: 8px;
  }
</style>
```

- [ ] **Step 34.2: Commit**

```bash
git add frontend/src/lib/components/InviteModal.svelte
git commit -m "feat(frontend): InviteModal (generate + copy bech32)"
```

---

### Task 35: `AddContactModal.svelte`

**Files:**
- Create: `frontend/src/lib/components/AddContactModal.svelte`

- [ ] **Step 35.1: Create**

Create `frontend/src/lib/components/AddContactModal.svelte`:

```svelte
<script lang="ts">
  import Modal from './Modal.svelte';
  import { addContact, listContacts } from '$lib/tauri';
  import { store } from '$lib/state.svelte';

  type Props = {
    open: boolean;
    onClose: () => void;
  };
  let { open, onClose }: Props = $props();

  let inviteInput = $state('');
  let busy = $state(false);
  let errorMsg = $state<string | null>(null);
  let okMsg = $state<string | null>(null);

  async function submit(e: Event) {
    e.preventDefault();
    if (inviteInput.trim() === '' || busy) return;
    busy = true;
    errorMsg = null;
    okMsg = null;
    try {
      await addContact(inviteInput.trim());
      inviteInput = '';
      okMsg = 'Контакт добавлен.';
      const cs = await listContacts();
      store.setContacts(cs);
      setTimeout(() => onClose(), 1000);
    } catch (e) {
      errorMsg = String(e);
    } finally {
      busy = false;
    }
  }

  $effect(() => {
    if (!open) {
      inviteInput = '';
      errorMsg = null;
      okMsg = null;
    }
  });
</script>

<Modal {open} {onClose} title="Добавить контакт">
  <p class="desc">Вставьте инвайт-строку, которую вам прислали.</p>

  <form onsubmit={submit}>
    <textarea
      bind:value={inviteInput}
      disabled={busy}
      rows="3"
      placeholder="ghostinvite1q…"
    ></textarea>
    <button
      type="submit"
      class="primary"
      disabled={busy || inviteInput.trim() === ''}
    >
      {busy ? 'Добавление…' : 'Добавить'}
    </button>
  </form>

  {#if errorMsg}<p class="error">{errorMsg}</p>{/if}
  {#if okMsg}<p class="ok">{okMsg}</p>{/if}
</Modal>

<style>
  .desc {
    margin: 0 0 12px 0;
    color: var(--text-dim);
    font-size: 13px;
  }
  textarea {
    width: 100%;
    padding: 10px;
    background: var(--bg);
    color: var(--text);
    border: 0.5px solid var(--border);
    border-radius: 8px;
    font-family: 'JetBrains Mono', monospace;
    font-size: 12px;
    resize: vertical;
  }
  .primary {
    margin-top: 10px;
    padding: 10px 18px;
    background: var(--accent);
    color: #fff;
    border: 0;
    border-radius: 8px;
    cursor: pointer;
    font-weight: 600;
  }
  .primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .error {
    color: var(--danger);
    font-size: 12px;
    margin-top: 8px;
  }
  .ok {
    color: var(--success);
    font-size: 12px;
    margin-top: 8px;
  }
</style>
```

- [ ] **Step 35.2: Commit**

```bash
git add frontend/src/lib/components/AddContactModal.svelte
git commit -m "feat(frontend): AddContactModal (paste invite + submit)"
```

---

### Task 36: `IdentityModal.svelte`

**Files:**
- Create: `frontend/src/lib/components/IdentityModal.svelte`

- [ ] **Step 36.1: Create**

Create `frontend/src/lib/components/IdentityModal.svelte`:

```svelte
<script lang="ts">
  import Modal from './Modal.svelte';
  import { store } from '$lib/state.svelte';

  type Props = {
    open: boolean;
    onClose: () => void;
  };
  let { open, onClose }: Props = $props();

  let copiedId = $state(false);
  let copiedFp = $state(false);

  async function copyId() {
    if (!store.info) return;
    await navigator.clipboard.writeText(store.info.ghost_id);
    copiedId = true;
    setTimeout(() => (copiedId = false), 1500);
  }

  async function copyFp() {
    if (!store.info) return;
    await navigator.clipboard.writeText(store.info.fingerprint);
    copiedFp = true;
    setTimeout(() => (copiedFp = false), 1500);
  }
</script>

<Modal {open} {onClose} title="Ваш Ghost ID">
  <p class="desc">
    Поделитесь полным ID или коротким fingerprint'ом для verbal-сверки. ID не
    содержит секретов — это публичный ключ вашей идентификации.
  </p>

  {#if store.info}
    <div class="label">Полный ID</div>
    <div class="row">
      <code>{store.info.ghost_id}</code>
      <button type="button" onclick={copyId}>{copiedId ? '✓' : 'Копировать'}</button>
    </div>

    <div class="label">Fingerprint</div>
    <div class="row">
      <code class="fp">{store.info.fingerprint}</code>
      <button type="button" onclick={copyFp}>{copiedFp ? '✓' : 'Копировать'}</button>
    </div>
  {/if}
</Modal>

<style>
  .desc {
    margin: 0 0 14px 0;
    color: var(--text-dim);
    font-size: 13px;
  }
  .label {
    font-size: 10px;
    font-weight: 700;
    color: var(--text-muted);
    letter-spacing: 0.6px;
    text-transform: uppercase;
    margin-top: 12px;
    margin-bottom: 6px;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
    background: var(--bg);
    border: 0.5px solid var(--border);
    border-radius: 8px;
    padding: 8px 10px;
  }
  code {
    flex: 1;
    font-family: 'JetBrains Mono', monospace;
    font-size: 12px;
    color: var(--text);
    word-break: break-all;
    overflow-wrap: anywhere;
  }
  code.fp {
    font-size: 14px;
    letter-spacing: 0.5px;
  }
  button {
    padding: 6px 12px;
    background: transparent;
    color: var(--text);
    border: 0.5px solid var(--border);
    border-radius: 6px;
    cursor: pointer;
    font-size: 12px;
    flex-shrink: 0;
  }
  button:hover {
    background: var(--hover);
  }
</style>
```

- [ ] **Step 36.2: Commit**

```bash
git add frontend/src/lib/components/IdentityModal.svelte
git commit -m "feat(frontend): IdentityModal (full ID + fingerprint with copy)"
```

---

### Task 37: `EmptyState.svelte`

**Files:**
- Create: `frontend/src/lib/components/EmptyState.svelte`

- [ ] **Step 37.1: Create**

Create `frontend/src/lib/components/EmptyState.svelte`:

```svelte
<script lang="ts">
  import Icon from './Icon.svelte';
  import InviteModal from './InviteModal.svelte';
  import AddContactModal from './AddContactModal.svelte';

  let inviteOpen = $state(false);
  let addOpen = $state(false);
</script>

<div class="root">
  <div class="bg"></div>
  <div class="ghost">
    <!-- Orbit rings -->
    {#each [120, 90, 60] as r, i}
      <div
        class="orbit"
        style:width="{r * 2}px"
        style:height="{r * 2}px"
        style:margin-left="-{r}px"
        style:margin-top="-{r}px"
        style:opacity={(0.6 - i * 0.15).toFixed(2)}
      ></div>
    {/each}
    <svg width="280" height="280" viewBox="0 0 280 280" style="position: relative; z-index: 1;">
      <defs>
        <linearGradient id="ghostGrad" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stop-color="var(--accent)" stop-opacity="0.95" />
          <stop offset="100%" stop-color="#6c5ce7" stop-opacity="0.85" />
        </linearGradient>
        <filter id="softGlow">
          <feGaussianBlur stdDeviation="6" />
        </filter>
      </defs>
      <g transform="translate(70, 60)">
        <path
          d="M70 0 C30 0, 0 32, 0 72 L0 158 L18 144 L36 158 L54 144 L72 158 L90 144 L108 158 L126 144 L140 158 L140 72 C140 32, 110 0, 70 0 Z"
          fill="url(#ghostGrad)"
          opacity="0.18"
          filter="url(#softGlow)"
        />
        <path
          d="M70 0 C30 0, 0 32, 0 72 L0 158 L18 144 L36 158 L54 144 L72 158 L90 144 L108 158 L126 144 L140 158 L140 72 C140 32, 110 0, 70 0 Z"
          fill="url(#ghostGrad)"
        />
        <ellipse cx="48" cy="68" rx="6" ry="8" fill="var(--bg)" />
        <ellipse cx="92" cy="68" rx="6" ry="8" fill="var(--bg)" />
        <ellipse cx="48" cy="66" rx="2" ry="2.5" fill="#fff" opacity="0.6" />
        <ellipse cx="92" cy="66" rx="2" ry="2.5" fill="#fff" opacity="0.6" />
        <path
          d="M58 96 Q70 104 82 96"
          stroke="var(--bg)"
          stroke-width="3"
          stroke-linecap="round"
          fill="none"
          opacity="0.7"
        />
      </g>
      <circle cx="40" cy="70" r="3" fill="var(--accent)" opacity="0.5" />
      <circle cx="240" cy="100" r="2" fill="var(--accent)" opacity="0.4" />
      <circle cx="220" cy="220" r="4" fill="var(--accent)" opacity="0.3" />
      <circle cx="50" cy="220" r="2.5" fill="var(--accent)" opacity="0.4" />
    </svg>
  </div>

  <div class="title">Выберите чат, чтобы начать беседу</div>
  <div class="sub">
    Каждое сообщение в Ghost зашифровано end-to-end и не оставляет следов на серверах.
  </div>

  <div class="pills">
    <span class="pill"><Icon name="lock" size={13} sw={2} color="var(--success)" /> E2E активно</span>
    <span class="pill"><Icon name="ghost" size={13} sw={2} color="var(--text-dim)" /> 0 логов</span>
  </div>

  <div class="cta">
    <button type="button" class="primary" onclick={() => (inviteOpen = true)}>Создать инвайт</button>
    <button type="button" class="ghost" onclick={() => (addOpen = true)}>Добавить контакт</button>
  </div>
</div>

<InviteModal open={inviteOpen} onClose={() => (inviteOpen = false)} />
<AddContactModal open={addOpen} onClose={() => (addOpen = false)} />

<style>
  .root {
    flex: 1;
    background: var(--bg);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-direction: column;
    position: relative;
    overflow: hidden;
  }
  .bg {
    position: absolute;
    inset: 0;
    background: radial-gradient(circle at 50% 40%, var(--accent-dim) 0%, transparent 55%);
    pointer-events: none;
  }
  .ghost {
    position: relative;
    width: 280px;
    height: 280px;
    margin-bottom: 32px;
  }
  .orbit {
    position: absolute;
    left: 50%;
    top: 50%;
    border-radius: 50%;
    border: 1px dashed var(--border);
  }
  .title {
    font-size: 22px;
    font-weight: 600;
    color: var(--text);
    letter-spacing: -0.4px;
    margin-bottom: 8px;
    position: relative;
    z-index: 1;
  }
  .sub {
    font-size: 14px;
    color: var(--text-dim);
    max-width: 380px;
    text-align: center;
    line-height: 1.6;
    position: relative;
    z-index: 1;
  }
  .pills {
    display: flex;
    gap: 8px;
    margin-top: 28px;
    position: relative;
    z-index: 1;
  }
  .pill {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 14px;
    border-radius: 999px;
    background: var(--surface);
    border: 0.5px solid var(--border);
    font-size: 12px;
    font-weight: 500;
    color: var(--text-dim);
  }
  .cta {
    display: flex;
    gap: 8px;
    margin-top: 36px;
    position: relative;
    z-index: 1;
  }
  .primary {
    padding: 10px 18px;
    background: var(--accent);
    color: #fff;
    border: 0;
    border-radius: 10px;
    cursor: pointer;
    font-weight: 600;
    font-size: 13px;
  }
  .ghost {
    padding: 10px 18px;
    background: transparent;
    color: var(--text);
    border: 0.5px solid var(--border);
    border-radius: 10px;
    cursor: pointer;
    font-weight: 500;
    font-size: 13px;
  }
</style>
```

- [ ] **Step 37.2: Commit**

```bash
git add frontend/src/lib/components/EmptyState.svelte
git commit -m "feat(frontend): EmptyState (ghost illustration + welcome + CTAs)"
```

---

## Phase 8 — Routing wiring + cleanup

### Task 38: Update `+layout.svelte` to use ShellLayout + boot theme

**Files:**
- Modify: `frontend/src/routes/+layout.svelte`

- [ ] **Step 38.1: Replace layout to consolidate shell + theme + boot**

Replace `frontend/src/routes/+layout.svelte`:

```svelte
<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import {
    identityStatus,
    openClient,
    clientInfo,
    listContacts,
    onInbound,
    getSetting,
  } from '$lib/tauri';
  import { bootTheme } from '$lib/theme';
  import { store } from '$lib/state.svelte';
  import UpdateBanner from '$lib/components/UpdateBanner.svelte';
  import ShellLayout from '$lib/components/ShellLayout.svelte';

  let { children } = $props();

  let booting = $state(true);
  let bootError = $state<string | null>(null);
  let unlistenFn: (() => void) | null = null;

  let route = $derived(page.url.pathname);
  let isOnboarding = $derived(route === '/onboarding');

  async function boot() {
    try {
      // Theme first so it's applied before any UI shows.
      const t = await bootTheme();
      store.setTheme(t);

      // Ghost mode setting (visual-only).
      const gm = await getSetting('ghost_mode');
      store.setGhostMode(gm === '1');

      const status = await identityStatus();
      if (!status.exists) {
        if (!isOnboarding) await goto('/onboarding');
        return;
      }
      const info = status.client_open ? await clientInfo() : await openClient(null);
      store.setInfo(info);

      const cs = await listContacts();
      store.setContacts(cs);

      const u = await onInbound(async (ev) => {
        store.pushIncoming(ev.from_ghost_id, {
          uuid: '',
          direction: 'in',
          content: ev.content,
          sent_at: ev.received_at,
          received_at: ev.received_at,
        });
        // Keep sidebar (last-message previews + unread counts) in sync.
        const cs = await listContacts();
        store.setContacts(cs);
      });
      unlistenFn = () => void u();
    } catch (e) {
      bootError = String(e);
    } finally {
      booting = false;
    }
  }

  onMount(() => {
    void boot();
    return () => {
      unlistenFn?.();
    };
  });
</script>

<UpdateBanner />

{#if booting}
  <div class="boot">Загрузка…</div>
{:else if bootError}
  <div class="boot err">{bootError}</div>
{:else if isOnboarding}
  {@render children()}
{:else}
  <ShellLayout>
    {@render children()}
  </ShellLayout>
{/if}

<style>
  .boot {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
    color: var(--text-dim);
    font-size: 14px;
  }
  .boot.err {
    color: var(--danger);
    padding: 20px;
    text-align: center;
  }
</style>
```

- [ ] **Step 38.2: Commit**

```bash
git add frontend/src/routes/+layout.svelte
git commit -m "feat(frontend): layout boots theme/identity, hosts ShellLayout"
```

---

### Task 39: Replace `+page.svelte` with EmptyState wrapper

**Files:**
- Modify: `frontend/src/routes/+page.svelte`

- [ ] **Step 39.1: Replace**

Replace `frontend/src/routes/+page.svelte`:

```svelte
<script lang="ts">
  import EmptyState from '$lib/components/EmptyState.svelte';
</script>

<EmptyState />
```

- [ ] **Step 39.2: Commit**

```bash
git add frontend/src/routes/+page.svelte
git commit -m "feat(frontend): root route renders EmptyState"
```

---

### Task 40: Replace chat route with `ChatPane` wrapper

**Files:**
- Modify: `frontend/src/routes/chat/[ghost_id]/+page.svelte`

- [ ] **Step 40.1: Replace**

Replace `frontend/src/routes/chat/[ghost_id]/+page.svelte`:

```svelte
<script lang="ts">
  import { page } from '$app/state';
  import ChatPane from '$lib/components/ChatPane.svelte';

  let contactGhostId = $derived(decodeURIComponent(page.params.ghost_id ?? ''));
</script>

{#key contactGhostId}
  <ChatPane {contactGhostId} />
{/key}
```

(`{#key …}` forces remount when the ghost_id param changes — guarantees a fresh `loadInitial()`.)

- [ ] **Step 40.2: Commit**

```bash
git add frontend/src/routes/chat/[ghost_id]/+page.svelte
git commit -m "feat(frontend): chat route renders ChatPane"
```

---

### Task 41: Delete obsolete components

**Files:**
- Delete: `frontend/src/lib/components/InviteCard.svelte`
- Delete: `frontend/src/lib/components/AddContactForm.svelte`
- Delete: `frontend/src/lib/components/ContactList.svelte`

- [ ] **Step 41.1: Remove the files**

```bash
rm frontend/src/lib/components/InviteCard.svelte \
   frontend/src/lib/components/AddContactForm.svelte \
   frontend/src/lib/components/ContactList.svelte
```

- [ ] **Step 41.2: Run typecheck and build**

Run:

```bash
pnpm --dir frontend check
pnpm --dir frontend build
```

Expected: typecheck passes, build succeeds. Any remaining import errors point at a real bug — fix before moving on.

- [ ] **Step 41.3: Commit**

```bash
git add -A frontend/src/lib/components/
git commit -m "chore(frontend): remove obsolete InviteCard/AddContactForm/ContactList"
```

---

## Phase 9 — Local smoke + version bump + release

### Task 42: Local `pnpm dev` + Tauri dev smoke

**Files:** none (manual verification)

- [ ] **Step 42.1: Build the workspace**

Run:

```bash
cargo build --workspace
```

Expected: success, no warnings beyond pre-existing.

- [ ] **Step 42.2: Run the desktop app in dev mode**

Run:

```bash
cargo tauri dev --config apps/ghost-desktop/tauri.conf.json
```

(If `tauri-cli` not installed: `cargo install tauri-cli --version "^2" --locked` first.)

The app launches. Walk through:
- Onboarding (if no identity) → create one with display name / passphrase.
- Main shell shows up: rail (one active "All" + 6 decorative), empty chat list, EmptyState in main with ghost illustration + 2 CTAs.
- Click profile avatar in rail bottom → popover with theme switch, ghost-mode toggle, "Show my Ghost ID" button.
- Toggle theme dark↔light: tokens flip, layout intact.
- Toggle ghost mode: avatar gets purple ring, no other change (visual-only).
- Click "Create invite" → modal → generate invite → copy → close.
- (If you have a second machine / dev instance) paste invite into "Add contact" → contact appears in list.
- Right-click a contact row → menu opens → toggle pin / mute / verified, pick retention. Confirm sidebar refreshes.
- Click contact → ChatPane opens. Type message, Enter → sent. Bubble appears on right with double-check.
- Receive a message (test from peer) → bubble on left. Sidebar last-message + unread count update.
- Open chat with unread → unread badge clears (mark-read).

- [ ] **Step 42.3: Commit any tweaks discovered during smoke**

If the smoke surfaced bugs, fix and commit each in its own focused commit.

---

### Task 43: Version bump 0.0.3 → 0.0.4

**Files:**
- Modify: `apps/ghost-desktop/tauri.conf.json`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

- [ ] **Step 43.1: Bump tauri.conf.json version**

In `apps/ghost-desktop/tauri.conf.json`, change `"version": "0.0.3"` to `"version": "0.0.4"`.

- [ ] **Step 43.2: Bump workspace Cargo.toml**

In `Cargo.toml` `[workspace.package]`, change `version = "0.0.3"` to `version = "0.0.4"`.

- [ ] **Step 43.3: Bump workspace crate versions in Cargo.lock**

Run:

```bash
sed -i '/^name = "ghost-/{n; s/^version = "0\.0\.3"$/version = "0.0.4"/}' Cargo.lock
```

Verify:

```bash
grep -A1 '^name = "ghost-' Cargo.lock | head -20
```

Expected: every `ghost-*` crate shows `version = "0.0.4"`.

- [ ] **Step 43.4: Commit the bump**

```bash
git add apps/ghost-desktop/tauri.conf.json Cargo.toml Cargo.lock
git commit -m "chore: bump version to 0.0.4 (sidebar redesign)"
```

---

### Task 44: Tag v0.0.4, push, watch CI publish

**Files:** none (git operations)

- [ ] **Step 44.1: Push master**

```bash
git push origin HEAD:master
```

(If the working branch is `master`, this is just `git push`. If it's a feature branch in a worktree, this pushes the branch's HEAD to `origin/master` directly.)

- [ ] **Step 44.2: Tag v0.0.4 with annotated message**

```bash
git tag -a v0.0.4 -m "$(cat <<'EOF'
Ghost v0.0.4 — sidebar redesign.

Replaces the single-column home with a Telegram/Discord-style shell:
80px folder rail + 360px chat list + main pane (open chat or empty
state). Per-contact pin/mute/verified/retention now persist; theme is
selectable (dark/light); ghost-mode toggle (visual-only in MVP-1).

Backend: migration 0003 adds last_read_at/pinned/muted/retention_seconds
on contacts. Background scrubber purges expired messages every 60s.
EOF
)"
```

- [ ] **Step 44.3: Push the tag**

```bash
git push origin v0.0.4
```

- [ ] **Step 44.4: Confirm CI started**

Open https://github.com/borgershalm559-hash/ghost-im/actions — there should be a Release run for tag v0.0.4 plus a CI run for the master push. The Release run takes ~25-30 min (no cargo cache).

- [ ] **Step 44.5: Watch for publication**

Run:

```bash
until curl -sSL https://github.com/borgershalm559-hash/ghost-im/releases/download/v0.0.4/latest.json 2>/dev/null | grep -q '"version": "0.0.4"'; do
  sleep 90
  date +'%H:%M:%S still building'
done
echo 'PUBLISHED'
curl -sSL https://github.com/borgershalm559-hash/ghost-im/releases/download/v0.0.4/latest.json
```

Expected: eventually prints `PUBLISHED` then the latest.json with `"version": "0.0.4"`.

- [ ] **Step 44.6: Verify auto-update on running v0.0.3**

On the test machine where v0.0.3 is installed:
1. Close Ghost.exe (if open).
2. Open Ghost.exe — within ~5-10s the yellow banner "↑ Доступна Ghost 0.0.4" should appear.
3. Click "Перезапустить" — banner switches to download progress, MSI installer takes over.
4. After install, Ghost re-launches as v0.0.4. Confirm:
   - Identity preserved (same Ghost ID).
   - Sidebar layout active (rail + list + main).
   - Old contacts and messages still present.

- [ ] **Step 44.7: Done**

If 44.6 succeeds, the entire plan is delivered: sidebar shipped, auto-update path proven across two real builds, install-once-and-update path established. No more manual reinstalls for subsequent feature work — every future bump auto-flows the same way.

---

## Self-review checklist (executed by the plan writer, archived here for the implementer)

**Spec coverage:**
- IA / routes (§2 of spec) → Tasks 38-40
- Visual system colors/typography/sizes (§3) → Tasks 12-13, 15
- Components (§4): Avatar/Modal/SearchBar/Icon → Tasks 19-22; Rail/ChatList/ChatRow/ContactMenu/Profile* → Tasks 23-28; ChatPane suite → Tasks 29-33; Modals + EmptyState → Tasks 34-37
- Backend migration 0003 (§5) → Task 1
- Contact repo extension (§5) → Task 2
- Setter methods (§5) → Task 3
- unread_count (§5) → Task 4
- send_message retention (§5) → Task 5
- Background scrubber (§5) → Task 6
- Settings commands (§5) → Task 7
- Contact-action commands (§5) → Task 8
- Extended ContactDto + list_contacts (§5) → Task 9
- Tauri command registration (§5) → Task 10
- Tests (§6) → Tasks 1.3, 2.7, 3.x, 4.x, 11
- Rollout (§7): bump + tag + watch → Tasks 43-44
- Decisions (§8): default theme dark via Task 14; default retention NULL via Task 1's schema default; ghost_mode visual-only via Task 24's toggleGhostMode; decorative folders no-op via Task 23

**Placeholder scan:** every step contains concrete code or commands. No "TBD", "TODO", "implement later", "similar to Task N", "add appropriate validation".

**Type consistency:**
- `set_pinned`/`set_muted`/`set_verified`/`set_retention`/`set_last_read_at` — same names in Task 3 (repo), Task 8 (Client wrappers), Task 8 (Tauri commands), Task 17 (TS), Task 26 (component).
- `mark_chat_read` — Task 8 / 17 / 33.
- `purge_expired` (existing) — Task 6 / 11.
- `unread_count` — Task 4 (repo) / Task 9 (client wrapper) / Task 9 (DTO field).
- `getSetting` / `setSetting` — Task 7 / 14 / 17.
- `ContactDto` field names match across `dto.rs` (Task 9) and `types.ts` (Task 16).

No drift detected.

---

## Plan complete

**Plan complete and saved to `docs/superpowers/plans/2026-05-07-ghost-plan-09-sidebar.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints for review.

**Which approach?**






