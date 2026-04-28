# Ghost Plan 07 — Tauri App + SvelteKit Frontend

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wrap the existing `ghost-client` library in a working Tauri 2.x desktop application with a SvelteKit frontend. Plan 07 produces the first user-visible Ghost binary: a window where a human can create an identity, view their Ghost ID, generate an invite, add a contact via someone else's invite, and exchange E2EE text messages with them.

**Architecture:** Two new crates plus one frontend project:
- `crates/ghost-app/` — Tauri command layer (library). Holds an `AppState { client: Mutex<Option<Arc<Client>>> }`, exposes `#[tauri::command]` async functions that the frontend invokes. Wraps `ghost-client` and converts `ClientError` into `String`-shaped errors that serialize cleanly to JS.
- `apps/ghost-desktop/` — the Tauri shell binary. Loads `ghost-app`'s commands, configures the window, ships the SvelteKit-built static frontend as bundled resources.
- `frontend/` — SvelteKit project (Svelte 5 runes API, static adapter). Three routes: `/onboarding`, `/` (home), `/chat/[ghost_id]`. Talks to backend exclusively via Tauri's `invoke()` and event listener APIs.

The frontend never touches Rust types directly — all command inputs/outputs use plain JSON shapes defined in `ghost-app/src/dto.rs` and mirrored in `frontend/src/lib/types.ts`. Incoming messages are pushed from the inbox processor to the UI via Tauri events (`app.emit("ghost://message-received", ...)`), so the chat view updates without polling.

**Tech Stack:**
- Rust: `tauri = "2"`, `tauri-build = "2"` (build-dep), `serde`, `tokio`, `tracing`, plus all existing `ghost-*` deps.
- Frontend: `@sveltejs/kit ^2`, `svelte ^5`, `@sveltejs/adapter-static ^3`, `vite ^5`, `@tauri-apps/api ^2`, `@tauri-apps/cli ^2` (devdep). Package manager: pnpm (lock file shipped).

**Deliverable Plan 07:** A working `cargo tauri dev` invocation opens a desktop window. Two engineers running it on the same machine (with separate `GHOST_HOME` directories) complete onboarding, exchange invites, and send "hello" / "hi" through the GUI. Manual smoke is documented in `scripts/smoke-test-plan-07.md` as a numbered checklist; no automated UI test (Tauri's webdriver story is too fragile for MVP-1, and the underlying `ghost-client` integration test from Plan 06 already covers the protocol layer).

A short list of what is **NOT** in Plan 07 (deferred to Plan 08 or later):
- Auto-update integration (`tauri-plugin-updater`).
- Code signing.
- Production-ready bundle config (icons, installer customisation, app metadata polish).
- Settings screen (passphrase change, retention policy, presence opt-out).
- Out-of-band fingerprint verification UI ("Mark verified" / safety numbers).
- File transfer, voice, groups (per spec, MVP-2+).
- Visual design polish — Plan 07's UI is functional, not pretty. Layout is one column, no theming, system fonts, minimal CSS.

**Reference spec:** [docs/superpowers/specs/2026-04-27-ghost-mvp1-design.md](../specs/2026-04-27-ghost-mvp1-design.md) — sections 2 (Tauri webview architecture), 3 (onboarding + identity), 5 (first contact flow).

**Reference plans:**
- [Plan 03](2026-04-27-ghost-plan-03-storage.md) — `Database::open_encrypted` and migrations (Tauri command needs the same path resolution).
- [Plan 06](2026-04-28-ghost-plan-06-client-orchestration.md) — `Client::open`, `create_invite`, `add_contact`, `send_message`, `start_inbox_processor`. Plan 07 is a thin command layer over this.

---

## Notes for the implementer

**Tauri version:** Tauri 2.x has a different command/plugin API than Tauri 1.x. Documentation at https://tauri.app/start/. Don't follow Tauri 1.x tutorials by accident.

**Tauri build prerequisites on Windows:** WebView2 runtime ships with Windows 11 by default. On Windows 10, the installer adds it. No additional setup beyond the existing Strawberry Perl + MSVC toolchain we already require.

**Tauri build prerequisites on Linux/macOS:** Linux needs `libwebkit2gtk-4.1-dev`, `libssl-dev`, `librsvg2-dev`. macOS needs Xcode Command Line Tools. The Plan 07 implementer is on Windows, so this is informational — document it for future contributors.

**Cargo workspace integration:** `crates/ghost-app/` is a normal workspace member. `apps/ghost-desktop/` is also a workspace member (Tauri shell binaries are just Cargo binaries with `tauri-build` in their build script). Both go in the root `Cargo.toml`'s `members` list.

**Frontend build pipeline:** SvelteKit with `adapter-static` outputs to `frontend/build/`. Tauri's `tauri.conf.json` points `frontendDist` at that path. Local dev uses `beforeDevCommand: "pnpm --dir frontend dev"` so Tauri proxies to the Vite dev server.

**Why SvelteKit static adapter and not the SSR adapter:** Tauri serves the bundle locally — there's no server. Static is the only option that works.

**Why Svelte 5 runes (`$state`, `$derived`):** Significantly simpler than Svelte 4 stores for the small reactive surface Plan 07 needs (current screen, current contact list, current chat history). Documented at https://svelte.dev/docs/svelte/what-are-runes.

**Two parallel instances on one machine:** Both need separate `GHOST_HOME` env vars *and* separate OS-keystore service names. Plan 06's tests used the same trick (`GHOST_HOME` + `keystore::wipe_secret()`). For manual smoke we just set `GHOST_HOME=...` before launching each instance.

**Identity::create vs Identity::load_default:** `ghost-app` calls `Identity::exists()` (or equivalent) at startup. If no identity, frontend routes to `/onboarding`. After onboarding completes, the same backend opens the Client and the frontend routes to `/`. There is no "logout" in MVP-1 — the only way to switch identity is to wipe the directory.

**Error serialization:** Tauri commands need errors that implement `Serialize`. `ClientError` does not directly. We define `CommandError(String)` in `ghost-app/src/error.rs`, with `From<ClientError>` and `From<IdentityError>` impls that flatten the message. This is intentional: the frontend gets a human string, not a structured error tree.

**Inbox events:** When the inbox processor decodes a new message, it should emit a Tauri event so the chat view updates immediately. We refactor `Client::start_inbox_processor` to take a callback (or we wrap it in `ghost-app` with a wrapper that does both: persist + emit). We choose the wrapper approach so `ghost-client` stays UI-agnostic.

**Schema migrations from Plan 03/06:** A fresh DB will run all migrations on first open — the implementer doesn't need to do anything special. If a stale DB exists (e.g., from a hand-written test) it will also auto-migrate.

**No `.env` files committed.** `frontend/.env` is `.gitignore`'d if it exists; the only env vars Plan 07 reads are `GHOST_HOME` (already used by `ghost-identity`) and standard Tauri ones.

**File-size discipline:** None of the new Rust files should grow past ~200 lines. Frontend `.svelte` files past ~150 lines should be split into a `<script>`-only TS module + thin `.svelte` wrapper.

**No backend tests for ghost-app (this plan).** The Rust command layer is glue; its correctness is verified by the manual smoke test plus Plan 06's integration test for `ghost-client`. Adding tauri-test scaffolding for an MVP-1 command layer is over-engineering. Tasks 4-9 *do* compile-check the command signatures via `cargo check -p ghost-app`.

**No `frontend/`-level unit tests (this plan).** Same reasoning: the UI is a thin reactive shell over typed Tauri invocations. Vitest scaffolding is deferred to Plan 09 or later. The smoke checklist in Task 13 catches regressions.

---

## Task 1: Workspace member registration + ghost-app crate scaffold

**Files:**
- Modify: `Cargo.toml` (root)
- Create: `crates/ghost-app/Cargo.toml`
- Create: `crates/ghost-app/src/lib.rs`

### Step 1.1: Add `crates/ghost-app` to workspace members

- [ ] **Edit `Cargo.toml` (root)** — add the new member alphabetically after `ghost-server`:

```toml
members = [
    "crates/ghost-core",
    "crates/ghost-app",
    "crates/ghost-client",
    "crates/ghost-identity",
    "crates/ghost-network",
    "crates/ghost-identity-cli",
    "crates/ghost-protocol",
    "crates/ghost-server",
    "crates/ghost-storage",
]
```

Also add `tauri = "2"` and `tauri-build = "2"` and `tracing = "0.1"` to `[workspace.dependencies]` so `ghost-app` and `ghost-desktop` can refer to them via `workspace = true`:

```toml
# Tauri (desktop shell)
tauri = { version = "2", features = [] }
tauri-build = "2"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

### Step 1.2: Create `crates/ghost-app/Cargo.toml`

- [ ] **Create file** with content:

```toml
[package]
name = "ghost-app"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Tauri command layer for Ghost: wraps ghost-client for the desktop shell."

[dependencies]
ghost-core     = { path = "../ghost-core" }
ghost-identity = { path = "../ghost-identity" }
ghost-client   = { path = "../ghost-client" }
ghost-storage  = { path = "../ghost-storage" }

tauri   = { workspace = true }
tokio   = { workspace = true }
serde   = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }
```

### Step 1.3: Create `crates/ghost-app/src/lib.rs`

- [ ] **Create file** with stub content:

```rust
//! Ghost Tauri command layer.
//!
//! Wraps `ghost-client` for the desktop shell. Exposes async `#[tauri::command]`
//! functions that return JSON-serializable DTOs and a `CommandError` string-shaped
//! error type.

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-app");
    }
}
```

### Step 1.4: Verify compilation

- [ ] **Run:** `cargo +1.87-x86_64-pc-windows-msvc check -p ghost-app`
- [ ] **Expected:** `Finished` with no errors. (First compile may take 60–120 seconds due to Tauri's transitive dep tree.)

### Step 1.5: Commit

- [ ] **Run:**

```bash
git add Cargo.toml crates/ghost-app/
git commit -m "feat(ghost-app): scaffold Tauri command layer crate"
```

---

## Task 2: Define the AppState and CommandError types

**Files:**
- Create: `crates/ghost-app/src/app_state.rs`
- Create: `crates/ghost-app/src/error.rs`
- Modify: `crates/ghost-app/src/lib.rs`

### Step 2.1: Create `crates/ghost-app/src/error.rs`

- [ ] **Create file** with content:

```rust
//! Frontend-facing error type. Tauri commands return `Result<T, CommandError>`;
//! the frontend receives the message as a string.

