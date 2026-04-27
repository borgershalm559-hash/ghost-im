//! Pre-keys: X25519 one-time keypairs published to allow asynchronous first-contact.
//! In MVP-1 we generate a batch of one-time pre-keys plus one last-resort key.

use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use x25519_dalek::{PublicKey, StaticSecret};

#[derive(Serialize, Deserialize)]
pub struct PreKey {
    pub id: u32,
    pub secret: StaticSecret,
    pub is_last_resort: bool,
    pub created_at: u64,
}

impl PreKey {
    pub fn new(id: u32, is_last_resort: bool, created_at: u64) -> Self {
        Self {
            id,
            secret: StaticSecret::random_from_rng(&mut OsRng),
            is_last_resort,
            created_at,
        }
    }

    pub fn public(&self) -> PublicKey {
        PublicKey::from(&self.secret)
    }
}

impl std::fmt::Debug for PreKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PreKey(id={}, last_resort={}, pub={}…)",
            self.id,
            self.is_last_resort,
            hex::encode(&self.public().as_bytes()[..4])
        )
    }
}

/// Generate `count` one-time pre-keys plus one last-resort pre-key.
/// IDs are assigned sequentially starting from `start_id`.
pub fn generate_batch(count: u32, start_id: u32, now: u64) -> (Vec<PreKey>, PreKey) {
    let mut ones = Vec::with_capacity(count as usize);
    for i in 0..count {
        ones.push(PreKey::new(start_id + i, false, now));
    }
    let last_resort = PreKey::new(start_id + count, true, now);
    (ones, last_resort)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_prekey_has_distinct_public() {
        let a = PreKey::new(1, false, 0);
        let b = PreKey::new(2, false, 0);
        assert_ne!(a.public().as_bytes(), b.public().as_bytes());
    }

    #[test]
    fn generate_batch_produces_correct_count_and_ids() {
        let (ones, last_resort) = generate_batch(10, 100, 1700000000);
        assert_eq!(ones.len(), 10);
        for (i, key) in ones.iter().enumerate() {
            assert_eq!(key.id, 100 + i as u32);
            assert!(!key.is_last_resort);
            assert_eq!(key.created_at, 1700000000);
        }
        assert_eq!(last_resort.id, 110);
        assert!(last_resort.is_last_resort);
    }

    #[test]
    fn debug_does_not_leak_secret() {
        let pk = PreKey::new(42, false, 0);
        let dbg = format!("{:?}", pk);
        assert!(dbg.starts_with("PreKey(id=42"));
        assert!(dbg.contains("last_resort=false"));
        // No 64-char hex string of the secret.
        let secret_hex = hex::encode(pk.secret.as_bytes());
        assert!(!dbg.contains(&secret_hex));
    }
}
