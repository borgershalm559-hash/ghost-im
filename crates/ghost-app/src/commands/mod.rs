//! Tauri command implementations. Each `pub async fn` annotated with
//! `#[tauri::command]` is exposed to the frontend's `invoke()` calls.
//!
//! Commands are split per-domain into submodules so each file stays small.

pub mod identity;