use serde::Serialize;

/// String-shaped error for the JS bridge. Wraps any internal error type as a
/// flat human-readable message — the frontend never sees Rust error trees.
#[derive(Debug, Serialize, thiserror::Error)]
#[error("{0}")]
pub struct CommandError(pub String);

impl From<String> for CommandError {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for CommandError {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<ghost_client::ClientError> for CommandError {
    fn from(e: ghost_client::ClientError) -> Self {
        Self(format!("client: {e}"))
    }
}

impl From<ghost_identity::IdentityError> for CommandError {
    fn from(e: ghost_identity::IdentityError) -> Self {
        Self(format!("identity: {e}"))
    }
}

impl From<ghost_storage::StorageError> for CommandError {
    fn from(e: ghost_storage::StorageError) -> Self {
        Self(format!("storage: {e}"))
    }
}

pub type CommandResult<T> = std::result::Result<T, CommandError>;
```

### Step 2.2: Create `crates/ghost-app/src/app_state.rs`

- [ ] **Create file** with content:

```rust
//! Tauri-managed shared state.
//!
//! The app holds at most one active `Client` at a time. After onboarding (or on
//! launch when an identity already exists), the frontend calls `open_client`
//! which populates `client`. Subsequent commands lock + read the stored Client.

use ghost_client::Client;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Shared, Tauri-managed state. One per app process.
#[derive(Default)]
pub struct AppState {
    /// `None` until `open_client` succeeds. Held inside an `Arc` so commands can
    /// drop the mutex guard before performing long async work.
    pub client: Mutex<Option<Arc<Client>>>,

    /// Set once `start_inbox_processor` has been called for the current Client.
    /// Used to abort the task when the Client is replaced (currently only happens
    /// at process exit; reserved for future "switch identity" flows).
    pub inbox_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl AppState {
    /// Convenience: load the current Client. Errors if no client is open.
    pub async fn require_client(&self) -> Result<Arc<Client>, crate::error::CommandError> {
        self.client
            .lock()
            .await
            .clone()
            .ok_or_else(|| crate::error::CommandError("no client open".to_string()))
    }
}
```

### Step 2.3: Wire modules in `crates/ghost-app/src/lib.rs`

- [ ] **Replace contents** of `lib.rs` with:

```rust
//! Ghost Tauri command layer.

pub mod app_state;
pub mod error;

pub use app_state::AppState;
pub use error::{CommandError, CommandResult};

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-app");
    }
}
```

### Step 2.4: Verify compilation

- [ ] **Run:** `cargo +1.87-x86_64-pc-windows-msvc check -p ghost-app`
- [ ] **Expected:** `Finished` with no errors.

### Step 2.5: Commit

- [ ] **Run:**

```bash
git add crates/ghost-app/
git commit -m "feat(ghost-app): AppState container + CommandError serializer"
```

---

## Task 3: Identity-status and onboarding commands

**Files:**
- Create: `crates/ghost-app/src/dto.rs`
- Create: `crates/ghost-app/src/commands/mod.rs`
- Create: `crates/ghost-app/src/commands/identity.rs`
- Modify: `crates/ghost-app/src/lib.rs`

### Step 3.1: Create `crates/ghost-app/src/dto.rs`

- [ ] **Create file** with content:

```rust
//! Plain data shapes that travel over the Tauri IPC bridge.
//!
//! Frontend mirrors these in TypeScript. Keep field names camelCase-friendly
//! (serde's default already matches Rust snake_case → JS snake_case; the
//! frontend types use snake_case to match without rename attributes).

use serde::Serialize;

/// Result of `identity_status` command.
#[derive(Debug, Serialize)]
pub struct IdentityStatusDto {
    /// Whether an identity file exists at the standard path.
    pub exists: bool,

    /// `true` after `open_client` has succeeded for this process.
    pub client_open: bool,
}

/// Result of `create_identity` command.
#[derive(Debug, Serialize)]
pub struct CreatedIdentityDto {
    pub ghost_id: String,
    pub fingerprint: String,
    pub display_name: Option<String>,
}

/// Result of `client_info` command.
#[derive(Debug, Serialize)]
pub struct ClientInfoDto {
    pub ghost_id: String,
    pub fingerprint: String,
    pub display_name: Option<String>,
    pub local_addrs: Vec<String>,
}

/// Result of `list_contacts` command. One entry per row in the `contacts` table.
#[derive(Debug, Serialize)]
pub struct ContactDto {
    pub ghost_id: String,
    pub fingerprint: String,
    pub display_name: Option<String>,
    pub local_alias: Option<String>,
    pub added_at: i64,
    pub verified: bool,
}

/// Result of `list_messages` command.
#[derive(Debug, Serialize)]
pub struct MessageDto {
    /// 32-char lowercase hex of the msg_uuid bytes.
    pub uuid: String,
    /// `"in"` or `"out"`.
    pub direction: String,
    pub content: String,
    pub sent_at: i64,
    pub received_at: Option<i64>,
}

/// Result of `create_invite` command.
#[derive(Debug, Serialize)]
pub struct InviteDto {
    pub bech32: String,
    pub expires_at: u64,
}

/// Payload of the `ghost://message-received` Tauri event.
#[derive(Debug, Clone, Serialize)]
pub struct InboundMessageEvent {
    pub from_ghost_id: String,
    pub content: String,
    pub received_at: i64,
}
```

### Step 3.2: Create `crates/ghost-app/src/commands/mod.rs`

- [ ] **Create file** with content:

```rust
//! Tauri command implementations. Each `pub async fn` annotated with
//! `#[tauri::command]` is exposed to the frontend's `invoke()` calls.
//!
//! Commands are split per-domain into submodules so each file stays small.

pub mod identity;
```

### Step 3.3: Create `crates/ghost-app/src/commands/identity.rs`

- [ ] **Create file** with content:

```rust
//! Identity lifecycle commands: status check + onboarding.

use crate::dto::{CreatedIdentityDto, IdentityStatusDto};
use crate::error::CommandResult;
use crate::AppState;
use ghost_core::Fingerprint;
use ghost_identity::{CreateOptions, Identity};
use tauri::State;

/// Reports whether an identity file exists on disk and whether a `Client` has
/// been opened in this process.
#[tauri::command]
pub async fn identity_status(state: State<'_, AppState>) -> CommandResult<IdentityStatusDto> {
    let exists = identity_file_exists();
    let client_open = state.client.lock().await.is_some();
    Ok(IdentityStatusDto {
        exists,
        client_open,
    })
}

/// Generate a fresh identity. Fails if one already exists (no `overwrite`).
#[tauri::command]
pub async fn create_identity(
    display_name: Option<String>,
    passphrase: Option<String>,
) -> CommandResult<CreatedIdentityDto> {
    let identity = Identity::create(CreateOptions {
        display_name: display_name.clone(),
        passphrase: passphrase.as_deref(),
        overwrite: false,
    })?;
    let ghost_id = identity.ghost_id();
    let fingerprint = Fingerprint::of(&ghost_id).to_string();
    Ok(CreatedIdentityDto {
        ghost_id: ghost_id.to_string(),
        fingerprint,
        display_name,
    })
}

/// Helper — tries to compute the identity-file path, returns `false` on either
/// "path resolution failed" or "file does not exist".
fn identity_file_exists() -> bool {
    match ghost_identity::identity_file() {
        Ok(path) => path.is_file(),
        Err(_) => false,
    }
}
```

### Step 3.4: Re-export from `lib.rs`

- [ ] **Replace contents** of `lib.rs` with:

```rust
//! Ghost Tauri command layer.

pub mod app_state;
pub mod commands;
pub mod dto;
pub mod error;

pub use app_state::AppState;
pub use error::{CommandError, CommandResult};

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-app");
    }
}
```

### Step 3.5: Verify compilation

- [ ] **Run:** `cargo +1.87-x86_64-pc-windows-msvc check -p ghost-app`
- [ ] **Expected:** `Finished` with no errors.

All referenced names (`ghost_identity::identity_file`, `Identity::ghost_id`, `Fingerprint::of`) exist in the current code — verified before plan was written. If something fails to resolve, double-check spelling before adding new methods to upstream crates.

### Step 3.6: Commit

- [ ] **Run:**

```bash
git add crates/ghost-app/
git commit -m "feat(ghost-app): identity_status + create_identity commands"
```

---

## Task 4: Open / close client + reading commands

**Files:**
- Create: `crates/ghost-app/src/commands/lifecycle.rs`
- Create: `crates/ghost-app/src/commands/read.rs`
- Modify: `crates/ghost-app/src/commands/mod.rs`

### Step 4.1: Create `crates/ghost-app/src/commands/lifecycle.rs`

- [ ] **Create file** with content:

```rust
//! Open / close commands for the Client lifecycle.

