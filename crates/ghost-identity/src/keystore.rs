//! Cross-platform OS keystore for the random secret used as part of the identity-file KDF input.
//! Linux: Secret Service / GNOME Keyring / KWallet
//! macOS: Keychain
//! Windows: Credential Manager
//!
//! We store a base64-encoded 32-byte random secret. It is generated once on first identity
//! creation and reused across launches.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rand::RngCore;
use thiserror::Error;

const SERVICE: &str = "im.ghost.app";
const ACCOUNT: &str = "identity-keystore-secret-v1";
const SECRET_LEN: usize = 32;

#[derive(Debug, Error)]
pub enum KeystoreError {
    #[error("OS keystore: {0}")]
    Backend(String),
    #[error("stored secret has unexpected length {0} (expected {SECRET_LEN})")]
    BadLength(usize),
    #[error("stored secret is not valid base64: {0}")]
    BadEncoding(String),
}

fn entry() -> Result<keyring::Entry, KeystoreError> {
    keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| KeystoreError::Backend(e.to_string()))
}

/// Look up the keystore secret, generating + storing a new one if absent.
pub fn load_or_create_secret() -> Result<[u8; SECRET_LEN], KeystoreError> {
    let entry = entry()?;
    match entry.get_password() {
        Ok(s) => decode_secret(&s),
        Err(keyring::Error::NoEntry) => {
            let mut secret = [0u8; SECRET_LEN];
            rand::thread_rng().fill_bytes(&mut secret);
            let encoded = B64.encode(secret);
            entry
                .set_password(&encoded)
                .map_err(|e| KeystoreError::Backend(e.to_string()))?;
            Ok(secret)
        }
        Err(e) => Err(KeystoreError::Backend(e.to_string())),
    }
}

/// Force-overwrite the stored secret. Used by `wipe()` and tests.
pub fn store_secret(secret: &[u8; SECRET_LEN]) -> Result<(), KeystoreError> {
    let encoded = B64.encode(secret);
    entry()?
        .set_password(&encoded)
        .map_err(|e| KeystoreError::Backend(e.to_string()))
}

/// Remove the stored secret entirely (e.g., during `ghost-identity wipe`).
pub fn wipe_secret() -> Result<(), KeystoreError> {
    match entry()?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(KeystoreError::Backend(e.to_string())),
    }
}

fn decode_secret(s: &str) -> Result<[u8; SECRET_LEN], KeystoreError> {
    let bytes = B64
        .decode(s)
        .map_err(|e| KeystoreError::BadEncoding(e.to_string()))?;
    if bytes.len() != SECRET_LEN {
        return Err(KeystoreError::BadLength(bytes.len()));
    }
    let mut out = [0u8; SECRET_LEN];
    out.copy_from_slice(&bytes);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// These tests touch the real OS keystore. We use a per-test isolation by always
    /// wiping before/after. Run sequentially (`--test-threads=1`) if your CI runs tests
    /// in parallel — `cargo test` serializes by default for keyring backends on
    /// Linux/Windows, but if you see flakes, set `RUST_TEST_THREADS=1`.
    fn isolate() {
        let _ = wipe_secret();
    }

    #[test]
    fn create_returns_random_secret() {
        isolate();
        let s1 = load_or_create_secret().unwrap();
        // Second call must return the same secret (it was persisted).
        let s2 = load_or_create_secret().unwrap();
        assert_eq!(s1, s2);
        wipe_secret().unwrap();
    }

    #[test]
    fn wipe_then_create_yields_new_secret() {
        isolate();
        let s1 = load_or_create_secret().unwrap();
        wipe_secret().unwrap();
        let s2 = load_or_create_secret().unwrap();
        assert_ne!(s1, s2);
        wipe_secret().unwrap();
    }

    #[test]
    fn decode_secret_rejects_wrong_length() {
        let encoded = B64.encode([1u8; 16]);
        let err = decode_secret(&encoded).unwrap_err();
        assert!(matches!(err, KeystoreError::BadLength(16)));
    }

    #[test]
    fn decode_secret_rejects_invalid_base64() {
        let err = decode_secret("not!base!64!").unwrap_err();
        assert!(matches!(err, KeystoreError::BadEncoding(_)));
    }
}
