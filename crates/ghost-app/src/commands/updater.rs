//! Updater commands: check for available updates, download + install them.
//!
//! These are thin wrappers over `tauri-plugin-updater`'s `UpdaterExt` trait.
//! The actual download progress is tracked via Tauri events emitted by the
//! plugin itself (`tauri-plugin-updater` v2 emits `update-progress` payloads
//! through a callback we pass to `download_and_install`).

use crate::dto::UpdateAvailableDto;
use crate::error::{CommandError, CommandResult};
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

/// Polls the configured update endpoints. Returns the update details if one is
/// available, or `None` if the running version is the latest.
#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> CommandResult<Option<UpdateAvailableDto>> {
    let updater = app
        .updater()
        .map_err(|e| CommandError(format!("updater init: {e}")))?;
    match updater.check().await {
        Ok(Some(update)) => Ok(Some(UpdateAvailableDto {
            version: update.version.clone(),
            notes: update.body.clone(),
            release_date: update.date.map(|d| d.to_string()),
        })),
        Ok(None) => Ok(None),
        Err(e) => Err(CommandError(format!("update check: {e}"))),
    }
}

/// Downloads and installs the latest update. Emits `update-progress` events
/// during the download. Returns once the installer has been launched (the
/// current process exits shortly after).
#[tauri::command]
pub async fn download_and_install_update(app: AppHandle) -> CommandResult<()> {
    let updater = app
        .updater()
        .map_err(|e| CommandError(format!("updater init: {e}")))?;
    let update = updater
        .check()
        .await
        .map_err(|e| CommandError(format!("update check: {e}")))?
        .ok_or_else(|| CommandError("no update available".to_string()))?;

    let app_clone = app.clone();
    update
        .download_and_install(
            move |chunk_length, content_length| {
                let payload = ProgressPayload {
                    chunk: chunk_length as u64,
                    total: content_length.unwrap_or(0),
                };
                let _ = app_clone.emit("ghost://update-progress", payload);
            },
            || {
                // Finished — handler called once after install completes.
            },
        )
        .await
        .map_err(|e| CommandError(format!("download/install: {e}")))?;
    Ok(())
}

#[derive(Clone, serde::Serialize)]
struct ProgressPayload {
    chunk: u64,
    total: u64,
}
