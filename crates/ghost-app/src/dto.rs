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

/// Result of `check_for_update` command. `None` (in `CommandResult<Option<…>>`) means
/// no update available; `Some(...)` means an update is available with these details.
#[derive(Debug, Serialize)]
pub struct UpdateAvailableDto {
    pub version: String,
    pub notes: Option<String>,
    pub release_date: Option<String>,
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