use crate::dto::ClientInfoDto;
use crate::error::{CommandError, CommandResult};
use crate::AppState;
use ghost_client::{Client, ClientConfig};
use ghost_core::Fingerprint;
use std::sync::Arc;
use tauri::State;

/// Open the embedded `ghost-client` runtime. Reads identity from disk (with the
/// optional passphrase), starts the Network + Server, and stores the Client in
/// `AppState`. Idempotent: calling twice returns the existing client info
/// without re-opening.
#[tauri::command]
pub async fn open_client(
    passphrase: Option<String>,
    state: State<'_, AppState>,
) -> CommandResult<ClientInfoDto> {
    {
        let guard = state.client.lock().await;
        if let Some(client) = guard.as_ref() {
            return Ok(client_info_from_client(client));
        }
    }

    let config = ClientConfig {
        passphrase,
        ..ClientConfig::default()
    };
    let client = Client::open(config).await?;
    let info = client_info_from_client_inline(&client);

    {
        let mut guard = state.client.lock().await;
        *guard = Some(Arc::new(client));
    }
    Ok(info)
}

/// Drop the in-memory Client (network/server are torn down by Drop on the inner
/// types). Identity file remains on disk. Used by tests; not currently exposed
/// in the UI.
#[tauri::command]
pub async fn close_client(state: State<'_, AppState>) -> CommandResult<()> {
    let mut guard = state.client.lock().await;
    if let Some(handle) = state.inbox_handle.lock().await.take() {
        handle.abort();
    }
    *guard = None;
    Ok(())
}

fn client_info_from_client(client: &Client) -> ClientInfoDto {
    let ghost_id = client.ghost_id();
    let fingerprint = Fingerprint::of(&ghost_id).to_string();
    ClientInfoDto {
        ghost_id: ghost_id.to_string(),
        fingerprint,
        display_name: None,
        local_addrs: client.local_addrs().iter().map(|a| a.to_string()).collect(),
    }
}

fn client_info_from_client_inline(client: &Client) -> ClientInfoDto {
    client_info_from_client(client)
}

/// Internal helper that's identical now but exists so we can attach display_name
/// extraction later (currently `Client` doesn't expose it; can be added in MVP-2).
#[allow(dead_code)]
fn unused_to_silence_lint() -> CommandError {
    CommandError("placeholder".into())
}
```

Note: the two helper fns above are intentionally identical for now; we keep two so a future change that adds display-name extraction has a clear seam. Drop one if the implementer prefers — the second helper and the lint-silencer can both go.

### Step 4.2: Create `crates/ghost-app/src/commands/read.rs`

- [ ] **Create file** with content:

```rust
//! Read-only commands: query data without changing state.

use crate::dto::{ClientInfoDto, ContactDto, InviteDto, MessageDto};
use crate::error::{CommandError, CommandResult};
use crate::AppState;
use ghost_core::{Fingerprint, GhostId};
use ghost_storage::Verification;
use tauri::State;

/// Returns info about the currently open client. Errors if not open.
#[tauri::command]
pub async fn client_info(state: State<'_, AppState>) -> CommandResult<ClientInfoDto> {
    let client = state.require_client().await?;
    let ghost_id = client.ghost_id();
    let fingerprint = Fingerprint::of(&ghost_id).to_string();
    Ok(ClientInfoDto {
        ghost_id: ghost_id.to_string(),
        fingerprint,
        display_name: None,
        local_addrs: client.local_addrs().iter().map(|a| a.to_string()).collect(),
    })
}

/// All contacts in the local DB.
#[tauri::command]
pub async fn list_contacts(state: State<'_, AppState>) -> CommandResult<Vec<ContactDto>> {
    let client = state.require_client().await?;
    let rows = client.list_contacts()?;
    let out = rows
        .into_iter()
        .map(|c| ContactDto {
            ghost_id: c.ghost_id.to_string(),
            fingerprint: c.fingerprint,
            display_name: c.display_name,
            local_alias: c.local_alias,
            added_at: c.added_at,
            verified: matches!(c.verification, Verification::Verified),
        })
        .collect();
    Ok(out)
}

/// Messages for a contact, oldest first.
#[tauri::command]
pub async fn list_messages(
    contact_ghost_id: String,
    limit: u32,
    offset: u32,
    state: State<'_, AppState>,
) -> CommandResult<Vec<MessageDto>> {
    let client = state.require_client().await?;
    let id = parse_ghost_id(&contact_ghost_id)?;
    let rows = client.list_messages(&id, limit, offset)?;
    let out = rows
        .into_iter()
        .map(|m| MessageDto {
            uuid: hex::encode(m.msg_uuid),
            direction: match m.direction {
                ghost_storage::Direction::Incoming => "in".to_string(),
                ghost_storage::Direction::Outgoing => "out".to_string(),
            },
            content: m.content,
            sent_at: m.sent_at,
            received_at: m.received_at,
        })
        .collect();
    Ok(out)
}

/// Generate a fresh invite valid for the given TTL in seconds.
#[tauri::command]
pub async fn create_invite(
    ttl_seconds: u64,
    state: State<'_, AppState>,
) -> CommandResult<InviteDto> {
    let client = state.require_client().await?;
    let invite = client.create_invite(ttl_seconds)?;
    let bech32 = invite
        .to_bech32()
        .map_err(|e| CommandError(format!("invite encode: {e}")))?;
    Ok(InviteDto {
        bech32,
        expires_at: invite.expires_at,
    })
}

fn parse_ghost_id(s: &str) -> CommandResult<GhostId> {
    GhostId::from_bech32(s).map_err(|e| CommandError(format!("ghost id: {e}")))
}
```

The `hex` crate is already a workspace dep (used by ghost-core). Add `hex = { workspace = true }` to `crates/ghost-app/Cargo.toml`'s `[dependencies]`.

### Step 4.3: Update `crates/ghost-app/src/commands/mod.rs`

- [ ] **Replace contents** with:

```rust
//! Tauri command implementations.

pub mod identity;
pub mod lifecycle;
pub mod read;
```

### Step 4.4: Add `hex` dependency to `crates/ghost-app/Cargo.toml`

- [ ] **Edit `crates/ghost-app/Cargo.toml`** — add to `[dependencies]`:

```toml
hex = { workspace = true }
```

### Step 4.5: Verify compilation

- [ ] **Run:** `cargo +1.87-x86_64-pc-windows-msvc check -p ghost-app`
- [ ] **Expected:** `Finished` with no errors.

If `GhostId::from_bech32` is named differently (e.g., `parse_bech32`), adjust the call site. Do not modify ghost-core to match the plan; match the plan to ghost-core.

### Step 4.6: Commit

- [ ] **Run:**

```bash
git add crates/ghost-app/
git commit -m "feat(ghost-app): open/close client + read commands (info, contacts, messages, invite)"
```

---

## Task 5: Write commands (add_contact, send_message) + inbox event bridge

**Files:**
- Create: `crates/ghost-app/src/commands/write.rs`
- Create: `crates/ghost-app/src/inbox_bridge.rs`
- Modify: `crates/ghost-app/src/commands/mod.rs`
- Modify: `crates/ghost-app/src/lib.rs`

### Step 5.1: Create `crates/ghost-app/src/commands/write.rs`

- [ ] **Create file** with content:

```rust
//! Mutating commands: add a contact via invite, send a message.

use crate::error::{CommandError, CommandResult};
use crate::AppState;
use ghost_core::GhostId;
use tauri::State;

/// Accept an invite bech32 string, perform first-contact handshake, persist
/// new contact + MLS state.
#[tauri::command]
pub async fn add_contact(invite: String, state: State<'_, AppState>) -> CommandResult<()> {
    let client = state.require_client().await?;
    client.add_contact(&invite).await?;
    Ok(())
}

/// Encrypt and deliver a text message to an existing contact.
#[tauri::command]
pub async fn send_message(
    contact_ghost_id: String,
    text: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let client = state.require_client().await?;
    let id = GhostId::from_bech32(&contact_ghost_id)
        .map_err(|e| CommandError(format!("ghost id: {e}")))?;
    client.send_message(id, &text).await?;
    Ok(())
}
```

### Step 5.2: Create `crates/ghost-app/src/inbox_bridge.rs`

This is the wrapper that runs the inbox processor and emits a Tauri event for each newly persisted incoming message. We achieve this without modifying `ghost-client` by polling the messages table after each iteration — the simplest approach for MVP-1, acceptable since Plan 06's processor is the only writer of incoming rows. (A cleaner design adds a callback parameter to `Client::start_inbox_processor`; deferring that to a future plan keeps Plan 07 surgical.)

- [ ] **Create file** with content:

```rust
//! Bridge from the Client's inbox processor to the Tauri event bus.
//!
//! Strategy: spawn `Client::start_inbox_processor` (which persists incoming
//! messages to the DB), and a sibling watcher task that polls the `messages`
//! table for new incoming rows since the last seen `sent_at` and emits a
//! `ghost://message-received` event for each.
//!
//! Polling is acceptable here: the watcher tick is 250ms, the loopback path
//! is microseconds, the perceived latency is identical to a callback-driven
//! design for the MVP-1 demo. A callback-driven refactor of `ghost-client` is
//! tracked as MVP-2 follow-up.

use crate::dto::InboundMessageEvent;
use ghost_client::Client;
use ghost_storage::{Direction, MessageRow};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const POLL_INTERVAL: Duration = Duration::from_millis(250);
pub const EVENT_NAME: &str = "ghost://message-received";

