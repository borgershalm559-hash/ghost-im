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
