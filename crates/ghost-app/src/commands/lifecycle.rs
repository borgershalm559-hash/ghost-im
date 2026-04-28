//! Open / close commands for the Client lifecycle.

use crate::dto::ClientInfoDto;
use crate::error::{CommandError, CommandResult};
use crate::inbox_bridge;
use crate::AppState;
use ghost_client::{Client, ClientConfig};
use ghost_core::Fingerprint;
use std::sync::Arc;
use tauri::{AppHandle, State};

/// Open the embedded `ghost-client` runtime. Reads identity from disk (with the
/// optional passphrase), starts the Network + Server, and stores the Client in
/// `AppState`. Idempotent: calling twice returns the existing client info
/// without re-opening.
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
