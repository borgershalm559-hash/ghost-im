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
