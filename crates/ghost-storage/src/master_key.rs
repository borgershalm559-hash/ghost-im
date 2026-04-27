//! Derive the SQLCipher master key from an IdentityKey.
//!
//! Holders of the IK can derive both halves of this key deterministically.
//! Without the IK there is no path to recover this key.

use ghost_identity::IdentityKey;
use hkdf::Hkdf;
use sha2::Sha256;

const HKDF_SALT: &[u8] = b"ghost.db.encryption.v1";
const HKDF_INFO: &[u8] = b"sqlcipher-master-key";
pub const MASTER_KEY_LEN: usize = 32;

/// Derive a 32-byte SQLCipher master key from `ik`.
/// Deterministic: same IK -> same key, on every machine, every time.
pub fn derive_master_key(ik: &IdentityKey) -> [u8; MASTER_KEY_LEN] {
    let seed = ik.secret_bytes();
    let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT), &seed);
    let mut okm = [0u8; MASTER_KEY_LEN];
    hk.expand(HKDF_INFO, &mut okm)
        .expect("32-byte expand always succeeds");
    okm
}

/// Format the key as a SQLCipher hex literal: `x'...'`. SQLCipher accepts this form
/// in `PRAGMA key = ...` and skips its built-in PBKDF2 (since the key is already
/// derived material).
pub fn master_key_pragma(key: &[u8; MASTER_KEY_LEN]) -> String {
    format!("x'{}'", hex::encode(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_is_deterministic_per_identity() {
        let ik = IdentityKey::generate();
        let k1 = derive_master_key(&ik);
        let k2 = derive_master_key(&ik);
        assert_eq!(k1, k2);
    }

    #[test]
    fn distinct_identities_yield_distinct_keys() {
        let a = IdentityKey::generate();
        let b = IdentityKey::generate();
        assert_ne!(derive_master_key(&a), derive_master_key(&b));
    }

    #[test]
    fn pragma_format_matches_sqlcipher_hex_literal() {
        let key = [0xABu8; MASTER_KEY_LEN];
        let pragma = master_key_pragma(&key);
        assert!(pragma.starts_with("x'"));
        assert!(pragma.ends_with('\''));
        assert_eq!(pragma.len(), 32 * 2 + 3);
        assert!(pragma.contains(&"ab".repeat(32)));
    }

    #[test]
    fn master_key_differs_from_identity_secret_bytes() {
        let ik = IdentityKey::generate();
        let key = derive_master_key(&ik);
        assert_ne!(key, ik.secret_bytes());
    }
}
