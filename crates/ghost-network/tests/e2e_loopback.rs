//! Plan 04 deliverable: two Network instances on loopback exchange bytes.

use ghost_identity::IdentityKey;
use ghost_network::{InboundEvent, Network};
use libp2p::Multiaddr;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn alice_and_bob_exchange_bytes() {
    let alice_ik = IdentityKey::generate();
    let bob_ik = IdentityKey::generate();

    let alice = Network::spawn(&alice_ik).await.expect("alice spawn");
    let mut bob = Network::spawn(&bob_ik).await.expect("bob spawn");

    let alice_peer_id = alice.local_peer_id();
    let bob_peer_id = bob.local_peer_id();

    // Bob listens on a loopback QUIC port.
    let listen_addr: Multiaddr = "/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap();
    let _bob_listener = bob.listen_on(listen_addr).await.expect("bob listen");

    // Wait for Bob's listener to bind and surface the chosen address.
    let bob_addrs = wait_for_addrs(&bob, Duration::from_secs(5)).await;
    assert!(
        !bob_addrs.is_empty(),
        "bob should have at least one local address"
    );
    let bob_addr = bob_addrs.into_iter().next().unwrap();
    println!("bob address: {bob_addr}");

    // Alice sends bytes to Bob with explicit endpoint (no DHT in this test).
    let payload = b"hello bob from alice".to_vec();
    alice
        .send_to(bob_peer_id, Some(bob_addr), payload.clone())
        .await
        .expect("alice send");

    // Bob receives.
    let event = timeout(Duration::from_secs(10), bob.next_inbound())
        .await
        .expect("inbound timeout")
        .expect("bob received None");
    match event {
        InboundEvent::Message {
            sender,
            payload: rx_payload,
        } => {
            assert_eq!(sender, alice_peer_id, "sender PeerId must match Alice");
            assert_eq!(rx_payload, payload, "payload bytes must round-trip");
        }
    }
}

async fn wait_for_addrs(net: &Network, deadline: Duration) -> Vec<Multiaddr> {
    let start = tokio::time::Instant::now();
    loop {
        let addrs = net.local_addrs().await;
        if !addrs.is_empty() {
            return addrs;
        }
        if start.elapsed() > deadline {
            return Vec::new();
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