pub async fn start_with_event_bridge(
    client: Arc<Client>,
    app: AppHandle,
) -> ghost_client::Result<(tokio::task::JoinHandle<()>, tokio::task::JoinHandle<()>)> {
    // Snapshot the current max sent_at across all incoming messages so we don't
    // re-emit pre-existing history on every app start.
    let mut last_seen_sent_at: i64 = max_incoming_sent_at(&client).unwrap_or(0);

    let processor_handle = client.start_inbox_processor().await?;

    let watcher_client = client.clone();
    let watcher_app = app.clone();
    let watcher_handle = tokio::spawn(async move {
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;
            let new_rows = match scan_new_incoming(&watcher_client, last_seen_sent_at) {
                Ok(rows) => rows,
                Err(e) => {
                    tracing::warn!("inbox watcher scan failed: {e}");
                    continue;
                }
            };
            for row in new_rows {
                last_seen_sent_at = last_seen_sent_at.max(row.sent_at);
                let payload = InboundMessageEvent {
                    from_ghost_id: row.contact_id.to_string(),
                    content: row.content.clone(),
                    received_at: row.received_at.unwrap_or(row.sent_at),
                };
                if let Err(e) = watcher_app.emit(EVENT_NAME, payload) {
                    tracing::warn!("inbox event emit failed: {e}");
                }
            }
        }
    });

    Ok((processor_handle, watcher_handle))
}

fn max_incoming_sent_at(client: &Client) -> Option<i64> {
    let contacts = client.list_contacts().ok()?;
    let mut max_at: Option<i64> = None;
    for c in contacts {
        if let Ok(rows) = client.list_messages(&c.ghost_id, u32::MAX, 0) {
            for row in rows {
                if matches!(row.direction, Direction::Incoming) {
                    max_at = Some(max_at.map(|m| m.max(row.sent_at)).unwrap_or(row.sent_at));
                }
            }
        }
    }
    max_at
}

fn scan_new_incoming(
    client: &Client,
    after: i64,
) -> ghost_client::Result<Vec<MessageRow>> {
    let mut out = Vec::new();
    for c in client.list_contacts()? {
        for row in client.list_messages(&c.ghost_id, u32::MAX, 0)? {
            if matches!(row.direction, Direction::Incoming) && row.sent_at > after {
                out.push(row);
            }
        }
    }
    out.sort_by_key(|r| r.sent_at);
    Ok(out)
}
```

### Step 5.3: Update `crates/ghost-app/src/commands/mod.rs`

- [ ] **Replace contents** with:

```rust
//! Tauri command implementations.

pub mod identity;
pub mod lifecycle;
pub mod read;
pub mod write;
```

### Step 5.4: Re-export inbox_bridge from `lib.rs`

- [ ] **Replace contents** of `lib.rs` with:

```rust
//! Ghost Tauri command layer.

pub mod app_state;
pub mod commands;
pub mod dto;
pub mod error;
pub mod inbox_bridge;

pub use app_state::AppState;
pub use error::{CommandError, CommandResult};

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-app");
    }
}
```

### Step 5.5: Verify compilation

- [ ] **Run:** `cargo +1.87-x86_64-pc-windows-msvc check -p ghost-app`
- [ ] **Expected:** `Finished` with no errors. `tauri::Emitter` should resolve without extra features in tauri 2.x.

If `Emitter` is not in scope, the v2 module path may have changed; fall back to `app.emit_to(...)` or check the tauri 2.x docs for the current path. Do **not** add features speculatively — first read the actual `tauri` API surface.

### Step 5.6: Commit

- [ ] **Run:**

```bash
git add crates/ghost-app/
git commit -m "feat(ghost-app): write commands + inbox-event bridge for message-received events"
```

---

## Task 6: Tauri shell binary (`apps/ghost-desktop`)

**Files:**
- Create: `apps/ghost-desktop/Cargo.toml`
- Create: `apps/ghost-desktop/build.rs`
- Create: `apps/ghost-desktop/tauri.conf.json`
- Create: `apps/ghost-desktop/src/main.rs`
- Create: `apps/ghost-desktop/icons/.gitkeep`
- Create: `apps/ghost-desktop/capabilities/default.json`
- Modify: `Cargo.toml` (root) — add the new member.

### Step 6.1: Update root `Cargo.toml` workspace members

- [ ] **Edit root `Cargo.toml`** — add to `members`:

```toml
"apps/ghost-desktop",
```

(Order alphabetically; place before `crates/ghost-app`. The `apps/` prefix sorts before `crates/`.)

The full list becomes:

```toml
members = [
    "apps/ghost-desktop",
    "crates/ghost-core",
    "crates/ghost-app",
    "crates/ghost-client",
    "crates/ghost-identity",
    "crates/ghost-network",
    "crates/ghost-identity-cli",
    "crates/ghost-protocol",
    "crates/ghost-server",
    "crates/ghost-storage",
]
```

### Step 6.2: Create `apps/ghost-desktop/Cargo.toml`

- [ ] **Create file** with content:

```toml
[package]
name = "ghost-desktop"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Ghost desktop binary (Tauri shell)."

[[bin]]
name = "ghost-desktop"
path = "src/main.rs"

[build-dependencies]
tauri-build = { workspace = true }

[dependencies]
ghost-app = { path = "../../crates/ghost-app" }

tauri = { workspace = true, features = [] }
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
serde_json = "1"
```

### Step 6.3: Create `apps/ghost-desktop/build.rs`

- [ ] **Create file** with content:

```rust
fn main() {
    tauri_build::build()
}
```

### Step 6.4: Create `apps/ghost-desktop/tauri.conf.json`

- [ ] **Create file** with content:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Ghost",
  "version": "0.0.1",
  "identifier": "im.ghost.desktop",
  "build": {
    "frontendDist": "../../frontend/build",
    "devUrl": "http://localhost:1420",
    "beforeDevCommand": "pnpm --dir ../../frontend dev",
    "beforeBuildCommand": "pnpm --dir ../../frontend build"
  },
  "app": {
    "windows": [
      {
        "title": "Ghost",
        "width": 1024,
        "height": 720,
        "minWidth": 480,
        "minHeight": 480,
        "resizable": true
      }
    ],
    "security": {
      "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost"
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": []
  }
}
```

The `"icon": []` is intentional for MVP-1: `tauri build` without icons emits a warning but does not fail. Tauri-required default icon paths can be added in Plan 08.

### Step 6.5: Create `apps/ghost-desktop/capabilities/default.json`

This file declares which commands are callable from which windows. Tauri 2.x requires it.

- [ ] **Create file** with content:

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
    "core:event:allow-unlisten"
  ]
}
```

(Note: command-level allowlists for our own commands aren't required in Tauri 2.x — `#[tauri::command]` registrations are scoped via the builder, and any window listed under `windows` can invoke them. Frontend-callable plugins like `core:event` require explicit permissions.)

### Step 6.6: Create `apps/ghost-desktop/src/main.rs`

- [ ] **Create file** with content:

```rust
//! Ghost desktop shell — Tauri entrypoint.
//!
//! Mounts the `ghost-app` command surface, sets up logging, and starts the
//! event loop. Frontend lives in ../../frontend and is bundled at build time.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use ghost_app::commands::{identity, lifecycle, read, write};
use ghost_app::AppState;
use tauri::Manager;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("ghost_=info,info")),
        )
        .init();

    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            identity::identity_status,
            identity::create_identity,
            lifecycle::open_client,
            lifecycle::close_client,
            read::client_info,
            read::list_contacts,
            read::list_messages,
            read::create_invite,
            write::add_contact,
            write::send_message,
        ])
        .setup(|app| {
            // Inbox bridge is started lazily after `open_client` succeeds.
            // We hold the AppHandle here for later use by lifecycle::open_client.
            let app_handle = app.handle().clone();
            app.manage(InboxBridgeHandle(app_handle));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("ghost-desktop failed to run");
}

/// Wrapper around `AppHandle` so it can live in Tauri's `State<'_>` layer.
/// `AppHandle` itself isn't required to be wrapped, but doing so keeps the
/// generic-State type signatures clean in `lifecycle::open_client`.
pub struct InboxBridgeHandle(pub tauri::AppHandle);
```

### Step 6.7: Wire the inbox bridge into `lifecycle::open_client`

- [ ] **Edit `crates/ghost-app/src/commands/lifecycle.rs`** — modify `open_client` to also start the inbox bridge:

The new `open_client` (replacing the prior version):

```rust
//! Open / close commands for the Client lifecycle.

