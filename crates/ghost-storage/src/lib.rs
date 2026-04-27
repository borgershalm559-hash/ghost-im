//! Ghost storage: SQLCipher-encrypted SQLite database.
//!
//! Provides a `Database` wrapper around `rusqlite::Connection` plus repository APIs
//! for the seven core tables (contacts, mls_groups, my_keypackages, messages,
//! outbox, inbox_dedup, settings). The DB encryption key is derived from the
//! user's IdentityKey via HKDF, so unlocking the identity unlocks the DB.

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-storage");
    }
}
