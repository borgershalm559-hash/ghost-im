//! Ghost Tauri command layer.

pub mod app_state;
pub mod commands;
pub mod dto;
pub mod error;
pub mod inbox_bridge;

pub use app_state::AppState;
pub use error::{CommandError, CommandResult};

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-app");
    }
}
