//! Ghost identity: Identity Key + Device Key + encrypted identity file + OS keystore.

pub mod crypto;
pub mod file_format;
pub mod identity;
pub mod keys;
pub mod keystore;
pub mod paths;
pub mod storage;

pub use file_format::{Header, FILE_FORMAT_VERSION};
pub use identity::{
    CreateOptions, Identity, IdentityError, IDENTITY_SCHEMA_VERSION, INITIAL_KEYPACKAGE_COUNT,
};
pub use keys::{DeviceKey, IdentityKey};
pub use keystore::{load_or_create_secret, store_secret, wipe_secret, KeystoreError};
pub use paths::{database_file, ghost_home, identity_file, logs_dir, PathsError};
pub use storage::{load, save, StorageError};
