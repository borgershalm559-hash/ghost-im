//! Generic settings get/set: simple key-value strings persisted in the
//! `settings` table. The keys we expect today: `theme` (`"dark"` | `"light"`),
//! `ghost_mode` (`"0"` | `"1"`).

use crate::error::CommandResult;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_setting(
    key: String,
    state: State<'_, AppState>,
) -> CommandResult<Option<String>> {
    let client = state.require_client().await?;
    Ok(client.get_setting(&key)?)
}

#[tauri::command]
pub async fn set_setting(
    key: String,
    value: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let client = state.require_client().await?;
    client.set_setting(&key, &value)?;
    Ok(())
}