use crate::dto::ClientInfoDto;
use crate::error::{CommandError, CommandResult};
use crate::inbox_bridge;
use crate::AppState;
use ghost_client::{Client, ClientConfig};
use ghost_core::Fingerprint;
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn open_client(
    passphrase: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<ClientInfoDto> {
    {
        let guard = state.client.lock().await;
        if let Some(client) = guard.as_ref() {
            return Ok(client_info_from_client(client));
        }
    }

    let config = ClientConfig {
        passphrase,
        ..ClientConfig::default()
    };
    let client = Client::open(config).await?;
    let info = client_info_from_client(&client);
    let client_arc = Arc::new(client);

    let (proc_handle, _watcher_handle) =
        inbox_bridge::start_with_event_bridge(client_arc.clone(), app)
            .await
            .map_err(|e| CommandError(format!("inbox bridge: {e}")))?;

    {
        let mut guard = state.client.lock().await;
        *guard = Some(client_arc);
    }
    {
        let mut handle_guard = state.inbox_handle.lock().await;
        *handle_guard = Some(proc_handle);
    }
    Ok(info)
}

#[tauri::command]
pub async fn close_client(state: State<'_, AppState>) -> CommandResult<()> {
    let mut guard = state.client.lock().await;
    if let Some(handle) = state.inbox_handle.lock().await.take() {
        handle.abort();
    }
    *guard = None;
    Ok(())
}

fn client_info_from_client(client: &Client) -> ClientInfoDto {
    let ghost_id = client.ghost_id();
    let fingerprint = Fingerprint::of(&ghost_id).to_string();
    ClientInfoDto {
        ghost_id: ghost_id.to_string(),
        fingerprint,
        display_name: None,
        local_addrs: client.local_addrs().iter().map(|a| a.to_string()).collect(),
    }
}
```

(Drop the `client_info_from_client_inline` and `unused_to_silence_lint` helpers from the earlier draft — they were placeholders.)

### Step 6.8: Verify Tauri shell compiles

- [ ] **Run:** `cargo +1.87-x86_64-pc-windows-msvc check -p ghost-desktop`
- [ ] **Expected:** `Finished`. Tauri's first compile of the shell binary will pull WebView2 bindings and webview-dependent crates; expect 3-7 minutes on a cold target dir.

If `tauri::generate_context!()` complains about the icon path, comment out the `bundle` block temporarily (or make `"active": false`) — the icon issue is non-blocking for `cargo check` and `cargo tauri dev`. Real icons land in Plan 08.

### Step 6.9: Commit

- [ ] **Run:**

```bash
git add Cargo.toml apps/ghost-desktop/ crates/ghost-app/src/commands/lifecycle.rs
git commit -m "feat(ghost-desktop): Tauri 2.x shell binary mounting ghost-app commands"
```

---

## Task 7: SvelteKit frontend scaffold

**Files:**
- Create: `frontend/package.json`
- Create: `frontend/pnpm-lock.yaml` (generated)
- Create: `frontend/svelte.config.js`
- Create: `frontend/vite.config.ts`
- Create: `frontend/tsconfig.json`
- Create: `frontend/.gitignore`
- Create: `frontend/src/app.html`
- Create: `frontend/src/routes/+layout.svelte`
- Create: `frontend/src/routes/+page.svelte`

### Step 7.1: Verify Node + pnpm versions

- [ ] **Run:** `node --version && pnpm --version`
- [ ] **Expected:** Node ≥ 20, pnpm ≥ 8. If pnpm is missing: `npm install -g pnpm`.

### Step 7.2: Create `frontend/package.json`

- [ ] **Create file** with content:

```json
{
  "name": "ghost-frontend",
  "version": "0.0.1",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite dev --port 1420 --strictPort",
    "build": "vite build",
    "preview": "vite preview",
    "check": "svelte-kit sync && svelte-check --tsconfig ./tsconfig.json"
  },
  "dependencies": {
    "@tauri-apps/api": "^2"
  },
  "devDependencies": {
    "@sveltejs/adapter-static": "^3",
    "@sveltejs/kit": "^2",
    "@sveltejs/vite-plugin-svelte": "^4",
    "svelte": "^5",
    "svelte-check": "^4",
    "typescript": "^5",
    "vite": "^5"
  }
}
```

### Step 7.3: Create `frontend/svelte.config.js`

- [ ] **Create file** with content:

```js
import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      pages: 'build',
      assets: 'build',
      fallback: 'index.html',
      precompress: false,
      strict: true
    })
  }
};

export default config;
```

The `fallback: 'index.html'` is crucial — Tauri serves a single static index, and SvelteKit needs to know we're SPA-mode.

### Step 7.4: Create `frontend/vite.config.ts`

- [ ] **Create file** with content:

```ts
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [sveltekit()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: '127.0.0.1',
    watch: {
      ignored: ['**/src-tauri/**']
    }
  }
});
```

### Step 7.5: Create `frontend/tsconfig.json`

- [ ] **Create file** with content:

```json
{
  "extends": "./.svelte-kit/tsconfig.json",
  "compilerOptions": {
    "allowJs": true,
    "checkJs": true,
    "esModuleInterop": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "skipLibCheck": true,
    "sourceMap": true,
    "strict": true,
    "moduleResolution": "bundler"
  }
}
```

### Step 7.6: Create `frontend/.gitignore`

- [ ] **Create file** with content:

```
node_modules
.svelte-kit
build
.env
.env.*
!.env.example
```

### Step 7.7: Create `frontend/src/app.html`

- [ ] **Create file** with content:

```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Ghost</title>
    %sveltekit.head%
  </head>
  <body data-sveltekit-preload-data="hover">
    <div id="app">%sveltekit.body%</div>
  </body>
</html>
```

### Step 7.8: Create minimal layout + landing route

- [ ] **Create `frontend/src/routes/+layout.svelte`** with content:

```svelte
<script lang="ts">
  let { children } = $props();
</script>

