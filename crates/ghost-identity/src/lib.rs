//! Ghost identity: Identity Key + Device Key + encrypted identity file + OS keystore.

pub mod keys;
pub mod prekey;

pub use keys::{DeviceKey, IdentityKey};
pub use prekey::{generate_batch, PreKey};
