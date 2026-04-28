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
