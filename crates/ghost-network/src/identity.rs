//! Map between Ghost identity types and libp2p identity types.

use crate::{NetworkError, Result};
use ghost_core::GhostId;
use ghost_identity::IdentityKey;
use libp2p::identity::{ed25519, Keypair, PeerId, PublicKey};

/// Build a libp2p [`Keypair`] from our [`IdentityKey`].
///
/// Both are Ed25519 over the same secret seed, so this is a 1:1 mapping.
/// The returned keypair shares the same public key (and therefore PeerId)
/// as the source `IdentityKey`.
pub fn libp2p_keypair_from_ik(ik: &IdentityKey) -> Result<Keypair> {
    // `ed25519_from_bytes` takes `impl AsMut<[u8]>` and zeroizes the buffer
    // on success. We copy the seed into a local mutable buffer.
    let mut seed = ik.secret_bytes();
    Keypair::ed25519_from_bytes(&mut seed)
        .map_err(|e| NetworkError::KeyConversion(format!("ed25519 keypair from seed: {e}")))
}

/// Convert a [`GhostId`] (raw 32-byte Ed25519 public key) to a libp2p [`PeerId`].
pub fn peer_id_from_ghost_id(id: &GhostId) -> Result<PeerId> {
    let ed_pub = ed25519::PublicKey::try_from_bytes(id.as_bytes())
        .map_err(|e| NetworkError::KeyConversion(format!("ed25519 public key: {e}")))?;
    let lp_pub = PublicKey::from(ed_pub);
    Ok(PeerId::from(lp_pub))
}

/// Convert a libp2p [`PeerId`] back to a [`GhostId`].
///
/// Only succeeds when the PeerId was constructed via identity-hash (i.e. from an
/// Ed25519 or similarly small key <= 42 bytes encoded). SHA-256-hashed PeerIds
/// are irreversible and will return an error.
pub fn ghost_id_from_peer_id(peer: &PeerId) -> Result<GhostId> {
    // PeerId::to_bytes() returns the multihash encoding.
    // For Ed25519 keys the encoded public key is ~36 bytes, which is under
    // MAX_INLINE_KEY_LENGTH (42), so libp2p uses identity-hash (code 0x00).
    // The multihash digest is the protobuf-encoded public key.
    use libp2p::multihash::Multihash;

    let peer_bytes = peer.to_bytes();
    let mh = Multihash::<64>::from_bytes(&peer_bytes)
        .map_err(|e| NetworkError::KeyConversion(format!("multihash decode: {e}")))?;

    if mh.code() != 0x00 {
        return Err(NetworkError::KeyConversion(
            "PeerId uses SHA-256 hash — public key is not recoverable".into(),
        ));
    }

    // The digest is the protobuf-encoded PublicKey.
    let lp_pub = PublicKey::try_decode_protobuf(mh.digest())
        .map_err(|e| NetworkError::KeyConversion(format!("protobuf public key: {e}")))?;

    let ed_pub = lp_pub
        .try_into_ed25519()
        .map_err(|_| NetworkError::KeyConversion("PeerId is not an Ed25519 key".into()))?;

    Ok(GhostId::from_bytes(ed_pub.to_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keypair_from_ik_yields_matching_peer_id() {
        let ik = IdentityKey::generate();
        let kp = libp2p_keypair_from_ik(&ik).unwrap();
        let from_kp = PeerId::from(kp.public());
        let from_id = peer_id_from_ghost_id(&ik.ghost_id()).unwrap();
        assert_eq!(
            from_kp, from_id,
            "PeerId via Keypair must equal PeerId via GhostId"
        );
    }

    #[test]
    fn peer_id_round_trip_via_ghost_id() {
        let ik = IdentityKey::generate();
        let original_id = ik.ghost_id();
        let peer = peer_id_from_ghost_id(&original_id).unwrap();
        let restored = ghost_id_from_peer_id(&peer).unwrap();
        assert_eq!(original_id, restored);
    }

    #[test]
    fn distinct_iks_yield_distinct_peer_ids() {
        let a = IdentityKey::generate();
        let b = IdentityKey::generate();
        let pa = peer_id_from_ghost_id(&a.ghost_id()).unwrap();
        let pb = peer_id_from_ghost_id(&b.ghost_id()).unwrap();
        assert_ne!(pa, pb);
    }

    #[test]
    fn keypair_signs_consistently_with_identity_key() {
        let ik = IdentityKey::generate();
        let kp = libp2p_keypair_from_ik(&ik).unwrap();
        let msg = b"hello libp2p";
        let lp_sig = kp.sign(msg).expect("sign");
        let ik_sig = ik.sign(msg).to_bytes();
        assert_eq!(
            lp_sig,
            ik_sig.to_vec(),
            "libp2p Keypair signs identically to IdentityKey \
             because they share the same Ed25519 secret seed"
        );
    }
}
