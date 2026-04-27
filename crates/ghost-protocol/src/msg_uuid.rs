//! MessageUuid — time-ordered 128-bit ID for messages, used for replay dedup.
//! Backed by UUID v7 (RFC 9562).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageUuid(Uuid);

impl MessageUuid {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }

    pub fn into_bytes(self) -> [u8; 16] {
        *self.0.as_bytes()
    }
}

impl Default for MessageUuid {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for MessageUuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MessageUuid({})", self.0)
    }
}

impl std::fmt::Display for MessageUuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn distinct_uuids_per_call() {
        let mut set = HashSet::new();
        for _ in 0..1000 {
            assert!(set.insert(MessageUuid::new()));
        }
    }

    #[test]
    fn roundtrip_bytes() {
        let original = MessageUuid::new();
        let bytes = original.into_bytes();
        let restored = MessageUuid::from_bytes(bytes);
        assert_eq!(original, restored);
    }

    #[test]
    fn time_ordered_within_same_millisecond_and_across() {
        let earlier = MessageUuid::new();
        thread::sleep(Duration::from_millis(2));
        let later = MessageUuid::new();
        assert!(earlier.as_bytes() < later.as_bytes());
    }

    #[test]
    fn cbor_roundtrip() {
        let original = MessageUuid::new();
        let mut buf = Vec::new();
        ciborium::into_writer(&original, &mut buf).unwrap();
        let restored: MessageUuid = ciborium::from_reader(&buf[..]).unwrap();
        assert_eq!(original, restored);
    }
}
