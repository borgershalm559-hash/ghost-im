//! Ghost identity: Identity Key + Device Key + encrypted identity file + OS keystore.

pub mod crypto;
pub mod file_format;
pub mod identity;
pub mod keys;
pub mod keystore;
pub mod paths;
pub mod prekey; // kept for now but no longer used by Identity; will be removed in Task 17
pub mod storage;

pub use file_format::{Header, FILE_FORMAT_VERSION};
pub use identity::{
    CreateOptions, Identity, IdentityError, IDENTITY_SCHEMA_VERSION,
    INITIAL_KEYPACKAGE_COUNT,
};
pub use keys::{DeviceKey, IdentityKey};
pub use keystore::{load_or_create_secret, store_secret, wipe_secret, KeystoreError};
pub use paths::{database_file, ghost_home, identity_file, logs_dir, PathsError};
pub use storage::{load, save, StorageError};

// Compatibility shim for INITIAL_PREKEY_COUNT — remove in Task 17.
#[deprecated(note = "Use INITIAL_KEYPACKAGE_COUNT (Plan 02). Will be removed in Task 17.")]
pub use identity::INITIAL_KEYPACKAGE_COUNT as INITIAL_PREKEY_COUNT;
