//! Plan 05 Task 1: two Network instances on loopback using request/response API.

use ghost_identity::IdentityKey;
use ghost_network::Network;
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

    let payload = b"hello bob from alice".to_vec();

    // Drive both sides concurrently via select!:
    //   - Alice sends a request (send_to = request + discard response)
    //   - Bob receives InboundRequest, verifies it, and responds
    //
    // We use a flag to track which side is done. When Bob responds first,
    // Alice's future unblocks; when Alice sends first, Bob receives.
    let alice_done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let bob_done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    let alice_done2 = alice_done.clone();
    let bob_done2 = bob_done.clone();
    let payload_for_alice = payload.clone();
    let payload_for_bob = payload.clone();

    // Alice's future: send request and wait.
    let alice_fut = async move {
        alice
            .send_to(bob_peer_id, Some(bob_addr), payload_for_alice)
            .await
            .expect("alice send");
        alice_done2.store(true, std::sync::atomic::Ordering::SeqCst);
    };

    // Bob's future: receive request, verify, respond.
    let bob_fut = async move {
        let req = timeout(Duration::from_secs(10), bob.next_request())
            .await
            .expect("bob inbound timeout")
            .expect("bob received None");
        assert_eq!(req.sender, alice_peer_id, "sender PeerId must match Alice");
        assert_eq!(req.payload, payload_for_bob, "payload bytes must round-trip");
        bob.respond(req.response, Vec::new())
            .await
            .expect("bob respond");
        bob_done2.store(true, std::sync::atomic::Ordering::SeqCst);
    };

    // Run both concurrently. Both must complete within the deadline.
    timeout(
        Duration::from_secs(15),
        futures::future::join(alice_fut, bob_fut),
    )
    .await
    .expect("exchange timed out");

    assert!(alice_done.load(std::sync::atomic::Ordering::SeqCst));
    assert!(bob_done.load(std::sync::atomic::Ordering::SeqCst));
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
