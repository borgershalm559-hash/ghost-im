//! High-level envelope API: bundle SealedBlob signing, sealed-sender encryption,
//! and OuterEnvelope CBOR into single `wrap_message` / `unwrap_message` calls.

use crate::msg_uuid::MessageUuid;
use crate::outer_envelope::{MsgType, OuterEnvelope};
use crate::sealed_blob::{PayloadType, SealedBlob};
use crate::sealed_sender::{delivery_secret, seal_to, unseal};
use crate::{ProtoError, Result};
use ed25519_dalek::{Signature, Verifier};
use ghost_core::GhostId;
use ghost_identity::{DeviceKey, IdentityKey};

/// Bundle of values returned to the caller after `unwrap_message` succeeds.
#[derive(Debug)]
pub struct UnwrappedMessage {
    pub sender_id: GhostId,
    pub payload_type: PayloadType,
    pub payload: Vec<u8>,
    pub msg_uuid: MessageUuid,
}

/// Wrap an outgoing message: produces wire bytes (CBOR-encoded OuterEnvelope) intended for `recipient`.
///
/// Steps:
/// 1. Sign (sender_id || payload_type || payload || msg_uuid) with sender's DK.
/// 2. Build SealedBlob, CBOR-encode.
/// 3. Sealed-sender-encrypt to recipient's delivery pubkey.
/// 4. Wrap in OuterEnvelope, CBOR-encode.
#[allow(clippy::too_many_arguments)]
pub fn wrap_message(
    sender_ik: &IdentityKey,
    sender_dk: &DeviceKey,
    recipient_id: GhostId,
    recipient_delivery_pub: &x25519_dalek::PublicKey,
    msg_type: MsgType,
    payload_type: PayloadType,
    payload: Vec<u8>,
    timestamp: u64,
) -> Result<Vec<u8>> {
    let sender_id = sender_ik.ghost_id();
    let msg_uuid = MessageUuid::new();

    let signing_bytes =
        SealedBlob::signing_bytes(&sender_id, payload_type, &payload, &msg_uuid);
    let signature = sender_dk.sign(&signing_bytes);

    let blob = SealedBlob {
        sender_id,
        payload_type,
        payload,
        msg_uuid,
        sender_signature: signature.to_bytes(),
    };
    let blob_bytes = blob.to_cbor()?;
    let sealed = seal_to(recipient_delivery_pub, &blob_bytes)?;

    let outer = OuterEnvelope::new(msg_type, recipient_id, timestamp, sealed);
    outer.to_cbor()
}

