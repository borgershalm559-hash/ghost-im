//! Plan 03 deliverable: messaging round-trips across a simulated process restart.

use ghost_core::Fingerprint;
use ghost_identity::Identity;
use ghost_protocol::{
    delivery_public, new_provider, populate_initial_keypackages, unwrap_message, wrap_message,
    MlsSession, MsgType, PayloadType,
};
use ghost_storage::{derive_master_key, Contact, Database, MlsGroupRow, Verification};
use openmls::prelude::tls_codec::{Deserialize as TlsDeserialize, Serialize as TlsSerialize};
use openmls::prelude::{KeyPackageIn, MlsMessageIn};
use openmls_traits::OpenMlsProvider;
use tempfile::tempdir;

#[test]
fn alice_and_bob_persist_and_continue_after_restart() {
    let dir = tempdir().unwrap();
    let alice_db_path = dir.path().join("alice.db");
    let bob_db_path = dir.path().join("bob.db");

    // ===== Identity setup =====
    let mut alice_id = Identity::generate(Some("Alice".into()), 1700000000);
    let mut bob_id = Identity::generate(Some("Bob".into()), 1700000000);

    let alice_key = derive_master_key(&alice_id.identity_key);
    let bob_key = derive_master_key(&bob_id.identity_key);

    // ===== First session: Plan 02 exchange + persist state =====
    let alice_state_bytes_v1: Vec<u8>;
    let bob_state_bytes_v1: Vec<u8>;

    {
        let alice_db = Database::open_encrypted(&alice_db_path, &alice_key).unwrap();
        alice_db.migrate().unwrap();
        let bob_db = Database::open_encrypted(&bob_db_path, &bob_key).unwrap();
        bob_db.migrate().unwrap();

        // Add each other as contacts.
        let alice_fp = Fingerprint::of(&alice_id.ghost_id()).to_string();
        let bob_fp = Fingerprint::of(&bob_id.ghost_id()).to_string();
        alice_db
            .contacts()
            .insert(&Contact {
                ghost_id: bob_id.ghost_id(),
                display_name: Some("Bob".into()),
                local_alias: None,
                fingerprint: bob_fp.clone(),
                added_at: 1700000000,
                last_endpoint: None,
                verification: Verification::Unverified,
                notes: None,
                blocked: false,
            })
            .unwrap();
        bob_db
            .contacts()
            .insert(&Contact {
                ghost_id: alice_id.ghost_id(),
                display_name: Some("Alice".into()),
                local_alias: None,
                fingerprint: alice_fp.clone(),
                added_at: 1700000000,
                last_endpoint: None,
                verification: Verification::Unverified,
                notes: None,
                blocked: false,
            })
            .unwrap();

        // MLS providers + KeyPackages
        let alice_provider = new_provider();
        let bob_provider = new_provider();
        populate_initial_keypackages(&mut alice_id, &alice_provider, 2).unwrap();
        populate_initial_keypackages(&mut bob_id, &bob_provider, 2).unwrap();

        // Alice fetches Bob's KP, validates, then adds Bob.
        let bob_kp_bytes = bob_id.mls_keypackages.first().unwrap().clone();
        let bob_kp_in = KeyPackageIn::tls_deserialize(&mut bob_kp_bytes.as_slice()).unwrap();
        let bob_kp = bob_kp_in
            .validate(alice_provider.crypto(), openmls::versions::ProtocolVersion::Mls10)
            .expect("validate bob KP");

        let mut alice_session =
            MlsSession::create(&alice_provider, &alice_id.identity_key, &alice_id.device_key)
                .unwrap();
        let alice_signer = MlsSession::signer_from_dk(&alice_id.device_key);
        let invite = alice_session
            .add_member(&alice_provider, &alice_signer, bob_kp)
            .unwrap();

        let welcome_bytes = invite.welcome.tls_serialize_detached().unwrap();
        let welcome_in = MlsMessageIn::tls_deserialize(&mut welcome_bytes.as_slice()).unwrap();
        let mut bob_session = MlsSession::join_via_welcome(&bob_provider, welcome_in).unwrap();

        // Round 1 exchange: alice -> bob.
        let mls_ct = alice_session
            .encrypt_app_message(&alice_provider, &alice_signer, b"hello bob")
            .unwrap();
        let bob_delivery = delivery_public(&bob_id.identity_key);
        let wire = wrap_message(
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
        let alice_dk_pub = alice_id.device_key.public();
        let unwrapped = unwrap_message(&wire, &bob_id.identity_key, |id| {
            (id == &alice_id.ghost_id()).then_some(alice_dk_pub)
        })
        .unwrap();
        let plaintext = bob_session
            .decrypt_app_message(&bob_provider, &unwrapped.payload)
            .unwrap();
        assert_eq!(plaintext, b"hello bob");

        // ===== Persist MLS state =====
        alice_state_bytes_v1 = alice_session.serialize_state(&alice_provider).unwrap();
        bob_state_bytes_v1 = bob_session.serialize_state(&bob_provider).unwrap();

        let alice_group_id = alice_session.group_id_bytes();
        let bob_group_id = bob_session.group_id_bytes();

        let now = 1700000100;
        alice_db
            .mls_groups()
            .upsert(&MlsGroupRow {
                group_id: bytes_to_array_32(&alice_group_id),
                contact_id: bob_id.ghost_id(),
                state_blob: alice_state_bytes_v1.clone(),
                current_epoch: alice_session.current_epoch(),
                created_at: now,
                last_updated: now,
            })
            .unwrap();
        bob_db
            .mls_groups()
            .upsert(&MlsGroupRow {
                group_id: bytes_to_array_32(&bob_group_id),
                contact_id: alice_id.ghost_id(),
                state_blob: bob_state_bytes_v1.clone(),
                current_epoch: bob_session.current_epoch(),
                created_at: now,
                last_updated: now,
            })
            .unwrap();

        // Drop everything to simulate process restart.
        drop(alice_session);
        drop(bob_session);
        drop(alice_provider);
        drop(bob_provider);
        drop(alice_db);
        drop(bob_db);
    }

    // ===== "Restart": reopen DBs, restore sessions, continue messaging =====
    let alice_db = Database::open_encrypted(&alice_db_path, &alice_key).unwrap();
    let bob_db = Database::open_encrypted(&bob_db_path, &bob_key).unwrap();

    let alice_state_loaded = alice_db
        .mls_groups()
        .load_for_contact(&bob_id.ghost_id())
        .unwrap()
        .expect("alice state present");
    let bob_state_loaded = bob_db
        .mls_groups()
        .load_for_contact(&alice_id.ghost_id())
        .unwrap()
        .expect("bob state present");
    assert_eq!(alice_state_loaded.state_blob, alice_state_bytes_v1);
    assert_eq!(bob_state_loaded.state_blob, bob_state_bytes_v1);

    // Restore sessions. Each call returns a (provider, session) tuple — both must stay alive.
    let (alice_provider, mut alice_session) =
        MlsSession::deserialize_state(&alice_state_loaded.state_blob).unwrap();
    let (bob_provider, mut bob_session) =
        MlsSession::deserialize_state(&bob_state_loaded.state_blob).unwrap();

    // Re-store the signers into the restored providers' storage.
    let alice_signer = MlsSession::signer_from_dk(&alice_id.device_key);
    alice_signer
        .store(alice_provider.storage())
        .expect("re-store alice signer");

    // Round 2 exchange: alice (restored) -> bob (restored).
    let mls_ct = alice_session
        .encrypt_app_message(&alice_provider, &alice_signer, b"after restart")
        .unwrap();
    let bob_delivery = delivery_public(&bob_id.identity_key);
    let wire = wrap_message(
        &alice_id.identity_key,
        &alice_id.device_key,
        bob_id.ghost_id(),
        &bob_delivery,
        MsgType::AppMessage,
        PayloadType::AppText,
        mls_ct,
        1700000200,
    )
    .unwrap();
    let alice_dk_pub = alice_id.device_key.public();
    let unwrapped = unwrap_message(&wire, &bob_id.identity_key, |id| {
        (id == &alice_id.ghost_id()).then_some(alice_dk_pub)
    })
    .unwrap();
    let plaintext = bob_session
        .decrypt_app_message(&bob_provider, &unwrapped.payload)
        .unwrap();
    assert_eq!(plaintext, b"after restart");
}

/// MLS group IDs are arbitrary bytes (size depends on openmls config). We BLAKE3-hash
/// to a fixed 32-byte key so it fits the schema's `group_id BLOB` PK with
/// deterministic length.
fn bytes_to_array_32(b: &[u8]) -> [u8; 32] {
    *blake3::hash(b).as_bytes()
}
