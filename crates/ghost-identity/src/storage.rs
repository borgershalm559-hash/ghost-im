//! Save/load Identity to/from an encrypted file using header + AEAD.

use crate::crypto::{aead_decrypt, aead_encrypt, derive_key, AeadError, KdfError, SALT_LEN};
use crate::file_format::{FileFormatError, Header, FILE_FORMAT_VERSION, HEADER_LEN};
use crate::identity::Identity;
use rand::RngCore;
use std::io;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("file format: {0}")]
    Format(#[from] FileFormatError),
    #[error("KDF: {0}")]
    Kdf(#[from] KdfError),
    #[error("AEAD: {0}")]
    Aead(#[from] AeadError),
    #[error("CBOR encode: {0}")]
    CborEncode(String),
    #[error("CBOR decode: {0}")]
    CborDecode(String),
}

/// `key_input` is the secret material (passphrase || keystore_secret) used for KDF.
/// Empty passphrase + non-empty keystore_secret is the default UX.
pub fn save(identity: &Identity, key_input: &[u8], path: &Path) -> Result<(), StorageError> {
    let mut salt = [0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    let key = derive_key(key_input, &salt)?;

    let mut plaintext = Vec::new();
    ciborium::into_writer(identity, &mut plaintext)
        .map_err(|e| StorageError::CborEncode(e.to_string()))?;

    // Build header so we can use it as AEAD AAD (binds header to ciphertext).
    let nonce_holder: [u8; crate::crypto::NONCE_LEN];
    let (nonce, ciphertext);
    {
        let (n, ct) = aead_encrypt(&key, &plaintext, b"ghost.identity.v1.aad")?;
        nonce_holder = n;
        nonce = nonce_holder;
        ciphertext = ct;
    }

    let header = Header {
        version: FILE_FORMAT_VERSION,
        salt,
        nonce,
    };
    let mut file_bytes = header.encode();
    file_bytes.extend_from_slice(&ciphertext);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, &file_bytes)?;
    Ok(())
}

pub fn load(key_input: &[u8], path: &Path) -> Result<Identity, StorageError> {
    let bytes = std::fs::read(path)?;
    if bytes.len() < HEADER_LEN {
        return Err(FileFormatError::Truncated(bytes.len()).into());
    }
    let header = Header::parse(&bytes[..HEADER_LEN])?;
    let ciphertext = &bytes[HEADER_LEN..];
    let key = derive_key(key_input, &header.salt)?;
    let plaintext = aead_decrypt(&key, &header.nonce, ciphertext, b"ghost.identity.v1.aad")?;
    let identity: Identity = ciborium::from_reader(&plaintext[..])
        .map_err(|e| StorageError::CborDecode(e.to_string()))?;
    Ok(identity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("identity.encrypted");
        let original = Identity::generate(Some("Alice".to_string()), 1700000000);
        let original_id = original.ghost_id();

        save(&original, b"keystore-secret-32-bytes-of-fluff", &path).unwrap();
        let loaded = load(b"keystore-secret-32-bytes-of-fluff", &path).unwrap();

        assert_eq!(loaded.ghost_id(), original_id);
        assert_eq!(loaded.display_name.as_deref(), Some("Alice"));
        assert_eq!(loaded.mls_keypackages.len(), original.mls_keypackages.len());
    }

    #[test]
    fn load_fails_with_wrong_key() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("identity.encrypted");
        let original = Identity::generate(None, 0);
        save(&original, b"correct-key", &path).unwrap();

        let err = load(b"wrong-key", &path).unwrap_err();
        assert!(matches!(err, StorageError::Aead(_)));
    }

    #[test]
    fn load_fails_with_tampered_header() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("identity.encrypted");
        let original = Identity::generate(None, 0);
        save(&original, b"key", &path).unwrap();

        // Corrupt one byte of salt (within header) — AAD covers header, so AEAD must fail.
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[10] ^= 0xFF;
        std::fs::write(&path, &bytes).unwrap();

        let err = load(b"key", &path).unwrap_err();
        assert!(matches!(err, StorageError::Aead(_)));
    }

    #[test]
    fn load_fails_on_bad_magic() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("identity.encrypted");
        // Must be >= HEADER_LEN bytes so the truncation check passes and
        // Header::parse reaches the magic-byte comparison.
        std::fs::write(&path, vec![0u8; HEADER_LEN]).unwrap();
        let err = load(b"key", &path).unwrap_err();
        assert!(matches!(
            err,
            StorageError::Format(FileFormatError::BadMagic(_))
        ));
    }

    #[test]
    fn save_creates_parent_directory() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested/dirs/identity.encrypted");
        let original = Identity::generate(None, 0);
        save(&original, b"key", &path).unwrap();
        assert!(path.exists());
    }
}
