//! Ghost storage: SQLCipher-encrypted SQLite database.

pub mod database;
pub mod error;
pub mod master_key;
pub mod migrations;
pub mod repos;

pub use database::Database;
pub use error::{Result, StorageError};
pub use master_key::{derive_master_key, master_key_pragma, MASTER_KEY_LEN};
pub use migrations::APP_SCHEMA_VERSION;
pub use repos::{
    Contact, ContactsRepo, Direction, InboxDedupRepo, MessageRow, MessageStatus, MessagesRepo,
    MlsGroupRow, MlsGroupsRepo, MyKeyPackageRow, MyKeyPackagesRepo, OutboxRepo, OutboxRow,
    SettingsRepo, Verification,
};

#[cfg(test)]
mod smoke_tests {
    #[test]
    fn crate_compiles() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ghost-storage");
    }
}
