//! Symmetric crypto helpers: passphrase KDF and AEAD wrappers.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use rand::RngCore;
use thiserror::Error;

/// Length of derived key in bytes (256 bit).
pub const DERIVED_KEY_LEN: usize = 32;
/// Length of the salt for Argon2id — 16 random bytes per file.
pub const SALT_LEN: usize = 16;
/// XChaCha20-Poly1305 nonce length: 24 bytes.
pub const NONCE_LEN: usize = 24;

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

#[derive(Debug, Error)]
pub enum AeadError {
    #[error("encryption failed")]
    EncryptFailed,
    #[error("decryption failed (wrong key, tampered ciphertext, or corrupt file)")]
    DecryptFailed,
}

/// Encrypt plaintext with the 32-byte key. Returns (nonce, ciphertext_with_tag).
/// `aad` is optional Associated Authenticated Data — bound but not encrypted.
pub fn aead_encrypt(
    key: &[u8; DERIVED_KEY_LEN],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<([u8; NONCE_LEN], Vec<u8>), AeadError> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| AeadError::EncryptFailed)?;
    Ok((nonce_bytes, ciphertext))
}

pub fn aead_decrypt(
    key: &[u8; DERIVED_KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    ciphertext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, AeadError> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce = XNonce::from_slice(nonce);
    cipher
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| AeadError::DecryptFailed)
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

#[cfg(test)]
mod aead_tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = [9u8; 32];
        let plaintext = b"the secret message";
        let aad = b"some context";
        let (nonce, ct) = aead_encrypt(&key, plaintext, aad).unwrap();
        let pt = aead_decrypt(&key, &nonce, &ct, aad).unwrap();
        assert_eq!(pt, plaintext);
    }

    #[test]
    fn decrypt_fails_with_wrong_key() {
        let (nonce, ct) = aead_encrypt(&[1u8; 32], b"data", b"aad").unwrap();
        let err = aead_decrypt(&[2u8; 32], &nonce, &ct, b"aad").unwrap_err();
        assert!(matches!(err, AeadError::DecryptFailed));
    }

    #[test]
    fn decrypt_fails_with_tampered_ciphertext() {
        let key = [3u8; 32];
        let (nonce, mut ct) = aead_encrypt(&key, b"data", b"aad").unwrap();
        ct[0] ^= 0xFF;
        let err = aead_decrypt(&key, &nonce, &ct, b"aad").unwrap_err();
        assert!(matches!(err, AeadError::DecryptFailed));
    }

    #[test]
    fn decrypt_fails_with_wrong_aad() {
        let key = [4u8; 32];
        let (nonce, ct) = aead_encrypt(&key, b"data", b"original aad").unwrap();
        let err = aead_decrypt(&key, &nonce, &ct, b"different aad").unwrap_err();
        assert!(matches!(err, AeadError::DecryptFailed));
    }

    #[test]
    fn nonce_differs_between_encryptions() {
        let key = [5u8; 32];
        let (n1, _) = aead_encrypt(&key, b"data", b"").unwrap();
        let (n2, _) = aead_encrypt(&key, b"data", b"").unwrap();
        assert_ne!(n1, n2, "two encryptions must produce different nonces");
    }
}
