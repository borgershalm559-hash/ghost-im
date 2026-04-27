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

use crate::keystore::{load_or_create_secret, KeystoreError};
use crate::paths::{identity_file, PathsError};
use crate::storage::{load as storage_load, save as storage_save, StorageError};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("paths: {0}")]
    Paths(#[from] PathsError),
    #[error("keystore: {0}")]
    Keystore(#[from] KeystoreError),
    #[error("storage: {0}")]
    Storage(#[from] StorageError),
    #[error("identity already exists at {0:?}")]
    AlreadyExists(PathBuf),
    #[error("identity not found at {0:?}")]
    NotFound(PathBuf),
    #[error("system clock returned a time before UNIX_EPOCH")]
    BadClock,
}

fn now_seconds() -> Result<u64, IdentityError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|_| IdentityError::BadClock)
}

fn key_input(passphrase: Option<&str>, keystore_secret: &[u8; 32]) -> Vec<u8> {
    let mut input = Vec::with_capacity(64);
    if let Some(p) = passphrase {
        input.extend_from_slice(p.as_bytes());
    }
    input.extend_from_slice(keystore_secret);
    input
}

pub struct CreateOptions<'a> {
    pub display_name: Option<String>,
    pub passphrase: Option<&'a str>,
    pub overwrite: bool,
}

impl Identity {
    /// Generate, encrypt, and persist a fresh Identity at the standard path.
    pub fn create(opts: CreateOptions<'_>) -> Result<Self, IdentityError> {
        let path = identity_file()?;
        if path.exists() && !opts.overwrite {
            return Err(IdentityError::AlreadyExists(path));
        }
        let keystore_secret = load_or_create_secret()?;
        let key_input = key_input(opts.passphrase, &keystore_secret);
        let identity = Identity::generate(opts.display_name, now_seconds()?);
        storage_save(&identity, &key_input, &path)?;
        Ok(identity)
    }

    /// Load and decrypt the Identity from the standard path.
    pub fn load_default(passphrase: Option<&str>) -> Result<Self, IdentityError> {
        let path = identity_file()?;
        if !path.exists() {
            return Err(IdentityError::NotFound(path));
        }
        let keystore_secret = load_or_create_secret()?;
        let key_input = key_input(passphrase, &keystore_secret);
        let identity = storage_load(&key_input, &path)?;
        Ok(identity)
    }
}

#[cfg(test)]
mod create_load_tests {
    use super::*;
    use crate::keystore;
    use std::sync::Mutex;
    use tempfile::tempdir;

    /// Serialize tests in this module — they share GHOST_HOME and OS keystore.
    static LOCK: Mutex<()> = Mutex::new(());

    fn isolated<F: FnOnce()>(f: F) {
        let _g = LOCK.lock().unwrap();
        let dir = tempdir().unwrap();
        std::env::set_var("GHOST_HOME", dir.path());
        let _ = keystore::wipe_secret();
        f();
        let _ = keystore::wipe_secret();
        std::env::remove_var("GHOST_HOME");
    }

    #[test]
    fn create_then_load_default() {
        isolated(|| {
            let created = Identity::create(CreateOptions {
                display_name: Some("Alice".to_string()),
                passphrase: None,
                overwrite: false,
            })
            .unwrap();
            let loaded = Identity::load_default(None).unwrap();
            assert_eq!(loaded.ghost_id(), created.ghost_id());
            assert_eq!(loaded.display_name.as_deref(), Some("Alice"));
        });
    }

    #[test]
    fn create_with_passphrase_load_with_same_passphrase() {
        isolated(|| {
            Identity::create(CreateOptions {
                display_name: None,
                passphrase: Some("hunter2"),
                overwrite: false,
            })
            .unwrap();
            let loaded = Identity::load_default(Some("hunter2")).unwrap();
            assert!(loaded.device_key.verify_parent(&loaded.identity_key.public()));
        });
    }

    #[test]
    fn load_with_wrong_passphrase_fails() {
        isolated(|| {
            Identity::create(CreateOptions {
                display_name: None,
                passphrase: Some("right"),
                overwrite: false,
            })
            .unwrap();
            let err = Identity::load_default(Some("wrong")).unwrap_err();
            assert!(matches!(err, IdentityError::Storage(_)));
        });
    }

    #[test]
    fn create_refuses_overwrite_by_default() {
        isolated(|| {
            Identity::create(CreateOptions {
                display_name: None,
                passphrase: None,
                overwrite: false,
            })
            .unwrap();
            let err = Identity::create(CreateOptions {
                display_name: None,
                passphrase: None,
                overwrite: false,
            })
            .unwrap_err();
            assert!(matches!(err, IdentityError::AlreadyExists(_)));
        });
    }

    #[test]
    fn create_with_overwrite_replaces_identity() {
        isolated(|| {
            let first = Identity::create(CreateOptions {
                display_name: Some("First".to_string()),
                passphrase: None,
                overwrite: false,
            })
            .unwrap();
            let second = Identity::create(CreateOptions {
                display_name: Some("Second".to_string()),
                passphrase: None,
                overwrite: true,
            })
            .unwrap();
            assert_ne!(first.ghost_id(), second.ghost_id());

            let loaded = Identity::load_default(None).unwrap();
            assert_eq!(loaded.ghost_id(), second.ghost_id());
        });
    }

    #[test]
    fn load_when_no_identity_exists_returns_not_found() {
        isolated(|| {
            let err = Identity::load_default(None).unwrap_err();
            assert!(matches!(err, IdentityError::NotFound(_)));
        });
    }
}
