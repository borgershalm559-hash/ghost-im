//! Server: spawns a tokio task that drains inbound requests from Network and dispatches them.

use crate::messages::{GhostRequest, GhostResponse, MIN_COMPAT_VERSION, PROTOCOL_VERSION};
use crate::Result;
use ghost_identity::IdentityKey;
use ghost_network::Network;
use ghost_protocol::delivery_public;
use ghost_storage::Database;
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
        db: Arc<Database>,
    ) -> Result<Self> {
        let (inbox_tx, inbox_rx) = mpsc::channel::<InboundEnvelope>(64);
        let task = tokio::spawn(run_server(ik, network, presence, db, inbox_tx));
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
    db: Arc<Database>,
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

        let response = handle_request(&ik, &presence, &db, &inbox_tx, &req.payload).await;

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
    db: &Arc<Database>,
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
        GhostRequest::GetKeyPackage => handle_get_key_package(db).await,
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

async fn handle_get_key_package(db: &Arc<Database>) -> GhostResponse {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let db_clone = db.clone();
    let result = tokio::task::spawn_blocking(move || -> std::result::Result<Vec<u8>, crate::ServerError> {
        let repo = db_clone.my_keypackages();
        let available = repo.list_available_one_time().map_err(crate::ServerError::from)?;
        let candidate = match available.first() {
            Some(c) => c.clone(),
            None => match repo.last_resort().map_err(crate::ServerError::from)? {
                Some(lr) => lr,
                None => return Err(crate::ServerError::NoKeyPackagesAvailable),
            },
        };
        if !candidate.is_last_resort {
            repo.mark_consumed(&candidate.package_id, now).map_err(crate::ServerError::from)?;
        }
        Ok(candidate.package_blob.clone())
    })
    .await;

    match result {
        Ok(Ok(bytes)) => GhostResponse::KeyPackage { bytes },
        Ok(Err(e)) => GhostResponse::Error { reason: format!("{e}") },
        Err(e) => GhostResponse::Error { reason: format!("task join: {e}") },
    }
}
