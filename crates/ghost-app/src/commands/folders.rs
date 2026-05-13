//! Folder CRUD + contact membership commands.

use crate::error::{CommandError, CommandResult};
use crate::AppState;
use ghost_core::GhostId;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct FolderDto {
    pub id: i64,
    pub name: String,
    pub icon: Option<String>,
    pub sort_order: i64,
    pub created_at: i64,
}

fn parse(s: &str) -> CommandResult<GhostId> {
    GhostId::from_bech32(s).map_err(|e| CommandError(format!("ghost id: {e}")))
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[tauri::command]
pub async fn list_folders(state: State<'_, AppState>) -> CommandResult<Vec<FolderDto>> {
    let client = state.require_client().await?;
    let rows = client.list_folders()?;
    Ok(rows
        .into_iter()
        .map(|f| FolderDto {
            id: f.id,
            name: f.name,
            icon: f.icon,
            sort_order: f.sort_order,
            created_at: f.created_at,
        })
        .collect())
}

#[tauri::command]
pub async fn create_folder(
    name: String,
    icon: Option<String>,
    state: State<'_, AppState>,
) -> CommandResult<i64> {
    let client = state.require_client().await?;
    if name.trim().is_empty() {
        return Err(CommandError("folder name cannot be empty".into()));
    }
    let id = client.create_folder(name.trim(), icon.as_deref(), now_seconds())?;
    Ok(id)
}

#[tauri::command]
pub async fn rename_folder(
    folder_id: i64,
    new_name: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let client = state.require_client().await?;
    if new_name.trim().is_empty() {
        return Err(CommandError("folder name cannot be empty".into()));
    }
    client.rename_folder(folder_id, new_name.trim())?;
    Ok(())
}

#[tauri::command]
pub async fn delete_folder(folder_id: i64, state: State<'_, AppState>) -> CommandResult<bool> {
    let client = state.require_client().await?;
    let removed = client.delete_folder(folder_id)?;
    Ok(removed)
}

#[tauri::command]
pub async fn add_contact_to_folder(
    folder_id: i64,
    contact_ghost_id: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let client = state.require_client().await?;
    client.add_contact_to_folder(folder_id, &parse(&contact_ghost_id)?)?;
    Ok(())
}

#[tauri::command]
pub async fn remove_contact_from_folder(
    folder_id: i64,
    contact_ghost_id: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let client = state.require_client().await?;
    client.remove_contact_from_folder(folder_id, &parse(&contact_ghost_id)?)?;
    Ok(())
}

#[tauri::command]
pub async fn contacts_for_folder(
    folder_id: i64,
    state: State<'_, AppState>,
) -> CommandResult<Vec<String>> {
    let client = state.require_client().await?;
    let ids = client.contacts_for_folder(folder_id)?;
    Ok(ids.into_iter().map(|g| g.to_string()).collect())
}