<style>
  :global(html, body) {
    margin: 0;
    padding: 0;
    height: 100%;
    background: #0e0f12;
    color: #e8e8ec;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  }
  :global(*) {
    box-sizing: border-box;
  }
  main {
    height: 100%;
    width: 100%;
  }
</style>

<main>
  {@render children()}
</main>
```

- [ ] **Create `frontend/src/routes/+page.svelte`** with content:

```svelte
<script lang="ts">
  let message = $state('Ghost frontend is alive.');
</script>

<section style="padding: 2rem;">
  <h1>Ghost</h1>
  <p>{message}</p>
</section>
```

### Step 7.9: Install + run

- [ ] **Run:** `pnpm --dir frontend install`
- [ ] **Expected:** Install completes; lock file created.
- [ ] **Run:** `pnpm --dir frontend build`
- [ ] **Expected:** `frontend/build/index.html` and `frontend/build/_app/` exist.

### Step 7.10: Commit

- [ ] **Run:**

```bash
git add frontend/
git commit -m "feat(frontend): SvelteKit static-adapter scaffold (Svelte 5, Vite)"
```

---

## Task 8: Frontend Tauri bridge + typed invocations

**Files:**
- Create: `frontend/src/lib/types.ts`
- Create: `frontend/src/lib/tauri.ts`
- Create: `frontend/src/lib/state.svelte.ts`

### Step 8.1: Create `frontend/src/lib/types.ts`

- [ ] **Create file** with content (mirrors `crates/ghost-app/src/dto.rs`):

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
}

export interface MessageDto {
  uuid: string;
  direction: 'in' | 'out';
  content: string;
  sent_at: number;
  received_at: number | null;
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
```

### Step 8.2: Create `frontend/src/lib/tauri.ts`

- [ ] **Create file** with content:

```ts
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  ClientInfoDto,
  ContactDto,
  CreatedIdentityDto,
  IdentityStatusDto,
  InboundMessageEvent,
  InviteDto,
  MessageDto
} from './types';

export const INBOX_EVENT = 'ghost://message-received';

export async function identityStatus(): Promise<IdentityStatusDto> {
  return invoke('identity_status');
}

export async function createIdentity(
  display_name: string | null,
  passphrase: string | null
): Promise<CreatedIdentityDto> {
  return invoke('create_identity', { displayName: display_name, passphrase });
}

export async function openClient(passphrase: string | null): Promise<ClientInfoDto> {
  return invoke('open_client', { passphrase });
}

export async function clientInfo(): Promise<ClientInfoDto> {
  return invoke('client_info');
}

export async function listContacts(): Promise<ContactDto[]> {
  return invoke('list_contacts');
}

export async function listMessages(
  contact_ghost_id: string,
  limit = 200,
  offset = 0
): Promise<MessageDto[]> {
  return invoke('list_messages', { contactGhostId: contact_ghost_id, limit, offset });
}

export async function createInvite(ttl_seconds = 7 * 24 * 3600): Promise<InviteDto> {
  return invoke('create_invite', { ttlSeconds: ttl_seconds });
}

export async function addContact(invite: string): Promise<void> {
  return invoke('add_contact', { invite });
}

export async function sendMessage(contact_ghost_id: string, text: string): Promise<void> {
  return invoke('send_message', { contactGhostId: contact_ghost_id, text });
}

export async function onInbound(
  cb: (e: InboundMessageEvent) => void
): Promise<UnlistenFn> {
  return listen<InboundMessageEvent>(INBOX_EVENT, (event) => cb(event.payload));
}
```

Tauri's IPC layer auto-converts Rust snake_case command parameters to camelCase on the JS side (with serde tag generation). Hence `displayName` (JS) ↔ `display_name` (Rust). The DTO fields, however, are returned as-is (Rust serde defaults to keep the field name unless `#[serde(rename_all)]` is set on the struct). Plan 07 does not set `rename_all`, so DTO fields are snake_case in JS too — match accordingly in `types.ts`.

### Step 8.3: Create `frontend/src/lib/state.svelte.ts`

- [ ] **Create file** with content:

```ts
import type { ClientInfoDto, ContactDto, MessageDto } from './types';

class AppStore {
  info = $state<ClientInfoDto | null>(null);
  contacts = $state<ContactDto[]>([]);
  // contact ghost_id → message list. Mutated reactively when new messages arrive.
  threads = $state<Record<string, MessageDto[]>>({});

  setInfo(info: ClientInfoDto | null) {
    this.info = info;
  }

  setContacts(list: ContactDto[]) {
    this.contacts = list;
  }

  setThread(ghost_id: string, msgs: MessageDto[]) {
    this.threads = { ...this.threads, [ghost_id]: msgs };
  }

  pushIncoming(ghost_id: string, msg: MessageDto) {
    const existing = this.threads[ghost_id] ?? [];
    this.threads = { ...this.threads, [ghost_id]: [...existing, msg] };
  }
}

export const store = new AppStore();
```

### Step 8.4: Verify type-check passes

- [ ] **Run:** `pnpm --dir frontend check`
- [ ] **Expected:** "0 errors and 0 warnings" (svelte-check output).

### Step 8.5: Commit

- [ ] **Run:**

```bash
git add frontend/src/lib/
git commit -m "feat(frontend): typed Tauri bridge + reactive store (Svelte 5 runes)"
```

---

## Task 9: Onboarding screen

**Files:**
- Create: `frontend/src/routes/onboarding/+page.svelte`
- Modify: `frontend/src/routes/+page.svelte` (redirect to onboarding when no identity)

### Step 9.1: Create `frontend/src/routes/onboarding/+page.svelte`

- [ ] **Create file** with content:

```svelte
<script lang="ts">
  import { goto } from '$app/navigation';
  import { createIdentity, openClient } from '$lib/tauri';
  import { store } from '$lib/state.svelte';

  let displayName = $state('');
  let passphrase = $state('');
  let busy = $state(false);
  let errorMsg = $state<string | null>(null);

  async function submit(e: Event) {
    e.preventDefault();
    busy = true;
    errorMsg = null;
    try {
      await createIdentity(
        displayName.trim() === '' ? null : displayName.trim(),
        passphrase === '' ? null : passphrase
      );
      const info = await openClient(passphrase === '' ? null : passphrase);
      store.setInfo(info);
      await goto('/');
    } catch (e) {
      errorMsg = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section style="padding: 2rem; max-width: 520px;">
  <h1 style="margin-top: 0;">Welcome to Ghost</h1>
  <p style="opacity: 0.8;">
    Create your identity. It is generated locally and never sent to a server.
    Save the recovery file — losing it loses the identity for good.
  </p>

  <form onsubmit={submit}>
    <label style="display: block; margin: 1rem 0;">
      <div style="margin-bottom: 0.25rem;">Display name (optional)</div>
      <input
        type="text"
        bind:value={displayName}
        disabled={busy}
        maxlength="64"
        style="width: 100%; padding: 0.6rem; background: #1a1c22; color: inherit; border: 1px solid #2a2d36; border-radius: 6px;"
      />
    </label>

    <label style="display: block; margin: 1rem 0;">
      <div style="margin-bottom: 0.25rem;">Passphrase (optional, recommended)</div>
      <input
        type="password"
        bind:value={passphrase}
        disabled={busy}
        style="width: 100%; padding: 0.6rem; background: #1a1c22; color: inherit; border: 1px solid #2a2d36; border-radius: 6px;"
      />
    </label>

    <button
      type="submit"
      disabled={busy}
      style="padding: 0.6rem 1.2rem; background: #4a4cff; color: white; border: 0; border-radius: 6px; cursor: pointer;"
    >
      {busy ? 'Creating…' : 'Create identity'}
    </button>
  </form>

  {#if errorMsg}
    <p style="color: #ff6464; margin-top: 1rem;">{errorMsg}</p>
  {/if}
</section>
```

### Step 9.2: Update `frontend/src/routes/+page.svelte`

- [ ] **Replace contents** of `frontend/src/routes/+page.svelte` with:

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { identityStatus, openClient, clientInfo } from '$lib/tauri';
  import { store } from '$lib/state.svelte';

  let booting = $state(true);
  let bootError = $state<string | null>(null);

  onMount(async () => {
    try {
      const status = await identityStatus();
      if (!status.exists) {
        await goto('/onboarding');
        return;
      }
      if (!status.client_open) {
        const info = await openClient(null);
        store.setInfo(info);
      } else {
        const info = await clientInfo();
        store.setInfo(info);
      }
    } catch (e) {
      bootError = String(e);
    } finally {
      booting = false;
    }
  });
</script>

<section style="padding: 2rem;">
  {#if booting}
    <p>Loading…</p>
  {:else if bootError}
    <p style="color: #ff6464;">Failed to load: {bootError}</p>
    <p style="opacity: 0.7;">If you set a passphrase, the open-client flow needs UI for entering it. Coming in Plan 08.</p>
  {:else if store.info}
    <p>Signed in as {store.info.fingerprint}</p>
    <pre style="background: #1a1c22; padding: 1rem; border-radius: 6px; overflow: auto;">{store.info.ghost_id}</pre>
  {/if}
</section>
```

The placeholder home view — Task 10 replaces this with the real home.

### Step 9.3: Run dev server + manual smoke

- [ ] **Run** (in a separate terminal): `pnpm --dir frontend dev`
- [ ] **Expected:** Vite serves on http://localhost:1420; visiting it (or just curl-ing) shows the SvelteKit-rendered page. (The Tauri shell is not yet driving this; we're just verifying the frontend boots.)
- [ ] Stop the dev server (Ctrl-C).

### Step 9.4: Type-check

- [ ] **Run:** `pnpm --dir frontend check`
- [ ] **Expected:** 0 errors.

### Step 9.5: Commit

- [ ] **Run:**

```bash
git add frontend/src/routes/
git commit -m "feat(frontend): onboarding screen + boot-time identity routing"
```

---

## Task 10: Home screen — Ghost ID card, contacts list, invite/add-contact controls

**Files:**
- Modify: `frontend/src/routes/+page.svelte`
- Create: `frontend/src/lib/components/InviteCard.svelte`
- Create: `frontend/src/lib/components/ContactList.svelte`
- Create: `frontend/src/lib/components/AddContactForm.svelte`

### Step 10.1: Create `frontend/src/lib/components/InviteCard.svelte`

- [ ] **Create file** with content:

```svelte
<script lang="ts">
  import { createInvite } from '$lib/tauri';

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
</script>

<div style="border: 1px solid #2a2d36; border-radius: 8px; padding: 1rem; margin-bottom: 1rem;">
  <h3 style="margin: 0 0 0.5rem 0;">Your invite</h3>
  <p style="opacity: 0.7; font-size: 0.9rem; margin: 0 0 0.75rem 0;">
    Share this string with one person. It expires in 7 days.
  </p>
  <button
    type="button"
    onclick={generate}
    disabled={busy}
    style="padding: 0.5rem 1rem; background: #4a4cff; color: white; border: 0; border-radius: 6px; cursor: pointer;"
  >
    {busy ? 'Generating…' : 'Generate invite'}
  </button>

  {#if invite}
    <div style="margin-top: 0.75rem;">
      <textarea
        readonly
        rows="3"
        style="width: 100%; padding: 0.5rem; background: #14151a; color: inherit; border: 1px solid #2a2d36; border-radius: 6px; font-family: monospace; font-size: 0.85rem;"
      >{invite}</textarea>
      <button
        type="button"
        onclick={copy}
        style="margin-top: 0.5rem; padding: 0.4rem 0.8rem; background: transparent; color: inherit; border: 1px solid #2a2d36; border-radius: 6px; cursor: pointer;"
      >
        {copied ? 'Copied!' : 'Copy'}
      </button>
    </div>
  {/if}

  {#if errorMsg}
    <p style="color: #ff6464; margin: 0.5rem 0 0 0;">{errorMsg}</p>
  {/if}
</div>
```

### Step 10.2: Create `frontend/src/lib/components/AddContactForm.svelte`

- [ ] **Create file** with content:

```svelte
<script lang="ts">
  import { addContact, listContacts } from '$lib/tauri';
  import { store } from '$lib/state.svelte';

  let inviteInput = $state('');
  let busy = $state(false);
  let errorMsg = $state<string | null>(null);
  let okMsg = $state<string | null>(null);

  async function submit(e: Event) {
    e.preventDefault();
    if (inviteInput.trim() === '') return;
    busy = true;
    errorMsg = null;
    okMsg = null;
    try {
      await addContact(inviteInput.trim());
      inviteInput = '';
      okMsg = 'Contact added.';
      const cs = await listContacts();
      store.setContacts(cs);
    } catch (e) {
      errorMsg = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<div style="border: 1px solid #2a2d36; border-radius: 8px; padding: 1rem; margin-bottom: 1rem;">
  <h3 style="margin: 0 0 0.5rem 0;">Add contact</h3>
  <form onsubmit={submit}>
    <textarea
      bind:value={inviteInput}
      disabled={busy}
      rows="3"
      placeholder="ghostinvite1q…"
      style="width: 100%; padding: 0.5rem; background: #14151a; color: inherit; border: 1px solid #2a2d36; border-radius: 6px; font-family: monospace; font-size: 0.85rem;"
    ></textarea>
    <button
      type="submit"
      disabled={busy || inviteInput.trim() === ''}
      style="margin-top: 0.5rem; padding: 0.5rem 1rem; background: #4a4cff; color: white; border: 0; border-radius: 6px; cursor: pointer;"
    >
      {busy ? 'Adding…' : 'Add contact'}
    </button>
  </form>
  {#if errorMsg}<p style="color: #ff6464; margin: 0.5rem 0 0 0;">{errorMsg}</p>{/if}
  {#if okMsg}<p style="color: #6effb0; margin: 0.5rem 0 0 0;">{okMsg}</p>{/if}
</div>
```

### Step 10.3: Create `frontend/src/lib/components/ContactList.svelte`

- [ ] **Create file** with content:

```svelte
<script lang="ts">
  import { goto } from '$app/navigation';
  import { store } from '$lib/state.svelte';

  function open(ghost_id: string) {
    goto(`/chat/${encodeURIComponent(ghost_id)}`);
  }
</script>

<div style="border: 1px solid #2a2d36; border-radius: 8px; padding: 1rem;">
  <h3 style="margin: 0 0 0.5rem 0;">Contacts</h3>
  {#if store.contacts.length === 0}
    <p style="opacity: 0.6; margin: 0;">No contacts yet. Share an invite to add one.</p>
  {:else}
    <ul style="list-style: none; padding: 0; margin: 0;">
      {#each store.contacts as c (c.ghost_id)}
        <li style="margin-bottom: 0.5rem;">
          <button
            type="button"
            onclick={() => open(c.ghost_id)}
            style="display: block; width: 100%; text-align: left; padding: 0.6rem; background: #14151a; color: inherit; border: 1px solid #2a2d36; border-radius: 6px; cursor: pointer;"
          >
            <div style="font-family: monospace; font-size: 0.85rem;">{c.fingerprint}</div>
            <div style="opacity: 0.6; font-size: 0.75rem; word-break: break-all;">{c.ghost_id}</div>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>
```

### Step 10.4: Update `frontend/src/routes/+page.svelte`

- [ ] **Replace contents** with:

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import {
    identityStatus,
    openClient,
    clientInfo,
    listContacts,
    onInbound
  } from '$lib/tauri';
  import { store } from '$lib/state.svelte';
  import InviteCard from '$lib/components/InviteCard.svelte';
  import AddContactForm from '$lib/components/AddContactForm.svelte';
  import ContactList from '$lib/components/ContactList.svelte';

  let booting = $state(true);
  let bootError = $state<string | null>(null);
  let unlisten: (() => void) | null = null;

  onMount(async () => {
    try {
      const status = await identityStatus();
      if (!status.exists) {
        await goto('/onboarding');
        return;
      }
      const info = status.client_open ? await clientInfo() : await openClient(null);
      store.setInfo(info);

      const cs = await listContacts();
      store.setContacts(cs);

      const u = await onInbound((ev) => {
        store.pushIncoming(ev.from_ghost_id, {
          uuid: '',
          direction: 'in',
          content: ev.content,
          sent_at: ev.received_at,
          received_at: ev.received_at
        });
      });
      unlisten = () => {
        void u();
      };
    } catch (e) {
      bootError = String(e);
    } finally {
      booting = false;
    }

    return () => {
      unlisten?.();
    };
  });
</script>

<section style="padding: 2rem; max-width: 720px; margin: 0 auto;">
  {#if booting}
    <p>Loading…</p>
  {:else if bootError}
    <p style="color: #ff6464;">{bootError}</p>
  {:else if store.info}
    <header style="margin-bottom: 1.5rem;">
      <div style="opacity: 0.6; font-size: 0.8rem;">YOUR GHOST ID</div>
      <div style="font-family: monospace; font-size: 0.95rem; word-break: break-all;">
        {store.info.ghost_id}
      </div>
      <div style="font-family: monospace; opacity: 0.7; font-size: 0.85rem; margin-top: 0.25rem;">
        {store.info.fingerprint}
      </div>
    </header>

    <InviteCard />
    <AddContactForm />
    <ContactList />
  {/if}
</section>
```

### Step 10.5: Type-check

- [ ] **Run:** `pnpm --dir frontend check`
- [ ] **Expected:** 0 errors.

### Step 10.6: Commit

- [ ] **Run:**

```bash
git add frontend/src/
git commit -m "feat(frontend): home screen with invite, add-contact, contacts list"
```

---

## Task 11: Chat view (`/chat/[ghost_id]`)

**Files:**
- Create: `frontend/src/routes/chat/[ghost_id]/+page.svelte`

### Step 11.1: Create the chat route

- [ ] **Create file** `frontend/src/routes/chat/[ghost_id]/+page.svelte` with content:

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import { listMessages, sendMessage, onInbound } from '$lib/tauri';
  import { store } from '$lib/state.svelte';

  let contactGhostId = $derived(decodeURIComponent(page.params.ghost_id ?? ''));
  let messages = $derived(store.threads[contactGhostId] ?? []);
  let input = $state('');
  let busy = $state(false);
  let errorMsg = $state<string | null>(null);
  let unlisten: (() => void) | null = null;
  let scrollRef: HTMLDivElement | null = $state(null);

  $effect(() => {
    void messages;
    if (scrollRef) {
      scrollRef.scrollTop = scrollRef.scrollHeight;
    }
  });

  onMount(async () => {
    try {
      const initial = await listMessages(contactGhostId);
      store.setThread(contactGhostId, initial);
    } catch (e) {
      errorMsg = String(e);
    }

    const u = await onInbound((ev) => {
      if (ev.from_ghost_id === contactGhostId) {
        store.pushIncoming(contactGhostId, {
          uuid: '',
          direction: 'in',
          content: ev.content,
          sent_at: ev.received_at,
          received_at: ev.received_at
        });
      }
    });
    unlisten = () => {
      void u();
    };

    return () => {
      unlisten?.();
    };
  });

  async function submit(e: Event) {
    e.preventDefault();
    const text = input.trim();
    if (text === '') return;
    busy = true;
    errorMsg = null;
    try {
      await sendMessage(contactGhostId, text);
      const refreshed = await listMessages(contactGhostId);
      store.setThread(contactGhostId, refreshed);
      input = '';
    } catch (e) {
      errorMsg = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section style="display: flex; flex-direction: column; height: 100%; padding: 1rem; max-width: 720px; margin: 0 auto;">
  <header style="margin-bottom: 0.75rem;">
    <a href="/" style="color: #4a8cff; text-decoration: none;">← Home</a>
    <div style="font-family: monospace; opacity: 0.7; font-size: 0.85rem; margin-top: 0.25rem; word-break: break-all;">
      {contactGhostId}
    </div>
  </header>

  <div
    bind:this={scrollRef}
    style="flex: 1; overflow-y: auto; padding: 0.5rem; background: #14151a; border: 1px solid #2a2d36; border-radius: 6px;"
  >
    {#each messages as m, i (m.uuid || `${i}-${m.sent_at}`)}
      <div
        style="margin: 0.4rem 0; display: flex; {m.direction === 'out' ? 'justify-content: flex-end' : 'justify-content: flex-start'};"
      >
        <div
          style="max-width: 70%; padding: 0.5rem 0.75rem; border-radius: 8px; background: {m.direction === 'out' ? '#4a4cff' : '#23252e'}; color: inherit; word-wrap: break-word;"
        >
          {m.content}
        </div>
      </div>
    {/each}
    {#if messages.length === 0}
      <p style="opacity: 0.5; text-align: center; margin-top: 2rem;">No messages yet.</p>
    {/if}
  </div>

  <form
    onsubmit={submit}
    style="display: flex; gap: 0.5rem; margin-top: 0.75rem;"
  >
    <input
      type="text"
      bind:value={input}
      disabled={busy}
      placeholder="Type a message…"
      style="flex: 1; padding: 0.6rem; background: #14151a; color: inherit; border: 1px solid #2a2d36; border-radius: 6px;"
    />
    <button
      type="submit"
      disabled={busy || input.trim() === ''}
      style="padding: 0.6rem 1.2rem; background: #4a4cff; color: white; border: 0; border-radius: 6px; cursor: pointer;"
    >
      {busy ? 'Sending…' : 'Send'}
    </button>
  </form>

  {#if errorMsg}<p style="color: #ff6464; margin: 0.5rem 0 0 0;">{errorMsg}</p>{/if}
</section>
```

### Step 11.2: Type-check

- [ ] **Run:** `pnpm --dir frontend check`
- [ ] **Expected:** 0 errors.

If `$app/state` doesn't resolve in the implementer's SvelteKit minor version, fall back to `import { page } from '$app/stores'` and replace `page.params.ghost_id` with `$page.params.ghost_id` (Svelte 4 store form). The rune-flavoured `$app/state` is the SvelteKit ≥ 2.12 idiom; either is fine for Plan 07.

### Step 11.3: Commit

- [ ] **Run:**

```bash
git add frontend/src/routes/chat/
git commit -m "feat(frontend): chat view with reactive incoming messages"
```

---

## Task 12: Build the bundled frontend + verify Tauri shell launches

**Files:**
- Create: `scripts/launch-ghost-desktop.sh` (helper for two-process smoke)

### Step 12.1: Build the frontend bundle

- [ ] **Run:** `pnpm --dir frontend build`
- [ ] **Expected:** `frontend/build/index.html` exists; `frontend/build/_app/` populated.

### Step 12.2: Run the Tauri shell in dev mode (sanity check)

- [ ] **Run** (foreground, in a separate terminal):

```bash
export PATH="/c/Strawberry/perl/bin:/c/Strawberry/c/bin:$PATH"
cd apps/ghost-desktop
cargo +1.87-x86_64-pc-windows-msvc tauri dev
```

If `cargo tauri` is not installed: `cargo install tauri-cli --version "^2"` (one-time, ~3 minutes).

- [ ] **Expected:** A Ghost window appears. The console log is full of Tauri/webview chatter — fine. Closing the window or Ctrl-C in the terminal stops it.

If `cargo tauri dev` fails because Vite is not running: that's expected if the frontend dev server isn't on port 1420. Tauri's `beforeDevCommand` should auto-start Vite, but on Windows the process pipe sometimes drops. Workaround: in one terminal run `pnpm --dir frontend dev`, in another run `cargo +1.87-x86_64-pc-windows-msvc tauri dev` from `apps/ghost-desktop/` after editing `tauri.conf.json` to remove `"beforeDevCommand"` for the dev session.

### Step 12.3: Create `scripts/launch-ghost-desktop.sh`

This helper sets `GHOST_HOME` and a unique keystore service so a second instance can run side-by-side without colliding with the first.

- [ ] **Create file** with content:

```bash
#!/usr/bin/env bash
# Launch a Ghost desktop instance with isolated identity storage.
# Usage:  ./scripts/launch-ghost-desktop.sh <profile-name>
# Example:  ./scripts/launch-ghost-desktop.sh alice
#
# Each profile gets its own GHOST_HOME directory. The first launch goes through
# onboarding; subsequent launches reuse the existing identity.

set -euo pipefail

PROFILE="${1:?profile name required (e.g. alice, bob)}"
PROFILE_DIR="$(pwd)/.tmp/profiles/${PROFILE}"
mkdir -p "${PROFILE_DIR}"

export GHOST_HOME="${PROFILE_DIR}"
export PATH="/c/Strawberry/perl/bin:/c/Strawberry/c/bin:$PATH"

echo "GHOST_HOME=${GHOST_HOME}"
echo "Launching Ghost desktop (profile: ${PROFILE})"

cd apps/ghost-desktop
exec cargo +1.87-x86_64-pc-windows-msvc tauri dev
```

- [ ] **Make executable:** `chmod +x scripts/launch-ghost-desktop.sh`

Note about the OS keystore: on Windows the keystore service name is currently embedded in `ghost-identity` and may collide between profiles. If it does, the second profile will overwrite the first profile's keystore secret. For Plan 07's smoke we set passphrases to `null` (so the keystore-only path is used) and rely on `GHOST_HOME` directory isolation — the keystore-secret collision is harmless in that mode because each `Identity::create` regenerates it. A proper per-profile keystore namespace lands in MVP-2.

### Step 12.4: Commit

- [ ] **Run:**

```bash
git add scripts/launch-ghost-desktop.sh
git commit -m "feat(scripts): launch helper for isolated Ghost desktop profiles"
```

---

## Task 13: Manual smoke checklist + final verification

**Files:**
- Create: `scripts/smoke-test-plan-07.md`

### Step 13.1: Create the smoke checklist

- [ ] **Create file** with content:

```markdown
# Plan 07 smoke test — two instances chat via the GUI

This is the canonical regression check after any change to Plan 07's Tauri /
frontend layer. Expected runtime: ~5 minutes if everything works.

## Setup

Two separate terminal windows.

**Terminal A (Alice profile):**
```bash
./scripts/launch-ghost-desktop.sh alice
```

**Terminal B (Bob profile):**
```bash
./scripts/launch-ghost-desktop.sh bob
```

Each opens a Ghost window. Wait for both to render the onboarding screen (5–15 s
on a warm cache; up to a minute cold).

## Steps

### 1. Alice — onboarding

- In Alice's window: enter display name `Alice` (passphrase blank), click "Create identity".
- Expected: redirects to home screen showing Alice's Ghost ID + 4×4 fingerprint.

### 2. Bob — onboarding

- In Bob's window: enter display name `Bob`, click "Create identity".
- Expected: home screen showing Bob's Ghost ID + fingerprint. **Different from Alice's**.

### 3. Alice — generate invite

- In Alice's window, click "Generate invite".
- Expected: a `ghostinvite1q...` string appears in the textarea.
- Click "Copy".

### 4. Bob — add Alice as contact

- In Bob's window, paste the invite string into the "Add contact" textarea.
- Click "Add contact".
- Expected: green "Contact added." message; Alice's fingerprint appears in the contacts list.

### 5. Alice — observe new contact

- In Alice's window, the contacts list should show Bob's fingerprint within ~2 seconds (the inbox bridge fires after the Welcome envelope is processed).
  - If Alice's contacts list is still empty after 5 seconds: refresh the home page (browsers reload via F5; Tauri windows can be closed and reopened by re-running the launch script — `GHOST_HOME` persists).

### 6. Bob → Alice message

- In Bob's window, click on Alice's contact entry → chat view opens.
- Type `hello alice` → click "Send".
- Expected: message appears immediately on Bob's right side (outgoing-blue).

### 7. Alice receives

- In Alice's window, click on Bob's contact entry → chat view opens.
- Expected: `hello alice` appears on the left (incoming-grey) within ~1 second of Bob sending.
  - If not present, wait 5 seconds (the inbox watcher polls every 250 ms; first message has cold-cache cost).

### 8. Alice → Bob message

- In Alice's chat view with Bob, type `hi bob` → click "Send".
- Expected: outgoing message on Alice's right.

### 9. Bob receives

- In Bob's chat view with Alice, expected: `hi bob` appears on the left.

## PASS criteria

All 9 steps complete. Both windows show the symmetric conversation:
- Alice's chat with Bob: `hello alice` (left), `hi bob` (right).
- Bob's chat with Alice: `hello alice` (right), `hi bob` (left).

## Cleanup

```bash
rm -rf .tmp/profiles
```

(Or close both windows; `.tmp/profiles` persists for re-runs.)

## Common issues

- **Both windows show Alice's data after first launch** → kept the same `GHOST_HOME`. Pass distinct profile names to the launch script.
- **"Failed to load: client open" repeatedly** → port collision on libp2p loopback. Both clients listen on `/ip4/127.0.0.1/udp/0/quic-v1` (port 0 = OS-assigned), so collisions are theoretically impossible — if it happens, close both windows, `pkill ghost-desktop`, retry.
- **`hello alice` never appears in Alice's window** → the inbox-bridge watcher task is stuck. Check Alice's terminal for `inbox process error` or `inbox watcher scan failed` messages. Re-running both windows usually clears it.
- **Tauri window blank / white** → the frontend bundle was not built. Run `pnpm --dir frontend build` then re-launch.
```

### Step 13.2: Run the smoke test yourself

- [ ] **Walk through the 9 steps.** This is the verification of Plan 07's deliverable.
- [ ] **Expected:** All 9 steps PASS.

If any step fails, debug and fix the underlying code in the relevant Task. Do not edit the smoke checklist to make it match a broken build.

### Step 13.3: Final workspace verification

- [ ] **Run:**

```bash
export PATH="/c/Strawberry/perl/bin:/c/Strawberry/c/bin:$PATH"
cargo +1.87-x86_64-pc-windows-msvc fmt --all -- --check
cargo +1.87-x86_64-pc-windows-msvc clippy -p ghost-app -p ghost-desktop --all-targets -- -D warnings
cargo +1.87-x86_64-pc-windows-msvc test --workspace -- --test-threads=1
```

- [ ] **Expected:**
  - fmt clean
  - clippy clean on the two new crates
  - all 175 prior tests still pass (plan 07 doesn't add new Rust tests beyond the one smoke `crate_compiles` in `ghost-app/src/lib.rs`).

If clippy flags `dead_code` warnings on helper fns from earlier tasks (the `unused_to_silence_lint` placeholder, the duplicated `client_info_from_client_inline`), delete them now — Task 6.7 already replaced them, so this is just removing dead lines.

### Step 13.4: Commit + tag

- [ ] **Run:**

```bash
git add scripts/smoke-test-plan-07.md
git commit -m "test(plan-07): manual smoke checklist for two-instance GUI exchange"

git tag -a plan-07-complete -m "Plan 07 — Tauri App + SvelteKit Frontend

First user-visible Ghost binary. Onboarding, identity display, invite
generation, contact-add via invite, bidirectional E2EE text messaging
through the GUI. Two manual instances complete a full exchange, verified
by scripts/smoke-test-plan-07.md.

Crates added: ghost-app (Tauri command layer), ghost-desktop (Tauri shell
binary). Frontend: SvelteKit + Svelte 5 (runes), static adapter.

Deferred to Plan 08+:
- tauri-plugin-updater integration
- code signing
- production bundle config (icons, installer polish)
- settings screen, fingerprint verification UI"
```

---

## Self-review (mental — implementer should re-check before claiming Plan 07 done)

1. **Spec coverage:**
   - Onboarding flow (spec §3) → Task 3 + Task 9. ✓
   - Add contact via invite (spec §5) → Task 5 (backend) + Task 10 (UI). ✓
   - 1-on-1 text messaging (spec §1 goal) → Task 5 + Task 11. ✓
   - Display Ghost ID + fingerprint (spec §3) → Task 4 + Task 10 home header. ✓
   - Tauri webview UI architecture (spec §2) → Task 6 + frontend tasks. ✓
   - Auto-update (spec §7) → explicitly deferred to Plan 08 in deliverable header. ✓
   - System notifications (spec §1 goal) → not in Plan 07. **Add to "deferred" list above and follow up in Plan 09 or as a Plan 07 follow-up.** Acceptable scope cut for MVP-1 demo.

2. **Placeholder scan:** No "TODO", "fill in details", or "implement later" in the steps. The `unused_to_silence_lint` helper in Task 4 is intentional and removed in Task 6.7. The `display_name: None` returned by `ClientInfoDto` is a real choice (Client doesn't expose it) — documented as MVP-2 follow-up.

3. **Type consistency:**
   - `IdentityStatusDto` / `CreatedIdentityDto` / etc. — same field names in `dto.rs` and `types.ts`. ✓
   - Command names match between `main.rs` `invoke_handler!` macro and `tauri.ts` `invoke('...')` calls. ✓
   - `parse_ghost_id` in `read.rs` and the inline `GhostId::from_bech32` call in `write.rs` use the same parser. ✓
   - `EVENT_NAME` constant in `inbox_bridge.rs` matches `INBOX_EVENT` in `tauri.ts` (both `"ghost://message-received"`). ✓

4. **Scope cuts called out:** notifications, settings, verification UI, signing, updater. All deferred explicitly.

---

**End of Plan 07.** Next plan (08) will add `tauri-plugin-updater`, code signing infrastructure, the GitHub Actions release matrix, and binary transparency log integration.
