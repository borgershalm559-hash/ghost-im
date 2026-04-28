//! Ghost Tauri command layer.
//!
//! Wraps `ghost-client` for the desktop shell. Exposes async `#[tauri::command]`
//! functions that return JSON-serializable DTOs and a `CommandError` string-shaped
//! error type.

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-app");
    }
}
