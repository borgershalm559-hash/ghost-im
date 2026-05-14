//! Ghost desktop shell — Tauri entrypoint.
//!
//! Mounts the `ghost-app` command surface, sets up logging, and starts the
//! event loop. Frontend lives in ../../frontend and is bundled at build time.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use ghost_app::commands::{
    backup, contact_actions, folders, identity, lifecycle, read, settings as settings_cmd,
    updater, write,
};
use ghost_app::AppState;
use tauri::Manager;
use tracing_subscriber::prelude::*;

fn main() {
    init_logging();

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
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
            updater::check_for_update,
            updater::download_and_install_update,
            write::add_contact,
            write::send_message,
            contact_actions::set_pinned,
            contact_actions::set_muted,
            contact_actions::set_verified,
            contact_actions::set_retention,
            contact_actions::mark_chat_read,
            settings_cmd::get_setting,
            settings_cmd::set_setting,
            backup::export_backup,
            backup::import_backup,
            folders::list_folders,
            folders::create_folder,
            folders::rename_folder,
            folders::delete_folder,
            folders::add_contact_to_folder,
            folders::remove_contact_from_folder,
            folders::contacts_for_folder,
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();
            app.manage(InboxBridgeHandle(app_handle.clone()));

            // DevTools — opt-in via GHOST_DEVTOOLS=1. The `devtools` Cargo
            // feature is enabled for tauri in Cargo.toml, so this API exists
            // in both debug + release builds. A normal user without the env
            // var sees no DevTools panel.
            if std::env::var("GHOST_DEVTOOLS").is_ok() {
                if let Some(window) = app_handle.get_webview_window("main") {
                    window.open_devtools();
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("ghost-desktop failed to run");
}

/// Initialise logging: stdout (always) + rotating file in `%APPDATA%/Ghost/Ghost/data/logs/`.
fn init_logging() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("ghost_=info,info"));

    // File appender — daily rotation, 7-day retention is handled implicitly
    // by `tracing_appender::rolling::daily` (it doesn't auto-delete, but the
    // disk impact is tiny — ~1MB/day worst case for our log volume).
    let log_dir = match log_dir_path() {
        Some(p) => p,
        None => {
            // Fallback to stdout only.
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer())
                .init();
            return;
        }
    };
    let _ = std::fs::create_dir_all(&log_dir);
    let file_appender = tracing_appender::rolling::daily(&log_dir, "ghost.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
    // Leak the guard so it lives for the process lifetime — otherwise dropping
    // it flushes + closes the writer, breaking logging mid-session.
    std::mem::forget(guard);

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(file_writer)
                .with_ansi(false),
        )
        .init();
}

fn log_dir_path() -> Option<std::path::PathBuf> {
    // Mirror ghost-identity's path resolution. On Windows this is
    // %APPDATA%/Ghost/Ghost/data/logs/.
    directories::ProjectDirs::from("im", "Ghost", "Ghost").map(|pd| pd.data_dir().join("logs"))
}

/// Wrapper around `AppHandle` so it can live in Tauri's `State<'_>` layer.
/// `AppHandle` itself isn't required to be wrapped, but doing so keeps the
/// generic-State type signatures clean in `lifecycle::open_client`.
pub struct InboxBridgeHandle(pub tauri::AppHandle);
