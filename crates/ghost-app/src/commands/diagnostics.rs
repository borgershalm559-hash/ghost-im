//! Diagnostics command — exposes peer_id, listen addresses, version, etc.
//! Used by the Settings → Диагностика section in the UI.

use crate::error::CommandResult;
use crate::AppState;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct DiagnosticsDto {
    pub app_version: String,
    pub ghost_id: String,
    pub fingerprint: String,
    pub peer_id: String,
    pub local_addrs: Vec<String>,
    pub bootstrap_count: usize,
}

#[tauri::command]
pub async fn get_diagnostics(state: State<'_, AppState>) -> CommandResult<DiagnosticsDto> {
    let client = state.require_client().await?;
    let ghost_id = client.ghost_id();
    let fingerprint = ghost_core::Fingerprint::of(&ghost_id).to_string();
    Ok(DiagnosticsDto {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        ghost_id: ghost_id.to_string(),
        fingerprint,
        peer_id: client.local_peer_id().to_string(),
        local_addrs: client.local_addrs().iter().map(|a| a.to_string()).collect(),
        bootstrap_count: 4, // hardcoded in ghost-network
    })
}
