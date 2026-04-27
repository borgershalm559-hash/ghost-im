//! Plan 06 deliverable: two Client instances exchange E2EE messages over loopback.

use ghost_client::Client;
use ghost_identity::Identity;
use libp2p::Multiaddr;
use std::time::Duration;
use tempfile::tempdir;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn alice_and_bob_full_messaging_flow() {
    let dir = tempdir().unwrap();
    let alice_db_path = dir.path().join("alice.db");
    let bob_db_path = dir.path().join("bob.db");

    let alice_id = Identity::generate(Some("Alice".into()), 1700000000);
    let bob_id = Identity::generate(Some("Bob".into()), 1700000000);
    let alice_ghost_id = alice_id.identity_key.ghost_id();
    let bob_ghost_id = bob_id.identity_key.ghost_id();

    let alice_listen: Multiaddr = "/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap();
    let bob_listen: Multiaddr = "/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap();

    let alice = Client::open_with_in_memory_identity(alice_id, alice_db_path, alice_listen)
        .await
        .expect("alice open");
    let bob = Client::open_with_in_memory_identity(bob_id, bob_db_path, bob_listen)
        .await
        .expect("bob open");

    let _alice_inbox = alice.start_inbox_processor().await.expect("alice inbox");
    let _bob_inbox = bob.start_inbox_processor().await.expect("bob inbox");

    // Step 1: Alice creates invite, Bob accepts.
    let alice_invite = alice
        .create_invite(3600)
        .expect("create invite")
        .to_bech32()
        .expect("invite to bech32");
    bob.add_contact(&alice_invite).await.expect("bob add contact");

    // Wait for Alice's inbox processor to handle the Welcome.
    tokio::time::sleep(Duration::from_millis(800)).await;

    // Step 2: Verify Alice has Bob as a contact.
    let alice_contacts = alice.list_contacts().expect("list alice contacts");
    assert!(
        alice_contacts.iter().any(|c| c.ghost_id == bob_ghost_id),
        "Alice should have Bob as a contact after Welcome processing. \
         Contacts: {:?}",
        alice_contacts
            .iter()
            .map(|c| format!("{}", c.ghost_id))
            .collect::<Vec<_>>()
    );

    // Step 3: Bob sends "hello alice".
    bob.send_message(alice_ghost_id, "hello alice")
        .await
        .expect("bob send hello");
    tokio::time::sleep(Duration::from_millis(500)).await;

    let alice_messages = alice
        .list_messages(&bob_ghost_id, 10, 0)
        .expect("alice list messages");
    assert_eq!(
        alice_messages.len(),
        1,
        "Alice should have 1 received message. Got: {:?}",
        alice_messages.iter().map(|m| &m.content).collect::<Vec<_>>()
    );
    assert_eq!(alice_messages[0].content, "hello alice");

    // Step 4: Alice replies "hi bob".
    alice
        .send_message(bob_ghost_id, "hi bob")
        .await
        .expect("alice send hi");
    tokio::time::sleep(Duration::from_millis(500)).await;

    let bob_messages = bob
        .list_messages(&alice_ghost_id, 10, 0)
        .expect("bob list messages");
    // Bob has 2 messages: 1 outgoing ("hello alice") + 1 incoming ("hi bob").
    assert_eq!(
        bob_messages.len(),
        2,
        "Bob should have 2 messages. Got: {:?}",
        bob_messages.iter().map(|m| &m.content).collect::<Vec<_>>()
    );
    assert!(
        bob_messages.iter().any(|m| m.content == "hi bob"),
        "Bob's messages should include 'hi bob'"
    );
    assert!(
        bob_messages.iter().any(|m| m.content == "hello alice"),
        "Bob's messages should include 'hello alice' (outgoing)"
    );
}
