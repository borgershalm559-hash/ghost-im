//! Ghost identity: Identity Key + Device Key + encrypted identity file + OS keystore.

pub mod crypto;
pub mod identity;
pub mod keys;
pub mod prekey;

pub use identity::{Identity, IDENTITY_SCHEMA_VERSION, INITIAL_PREKEY_COUNT};
pub use keys::{DeviceKey, IdentityKey};
pub use prekey::{generate_batch, PreKey};
