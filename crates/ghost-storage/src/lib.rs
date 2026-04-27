//! Ghost storage: SQLCipher-encrypted SQLite database.

pub mod database;
pub mod error;
pub mod master_key;

pub use database::Database;
pub use error::{Result, StorageError};
pub use master_key::{derive_master_key, master_key_pragma, MASTER_KEY_LEN};

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-storage");
    }
}
