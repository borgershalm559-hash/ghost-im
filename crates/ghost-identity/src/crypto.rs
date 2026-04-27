//! Symmetric crypto helpers: passphrase KDF and AEAD wrappers.

use argon2::{Algorithm, Argon2, Params, Version};
use thiserror::Error;

/// Length of derived key in bytes (256 bit).
pub const DERIVED_KEY_LEN: usize = 32;
/// Length of the salt for Argon2id — 16 random bytes per file.
pub const SALT_LEN: usize = 16;

/// Argon2id parameters — moderate memory + time cost, conservative defaults.
/// m=64MiB, t=3, p=1 — accepted ballpark per OWASP Password Storage Cheat Sheet.
fn argon2() -> Argon2<'static> {
    let params = Params::new(64 * 1024, 3, 1, Some(DERIVED_KEY_LEN))
        .expect("hard-coded Argon2 params are valid");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

#[derive(Debug, Error)]
pub enum KdfError {
    #[error("Argon2 KDF failed: {0}")]
    Failed(String),
}

/// Derive a 32-byte key from input bytes (passphrase || keystore secret) and a 16-byte salt.
pub fn derive_key(input: &[u8], salt: &[u8; SALT_LEN]) -> Result<[u8; DERIVED_KEY_LEN], KdfError> {
    let mut out = [0u8; DERIVED_KEY_LEN];
    argon2()
        .hash_password_into(input, salt, &mut out)
        .map_err(|e| KdfError::Failed(e.to_string()))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_key_is_deterministic_given_input_and_salt() {
        let input = b"correct horse battery staple";
        let salt = [42u8; 16];
        let k1 = derive_key(input, &salt).unwrap();
        let k2 = derive_key(input, &salt).unwrap();
        assert_eq!(k1, k2);
    }

    #[test]
    fn different_salt_yields_different_key() {
        let input = b"same passphrase";
        let k1 = derive_key(input, &[1u8; 16]).unwrap();
        let k2 = derive_key(input, &[2u8; 16]).unwrap();
        assert_ne!(k1, k2);
    }

    #[test]
    fn different_input_yields_different_key() {
        let salt = [0u8; 16];
        let k1 = derive_key(b"alpha", &salt).unwrap();
        let k2 = derive_key(b"beta", &salt).unwrap();
        assert_ne!(k1, k2);
    }
}
