//! Tauri-managed shared state.
//!
//! The app holds at most one active `Client` at a time. After onboarding (or on
//! launch when an identity already exists), the frontend calls `open_client`
//! which populates `client`. Subsequent commands lock + read the stored Client.

use ghost_client::Client;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Shared, Tauri-managed state. One per app process.
#[derive(Default)]
pub struct AppState {
    /// `None` until `open_client` succeeds. Held inside an `Arc` so commands can
    /// drop the mutex guard before performing long async work.
    pub client: Mutex<Option<Arc<Client>>>,

    /// Set once `start_inbox_processor` has been called for the current Client.
    /// Used to abort the task when the Client is replaced (currently only happens
    /// at process exit; reserved for future "switch identity" flows).
    pub inbox_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl AppState {
    /// Convenience: load the current Client. Errors if no client is open.
    pub async fn require_client(&self) -> Result<Arc<Client>, crate::error::CommandError> {
        self.client
            .lock()
            .await
            .clone()
            .ok_or_else(|| crate::error::CommandError("no client open".to_string()))
    }
}
