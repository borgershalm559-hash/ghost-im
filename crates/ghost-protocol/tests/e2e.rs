//! Plan 02 deliverable: Alice and Bob exchange messages end-to-end.
//!
//! Flow:
//! 1. Alice and Bob create independent identities with KeyPackages
//! 2. Bob "publishes" a KeyPackage (in-memory stub mailbox)
//! 3. Alice picks up Bob's KeyPackage and creates an MLS group inviting Bob
//! 4. Bob processes the Welcome and joins
//! 5. Alice -> Bob: encrypt app msg, wrap in sealed-sender envelope, send wire bytes
//! 6. Bob receives wire bytes, unwraps, decrypts MLS, gets plaintext
//! 7. Bob -> Alice: reverse flow, verify plaintext

use ghost_identity::Identity;
use ghost_protocol::{
    delivery_public, new_provider, populate_initial_keypackages, unwrap_message, wrap_message,
    MlsSession, MsgType, PayloadType,
};
use openmls::prelude::tls_codec::{Deserialize as TlsDeserialize, Serialize as TlsSerialize};
use openmls::prelude::{KeyPackageIn, MlsMessageIn, ProtocolVersion};
use openmls_traits::OpenMlsProvider;

#[test]
fn alice_and_bob_full_exchange() {
    // ===== Setup =====
    let alice_provider = new_provider();
    let mut alice_id = Identity::generate(Some("Alice".into()), 1700000000);
    populate_initial_keypackages(&mut alice_id, &alice_provider, 2).unwrap();

    let bob_provider = new_provider();
    let mut bob_id = Identity::generate(Some("Bob".into()), 1700000000);
    populate_initial_keypackages(&mut bob_id, &bob_provider, 2).unwrap();

    // ===== Stub mailbox: Bob "publishes" a KeyPackage =====
    // Bob serialized his KeyPackages into bytes and "published" them.
    // Alice fetches the first one and deserializes it using KeyPackageIn -> validate.
    let bob_kp_bytes = bob_id
        .mls_keypackages
        .first()
        .expect("bob has key packages")
        .clone();

    // openmls 0.8 requires deserializing as KeyPackageIn first (for safety), then validating.
    let bob_kp_in = KeyPackageIn::tls_deserialize(&mut bob_kp_bytes.as_slice())
        .expect("deserialize bob's keypackage");
    let bob_kp = bob_kp_in
        .validate(alice_provider.crypto(), ProtocolVersion::Mls10)
        .expect("validate bob's keypackage");

    // ===== Alice creates the group, adds Bob =====
    let mut alice_session = MlsSession::create(
        &alice_provider,
        &alice_id.identity_key,
        &alice_id.device_key,
    )
    .expect("alice create session");
    let alice_signer = MlsSession::signer_from_dk(&alice_id.device_key);
    let invite = alice_session
        .add_member(&alice_provider, &alice_signer, bob_kp)
        .expect("alice add bob");

    // ===== Bob processes the Welcome, joins =====
    let welcome_bytes = invite.welcome.tls_serialize_detached().unwrap();
    let welcome_in = MlsMessageIn::tls_deserialize(&mut welcome_bytes.as_slice()).unwrap();
    let mut bob_session =
        MlsSession::join_via_welcome(&bob_provider, welcome_in).expect("bob join");

    assert_eq!(alice_session.epoch(), 1);
    assert_eq!(bob_session.epoch(), 1);

    // ===== Alice -> Bob: encrypt MLS, wrap envelope, send =====
    let mls_ct = alice_session
        .encrypt_app_message(&alice_provider, &alice_signer, b"hello bob")
        .unwrap();

    let bob_delivery = delivery_public(&bob_id.identity_key);
    let wire_alice_to_bob = wrap_message(
        &alice_id.identity_key,
        &alice_id.device_key,
        bob_id.ghost_id(),
        &bob_delivery,
        MsgType::AppMessage,
        PayloadType::AppText,
        mls_ct,
        1700000060,
    )
    .unwrap();

    // ===== Bob: unwrap envelope, decrypt MLS =====
    let alice_dk_pub = alice_id.device_key.public();
    let unwrapped = unwrap_message(&wire_alice_to_bob, &bob_id.identity_key, |id| {
        if id == &alice_id.ghost_id() {
            Some(alice_dk_pub)
        } else {
            None
        }
    })
    .unwrap();
    assert_eq!(unwrapped.sender_id, alice_id.ghost_id());
    assert_eq!(unwrapped.payload_type, PayloadType::AppText);

    let plaintext = bob_session
        .decrypt_app_message(&bob_provider, &unwrapped.payload)
        .unwrap();
    assert_eq!(plaintext, b"hello bob");

    // ===== Bob -> Alice: reverse flow =====
    let bob_signer = MlsSession::signer_from_dk(&bob_id.device_key);
    let mls_ct_back = bob_session
        .encrypt_app_message(&bob_provider, &bob_signer, b"hi alice")
        .unwrap();

    let alice_delivery = delivery_public(&alice_id.identity_key);
    let wire_bob_to_alice = wrap_message(
        &bob_id.identity_key,
        &bob_id.device_key,
        alice_id.ghost_id(),
        &alice_delivery,
        MsgType::AppMessage,
        PayloadType::AppText,
        mls_ct_back,
        1700000120,
    )
    .unwrap();

    let bob_dk_pub = bob_id.device_key.public();
    let unwrapped_back = unwrap_message(&wire_bob_to_alice, &alice_id.identity_key, |id| {
        if id == &bob_id.ghost_id() {
            Some(bob_dk_pub)
        } else {
            None
        }
    })
    .unwrap();
    let plaintext_back = alice_session
        .decrypt_app_message(&alice_provider, &unwrapped_back.payload)
        .unwrap();
    assert_eq!(plaintext_back, b"hi alice");
}
