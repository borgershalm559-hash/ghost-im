//! Identity — top-level user identity, serialized to CBOR before encryption.

use crate::keys::{DeviceKey, IdentityKey};
use crate::prekey::{generate_batch, PreKey};
use ghost_core::GhostId;
use serde::{Deserialize, Serialize};

/// Bumped on every breaking change to identity file schema.
pub const IDENTITY_SCHEMA_VERSION: u8 = 1;

/// Number of one-time pre-keys generated at identity creation.
pub const INITIAL_PREKEY_COUNT: u32 = 10;

#[derive(Serialize, Deserialize)]
pub struct Identity {
    pub schema_version: u8,
    pub identity_key: IdentityKey,
    pub device_key: DeviceKey,
    pub display_name: Option<String>,
    pub one_time_prekeys: Vec<PreKey>,
    pub last_resort_prekey: PreKey,
    pub next_prekey_id: u32,
    pub created_at: u64,
}

impl Identity {
    /// Generate a fresh Identity with the given display name and current timestamp.
    /// `now` is the Unix-epoch seconds.
    pub fn generate(display_name: Option<String>, now: u64) -> Self {
        let identity_key = IdentityKey::generate();
        let device_key = DeviceKey::generate(&identity_key);
        let (one_time_prekeys, last_resort_prekey) = generate_batch(INITIAL_PREKEY_COUNT, 0, now);
        Self {
            schema_version: IDENTITY_SCHEMA_VERSION,
            identity_key,
            device_key,
            display_name,
            one_time_prekeys,
            last_resort_prekey,
            next_prekey_id: INITIAL_PREKEY_COUNT + 1,
            created_at: now,
        }
    }

    pub fn ghost_id(&self) -> GhostId {
        self.identity_key.ghost_id()
    }
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identity")
            .field("schema_version", &self.schema_version)
            .field("ghost_id", &self.ghost_id())
            .field("display_name", &self.display_name)
            .field("one_time_prekeys", &self.one_time_prekeys.len())
            .field("created_at", &self.created_at)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_populates_all_fields() {
        let id = Identity::generate(Some("Alice".to_string()), 1700000000);
        assert_eq!(id.schema_version, IDENTITY_SCHEMA_VERSION);
        assert_eq!(id.display_name.as_deref(), Some("Alice"));
        assert_eq!(id.one_time_prekeys.len(), INITIAL_PREKEY_COUNT as usize);
        assert!(id.last_resort_prekey.is_last_resort);
        assert_eq!(id.created_at, 1700000000);
    }

    #[test]
    fn dk_signature_verifies_against_ik() {
        let id = Identity::generate(None, 0);
        assert!(id.device_key.verify_parent(&id.identity_key.public()));
    }

    #[test]
    fn cbor_roundtrip_preserves_identity() {
        let original = Identity::generate(Some("Bob".to_string()), 1700000000);
        let original_id = original.ghost_id();
        let original_pk_count = original.one_time_prekeys.len();

        let mut buf = Vec::new();
        ciborium::into_writer(&original, &mut buf).unwrap();

        let restored: Identity = ciborium::from_reader(&buf[..]).unwrap();
        assert_eq!(restored.ghost_id(), original_id);
        assert_eq!(restored.display_name.as_deref(), Some("Bob"));
        assert_eq!(restored.one_time_prekeys.len(), original_pk_count);
        assert!(restored.device_key.verify_parent(&restored.identity_key.public()));
    }
}
