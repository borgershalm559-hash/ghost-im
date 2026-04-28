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
