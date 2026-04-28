//! Ghost desktop shell — Tauri entrypoint.
//!
//! Mounts the `ghost-app` command surface, sets up logging, and starts the
//! event loop. Frontend lives in ../../frontend and is bundled at build time.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use ghost_app::commands::{identity, lifecycle, read, write};
use ghost_app::AppState;
use tauri::Manager;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("ghost_=info,info")),
        )
        .init();

    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            identity::identity_status,
            identity::create_identity,
            lifecycle::open_client,
            lifecycle::close_client,
            read::client_info,
            read::list_contacts,
            read::list_messages,
            read::create_invite,
            write::add_contact,
            write::send_message,
        ])
        .setup(|app| {
            // Reserved for future use (shutdown hook, switch-identity flow).
            // open_client gets AppHandle directly via Tauri command parameter,
            // so it does not retrieve this state.
            let app_handle = app.handle().clone();
            app.manage(InboxBridgeHandle(app_handle));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("ghost-desktop failed to run");
}

/// Wrapper around `AppHandle` so it can live in Tauri's `State<'_>` layer.
/// `AppHandle` itself isn't required to be wrapped, but doing so keeps the
/// generic-State type signatures clean in `lifecycle::open_client`.
pub struct InboxBridgeHandle(pub tauri::AppHandle);
