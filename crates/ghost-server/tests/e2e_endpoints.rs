//! Plan 05 deliverable: Bob's Client queries all five endpoints on Alice's Server.

use ghost_identity::Identity;
use ghost_network::Network;
use ghost_protocol::{delivery_public, new_provider, populate_initial_keypackages};
use ghost_network::NetworkInbox;
use ghost_server::{Client, PresenceState, Server};
use ghost_storage::{derive_master_key, Database, MyKeyPackageRow};
use libp2p::Multiaddr;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::sync::Mutex;
use tokio::time::timeout;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bob_queries_all_endpoints_on_alice() {
    let dir = tempdir().unwrap();
    let alice_db_path = dir.path().join("alice.db");

    // ===== Alice setup =====
    let mut alice_id = Identity::generate(Some("Alice".into()), 1700000000);

    // Derive the master key from Alice's IK before moving it into Arc.
    let alice_master = derive_master_key(&alice_id.identity_key);

    // Populate KeyPackages into alice_id.mls_keypackages.
    let provider = new_provider();
    populate_initial_keypackages(&mut alice_id, &provider, 3).unwrap();

    // Open Alice's DB, migrate, and insert the populated KeyPackages.
    let alice_db = {
        let db = Database::open_encrypted(&alice_db_path, &alice_master).unwrap();
        db.migrate().unwrap();
        for kp_bytes in &alice_id.mls_keypackages {
            let pkg_id = *blake3::hash(kp_bytes).as_bytes();
            db.my_keypackages()
                .insert(&MyKeyPackageRow {
                    package_id: pkg_id,
                    package_blob: kp_bytes.clone(),
                    private_key: vec![],
                    created_at: 1700000000,
                    consumed_at: None,
                    is_last_resort: false,
                })
                .unwrap();
        }
        db
    };
    let alice_db = Arc::new(alice_db);

    // Build Arc<IdentityKey> for Alice from the secret bytes of her IK.
    // IdentityKey is not Clone, so we reconstruct from the secret seed.
    let alice_ik = Arc::new(ghost_identity::IdentityKey::from_secret_bytes(
        alice_id.identity_key.secret_bytes(),
    ));

    let alice_network = Arc::new(Mutex::new(Network::spawn(&alice_ik).await.unwrap()));
    let alice_peer_id = alice_network.lock().await.local_peer_id();

    // Listen on loopback with an OS-assigned port.
    let listen_addr: Multiaddr = "/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap();
    alice_network
        .lock()
        .await
        .listen_on(listen_addr)
        .await
        .unwrap();
    let alice_addr = wait_for_addr(&alice_network).await;

    // Set Alice's presence.
    let alice_presence = Arc::new(Mutex::new(PresenceState {
        online: true,
        last_seen: 1700000060,
    }));

    // Split the inbox receiver out before spawning the server so the server
    // loop can drain requests without holding the network mutex.
    let alice_inbox = alice_network.lock().await.split_inbox();

    // Spawn Alice's Server.
    let mut alice_server = Server::spawn(
        alice_ik.clone(),
        alice_inbox,
        alice_network.clone(),
        alice_presence.clone(),
        alice_db.clone(),
    )
    .unwrap();

    // ===== Bob setup =====
    let bob_id = Identity::generate(Some("Bob".into()), 1700000000);
    let bob_ik = Arc::new(ghost_identity::IdentityKey::from_secret_bytes(
        bob_id.identity_key.secret_bytes(),
    ));
    let bob_network = Arc::new(Mutex::new(Network::spawn(&bob_ik).await.unwrap()));
    let bob_client = Client::new(bob_network.clone());

    // ===== 1. Version =====
    let (proto, min_compat) = bob_client
        .get_version(alice_peer_id, Some(alice_addr.clone()))
        .await
        .expect("get_version");
    assert_eq!(proto, "ghost/1");
    assert_eq!(min_compat, "ghost/1");

    // ===== 2. DeliveryKey =====
    let dk_remote = bob_client
        .get_delivery_key(alice_peer_id, Some(alice_addr.clone()))
        .await
        .expect("get_delivery_key");
    let dk_local = *delivery_public(&alice_ik).as_bytes();
    assert_eq!(
        dk_remote, dk_local,
        "remote delivery key must match local computation"
    );

    // ===== 3. KeyPackage (first call) — one of the three populated KPs =====
    let kp1 = bob_client
        .get_key_package(alice_peer_id, Some(alice_addr.clone()))
        .await
        .expect("get_key_package first");
    assert!(!kp1.is_empty(), "first KeyPackage must be non-empty");

    // ===== 4. KeyPackage (second call) — must be a DIFFERENT KP (proves consumption) =====
    let kp2 = bob_client
        .get_key_package(alice_peer_id, Some(alice_addr.clone()))
        .await
        .expect("get_key_package second");
    assert_ne!(
        kp1, kp2,
        "consecutive calls must return different KeyPackages"
    );

    // ===== 5. Presence =====
    let (online, last_seen) = bob_client
        .get_presence(alice_peer_id, Some(alice_addr.clone()))
        .await
        .expect("get_presence");
    assert!(online, "Alice must be online");
    assert_eq!(last_seen, 1700000060, "last_seen must match");

    // ===== 6. InboxMessage — envelope arrives at Server.next_inbox() =====
    let envelope_bytes = b"sealed envelope from bob".to_vec();
    bob_client
        .send_inbox(
            alice_peer_id,
            Some(alice_addr.clone()),
            envelope_bytes.clone(),
        )
        .await
        .expect("send_inbox");

    let received = timeout(Duration::from_secs(5), alice_server.next_inbox())
        .await
        .expect("inbox timeout: Server did not deliver the envelope within 5 s")
        .expect("alice inbox channel closed unexpectedly");
    assert_eq!(
        received.envelope, envelope_bytes,
        "received envelope must match what Bob sent"
    );
}

/// Poll until the Network has at least one listen address, or panic after 5 s.
async fn wait_for_addr(net: &Arc<Mutex<Network>>) -> Multiaddr {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let addrs = net.lock().await.local_addrs().await;
        if let Some(a) = addrs.into_iter().next() {
            return a;
        }
        if tokio::time::Instant::now() > deadline {
            panic!("listener never bound a local address within 5 s");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