/// Unwrap an incoming envelope: parse, verify recipient, decrypt sealed-sender, parse SealedBlob,
/// verify sender signature, return plaintext payload + sender info.
///
/// `sender_dk_for_check` is a callback the caller supplies: given the inner sender's GhostId,
/// return the sender's known DK public bytes for signature verification (typically pulled from a
/// contact-key store or from MLS group state).
pub fn unwrap_message<F>(
    wire_bytes: &[u8],
    recipient_ik: &IdentityKey,
    sender_dk_for_check: F,
) -> Result<UnwrappedMessage>
where
    F: FnOnce(&GhostId) -> Option<ed25519_dalek::VerifyingKey>,
{
    let outer = OuterEnvelope::from_cbor(wire_bytes)?;
    outer.check_recipient(&recipient_ik.ghost_id())?;

    let recipient_secret = delivery_secret(recipient_ik);
    let blob_bytes = unseal(&recipient_secret, &outer.sealed_blob)?;
    let blob = SealedBlob::from_cbor(&blob_bytes)?;

    let sender_dk_pub = sender_dk_for_check(&blob.sender_id).ok_or_else(|| {
        ProtoError::Invalid(format!("no known DK for sender {}", blob.sender_id))
    })?;
    let signing_bytes = SealedBlob::signing_bytes(
        &blob.sender_id,
        blob.payload_type,
        &blob.payload,
        &blob.msg_uuid,
    );
    let sig = Signature::from_bytes(&blob.sender_signature);
    sender_dk_pub
        .verify(&signing_bytes, &sig)
        .map_err(|_| ProtoError::BadSenderSignature)?;

    Ok(UnwrappedMessage {
        sender_id: blob.sender_id,
        payload_type: blob.payload_type,
        payload: blob.payload,
        msg_uuid: blob.msg_uuid,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ghost_identity::{DeviceKey, IdentityKey};

    fn alice_and_bob() -> (
        (IdentityKey, DeviceKey),
        (IdentityKey, DeviceKey),
    ) {
        let alice_ik = IdentityKey::generate();
        let alice_dk = DeviceKey::generate(&alice_ik);
        let bob_ik = IdentityKey::generate();
        let bob_dk = DeviceKey::generate(&bob_ik);
        ((alice_ik, alice_dk), (bob_ik, bob_dk))
    }

    #[test]
    fn wrap_then_unwrap_roundtrip() {
        let ((alice_ik, alice_dk), (bob_ik, _bob_dk)) = alice_and_bob();
        let alice_id = alice_ik.ghost_id();
        let bob_id = bob_ik.ghost_id();
        let bob_delivery = crate::sealed_sender::delivery_public(&bob_ik);
        let alice_dk_pub = alice_dk.public();

        let wire = wrap_message(
            &alice_ik,
            &alice_dk,
            bob_id,
            &bob_delivery,
            MsgType::AppMessage,
            PayloadType::AppText,
            b"hello bob".to_vec(),
            1700000000,
        )
        .unwrap();

        let result = unwrap_message(&wire, &bob_ik, |id| {
            if id == &alice_id {
                Some(alice_dk_pub)
            } else {
                None
            }
        })
        .unwrap();

        assert_eq!(result.sender_id, alice_id);
        assert_eq!(result.payload_type, PayloadType::AppText);
        assert_eq!(result.payload, b"hello bob");
    }

    #[test]
    fn unwrap_fails_when_recipient_mismatch() {
        let ((alice_ik, alice_dk), (bob_ik, _)) = alice_and_bob();
        let charlie_ik = IdentityKey::generate();
        let bob_id = bob_ik.ghost_id();
        let bob_delivery = crate::sealed_sender::delivery_public(&bob_ik);

        let wire = wrap_message(
            &alice_ik,
            &alice_dk,
            bob_id,
            &bob_delivery,
            MsgType::AppMessage,
            PayloadType::AppText,
            b"data".to_vec(),
            1,
        )
        .unwrap();

        let err = unwrap_message(&wire, &charlie_ik, |_| None).unwrap_err();
        assert!(matches!(err, ProtoError::WrongRecipient { .. }));
    }

    #[test]
    fn unwrap_fails_when_sender_dk_unknown() {
        let ((alice_ik, alice_dk), (bob_ik, _)) = alice_and_bob();
        let bob_id = bob_ik.ghost_id();
        let bob_delivery = crate::sealed_sender::delivery_public(&bob_ik);

        let wire = wrap_message(
            &alice_ik,
            &alice_dk,
            bob_id,
            &bob_delivery,
            MsgType::AppMessage,
            PayloadType::AppText,
            b"data".to_vec(),
            1,
        )
        .unwrap();

        let err = unwrap_message(&wire, &bob_ik, |_| None).unwrap_err();
        assert!(matches!(err, ProtoError::Invalid(_)));
    }

    #[test]
    fn unwrap_fails_on_tampered_payload() {
        let ((alice_ik, alice_dk), (bob_ik, _)) = alice_and_bob();
        let alice_id = alice_ik.ghost_id();
        let alice_dk_pub = alice_dk.public();
        let bob_id = bob_ik.ghost_id();
        let bob_delivery = crate::sealed_sender::delivery_public(&bob_ik);

        let mut wire = wrap_message(
            &alice_ik,
            &alice_dk,
            bob_id,
            &bob_delivery,
            MsgType::AppMessage,
            PayloadType::AppText,
            b"data".to_vec(),
            1,
        )
        .unwrap();
        let last = wire.len() - 1;
        wire[last] ^= 0xFF;

        let err = unwrap_message(&wire, &bob_ik, |id| {
            if id == &alice_id {
                Some(alice_dk_pub)
            } else {
                None
            }
        })
        .unwrap_err();
        assert!(matches!(err, ProtoError::SealedSenderDecrypt));
    }
}
