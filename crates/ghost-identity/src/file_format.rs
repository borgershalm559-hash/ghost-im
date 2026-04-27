//! On-disk encoding for `identity.encrypted`:
//! [0..4]    magic "GHST"
//! [4..5]    file format version u8 (currently 1)
//! [5..21]   16-byte Argon2 salt
//! [21..45]  24-byte XChaCha20 nonce
//! [45..]    AEAD ciphertext (with embedded 16-byte Poly1305 tag)
//!
//! AAD bound by the AEAD covers magic + version + salt + nonce, so a tampered
//! header makes decryption fail.

use crate::crypto::{NONCE_LEN, SALT_LEN};
use thiserror::Error;

pub const MAGIC: &[u8; 4] = b"GHST";
pub const FILE_FORMAT_VERSION: u8 = 1;
pub const HEADER_LEN: usize = 4 + 1 + SALT_LEN + NONCE_LEN; // 45

#[derive(Debug, Error, PartialEq)]
pub enum FileFormatError {
    #[error("file too short: {0} bytes (header is {} bytes)", HEADER_LEN)]
    Truncated(usize),
    #[error("bad magic bytes: expected GHST, got {0:?}")]
    BadMagic([u8; 4]),
    #[error("unsupported file format version {0} (this build supports {1})")]
    UnsupportedVersion(u8, u8),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub version: u8,
    pub salt: [u8; SALT_LEN],
    pub nonce: [u8; NONCE_LEN],
}

impl Header {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_LEN);
        out.extend_from_slice(MAGIC);
        out.push(self.version);
        out.extend_from_slice(&self.salt);
        out.extend_from_slice(&self.nonce);
        out
    }

    /// Parse a header from the start of `bytes`. Returns the parsed header on success.
    pub fn parse(bytes: &[u8]) -> Result<Self, FileFormatError> {
        if bytes.len() < HEADER_LEN {
            return Err(FileFormatError::Truncated(bytes.len()));
        }
        let mut magic = [0u8; 4];
        magic.copy_from_slice(&bytes[0..4]);
        if &magic != MAGIC {
            return Err(FileFormatError::BadMagic(magic));
        }
        let version = bytes[4];
        if version != FILE_FORMAT_VERSION {
            return Err(FileFormatError::UnsupportedVersion(
                version,
                FILE_FORMAT_VERSION,
            ));
        }
        let mut salt = [0u8; SALT_LEN];
        salt.copy_from_slice(&bytes[5..5 + SALT_LEN]);
        let mut nonce = [0u8; NONCE_LEN];
        nonce.copy_from_slice(&bytes[5 + SALT_LEN..HEADER_LEN]);
        Ok(Self {
            version,
            salt,
            nonce,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_encode_then_parse() {
        let h = Header {
            version: FILE_FORMAT_VERSION,
            salt: [7u8; 16],
            nonce: [9u8; 24],
        };
        let bytes = h.encode();
        assert_eq!(bytes.len(), HEADER_LEN);
        assert_eq!(&bytes[0..4], MAGIC);
        assert_eq!(bytes[4], FILE_FORMAT_VERSION);
        let parsed = Header::parse(&bytes).unwrap();
        assert_eq!(parsed, h);
    }

    #[test]
    fn parse_rejects_truncated() {
        let err = Header::parse(&[0u8; 10]).unwrap_err();
        assert!(matches!(err, FileFormatError::Truncated(10)));
    }

    #[test]
    fn parse_rejects_bad_magic() {
        let mut bytes = vec![0u8; HEADER_LEN];
        bytes[0..4].copy_from_slice(b"XXXX");
        bytes[4] = FILE_FORMAT_VERSION;
        let err = Header::parse(&bytes).unwrap_err();
        assert_eq!(err, FileFormatError::BadMagic(*b"XXXX"));
    }

    #[test]
    fn parse_rejects_future_version() {
        let mut bytes = vec![0u8; HEADER_LEN];
        bytes[0..4].copy_from_slice(MAGIC);
        bytes[4] = 99;
        let err = Header::parse(&bytes).unwrap_err();
        assert_eq!(err, FileFormatError::UnsupportedVersion(99, 1));
    }
}
