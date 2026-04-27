//! Ghost identity: Identity Key + Device Key + encrypted identity file + OS keystore.

pub mod crypto;
pub mod file_format;
pub mod identity;
pub mod keys;
pub mod prekey;
pub mod storage;

pub use file_format::{Header, FILE_FORMAT_VERSION};
pub use identity::{Identity, IDENTITY_SCHEMA_VERSION, INITIAL_PREKEY_COUNT};
pub use keys::{DeviceKey, IdentityKey};
pub use prekey::{generate_batch, PreKey};
pub use storage::{load, save, StorageError};
