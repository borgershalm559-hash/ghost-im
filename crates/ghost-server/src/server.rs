//! Server: spawns a tokio task that drains inbound requests from Network and dispatches them.

use crate::messages::{GhostRequest, GhostResponse, MIN_COMPAT_VERSION, PROTOCOL_VERSION};
use crate::Result;
use ghost_identity::IdentityKey;
use ghost_network::Network;
use ghost_protocol::delivery_public;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

/// Inbound envelope routed to the application layer (Plan 06 will consume this).
#[derive(Debug, Clone)]
pub struct InboundEnvelope {
    pub envelope: Vec<u8>,
}

/// Snapshot of presence state — set by the application layer.
#[derive(Debug, Clone, Copy)]
pub struct PresenceState {
    pub online: bool,
    pub last_seen: u64,
}

impl Default for PresenceState {
    fn default() -> Self {
        Self {
            online: false,
            last_seen: 0,
        }
    }
}

pub struct Server {
    _task: tokio::task::JoinHandle<()>,
    inbox_rx: mpsc::Receiver<InboundEnvelope>,
}

impl Server {
    /// Spawn a server attached to the given Network. The Network MUST be exclusive
    /// to this Server (the Server takes ownership of inbound request consumption
    /// via the locked Network).
    pub fn spawn(
        ik: Arc<IdentityKey>,
        network: Arc<Mutex<Network>>,
        presence: Arc<Mutex<PresenceState>>,
    ) -> Result<Self> {
        let (inbox_tx, inbox_rx) = mpsc::channel::<InboundEnvelope>(64);
        let task = tokio::spawn(run_server(ik, network, presence, inbox_tx));
        Ok(Self {
            _task: task,
            inbox_rx,
        })
    }

    /// Receive the next inbox envelope. Returns None if the server has shut down.
    pub async fn next_inbox(&mut self) -> Option<InboundEnvelope> {
        self.inbox_rx.recv().await
    }
}

async fn run_server(
    ik: Arc<IdentityKey>,
    network: Arc<Mutex<Network>>,
    presence: Arc<Mutex<PresenceState>>,
    inbox_tx: mpsc::Sender<InboundEnvelope>,
) {
    loop {
        // Acquire the network lock briefly to receive the next request, then release.
        let req = {
            let mut net = network.lock().await;
            net.next_request().await
        };
        let Some(req) = req else {
            break;
        };

        let response = handle_request(&ik, &presence, &inbox_tx, &req.payload).await;

        let bytes = match response.to_cbor() {
            Ok(b) => b,
            Err(_) => GhostResponse::Error {
                reason: "internal: response serialization failed".into(),
            }
            .to_cbor()
            .unwrap_or_default(),
        };

        let net = network.lock().await;
        let _ = net.respond(req.response, bytes).await;
    }
}

async fn handle_request(
    ik: &IdentityKey,
    presence: &Arc<Mutex<PresenceState>>,
    inbox_tx: &mpsc::Sender<InboundEnvelope>,
    payload: &[u8],
) -> GhostResponse {
    let request = match GhostRequest::from_cbor(payload) {
        Ok(r) => r,
        Err(e) => return GhostResponse::Error { reason: format!("decode: {e}") },
    };

    match request {
        GhostRequest::Version => GhostResponse::Version {
            protocol: PROTOCOL_VERSION.to_string(),
            min_compat: MIN_COMPAT_VERSION.to_string(),
        },
        GhostRequest::GetDeliveryKey => GhostResponse::DeliveryKey {
            x25519_pub: *delivery_public(ik).as_bytes(),
        },
        GhostRequest::GetKeyPackage => {
            // Plan 05 Task 4 fills this in (storage integration).
            GhostResponse::Error { reason: "key package handler not implemented yet".into() }
        }
        GhostRequest::GetPresence => {
            let p = *presence.lock().await;
            GhostResponse::Presence {
                online: p.online,
                last_seen: p.last_seen,
            }
        }
        GhostRequest::InboxMessage { envelope } => {
            let _ = inbox_tx.send(InboundEnvelope { envelope }).await;
            GhostResponse::InboxAck
        }
    }
}
