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
