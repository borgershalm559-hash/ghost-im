//! High-level Network handle: spawns a libp2p Swarm event loop in a tokio task.
//!
//! `Network` is the public API surface. Internally it communicates with the
//! event loop via mpsc channels, keeping the `Swarm` fully contained within
//! the spawned task.

use crate::address_record::AddressRecord;
use crate::behaviour::{GhostBehaviour, GhostBehaviourEvent};
use crate::identity::libp2p_keypair_from_ik;
use crate::{NetworkError, Result};
use futures::StreamExt;
use ghost_core::GhostId;
use ghost_identity::IdentityKey;
use libp2p::{
    core::{transport::ListenerId, ConnectedPoint},
    kad, request_response,
    swarm::SwarmEvent,
    Multiaddr, PeerId, Swarm, SwarmBuilder,
};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

// ---------------------------------------------------------------------------
// Internal command channel
// ---------------------------------------------------------------------------

/// Commands the event loop accepts from `Network` method calls.
pub(crate) enum Command {
    Listen {
        addr: Multiaddr,
        reply: oneshot::Sender<Result<ListenerId>>,
    },
    Request {
        target_peer: PeerId,
        target_addr: Option<Multiaddr>,
        bytes: Vec<u8>,
        reply: oneshot::Sender<Result<Vec<u8>>>,
    },
    Respond {
        channel: request_response::ResponseChannel<Vec<u8>>,
        bytes: Vec<u8>,
    },
    PutAddressRecord {
        record: AddressRecord,
        reply: oneshot::Sender<Result<()>>,
    },
    GetAddressRecord {
        ghost_id: GhostId,
        reply: oneshot::Sender<Result<Option<AddressRecord>>>,
    },
    LocalAddrs {
        reply: oneshot::Sender<Vec<Multiaddr>>,
    },
}

// ---------------------------------------------------------------------------
// Public inbound request/response types
// ---------------------------------------------------------------------------

/// Opaque handle used to send a response back to the requesting peer.
pub struct ResponseHandle {
    pub(crate) inner: request_response::ResponseChannel<Vec<u8>>,
}

/// An inbound request awaiting a response from the local node.
pub struct InboundRequest {
    pub sender: PeerId,
    pub payload: Vec<u8>,
    pub response: ResponseHandle,
}

impl std::fmt::Debug for InboundRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InboundRequest")
            .field("sender", &self.sender)
            .field("payload_len", &self.payload.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Network handle
// ---------------------------------------------------------------------------

/// Handle to the running Ghost network stack.
///
/// Dropping this value stops the event loop.
pub struct Network {
    cmd_tx: mpsc::Sender<Command>,
    request_rx: mpsc::Receiver<InboundRequest>,
    local_peer_id: PeerId,
    // Keeps the spawned task alive for as long as the Network is alive.
    _task: tokio::task::JoinHandle<()>,
}

impl Network {
    /// Spawn a new network stack for the given identity key.
    ///
    /// Uses QUIC v1 as the sole transport; TLS authentication is built into
    /// QUIC so no separate security upgrade is needed.
    pub async fn spawn(ik: &IdentityKey) -> Result<Self> {
        let kp = libp2p_keypair_from_ik(ik)?;
        let local_peer_id = PeerId::from(kp.public());

        // Clone so the keypair can be moved into SwarmBuilder by value while
        // we retain a copy for GhostBehaviour::new.
        let kp_clone = kp.clone();
        let swarm = SwarmBuilder::with_existing_identity(kp)
            .with_tokio()
            .with_quic()
            .with_behaviour(|_kp_inner| {
                // GhostBehaviour::new needs the peer id and the keypair.
                // We captured kp_clone from the outer scope since _kp_inner
                // is a shared reference (&Keypair) to the builder's copy.
                GhostBehaviour::new(local_peer_id, &kp_clone)
            })
            .map_err(|e| NetworkError::Transport(format!("build behaviour: {e}")))?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        let (cmd_tx, cmd_rx) = mpsc::channel::<Command>(64);
        let (request_tx, request_rx) = mpsc::channel::<InboundRequest>(64);

        let task = tokio::spawn(run_event_loop(swarm, cmd_rx, request_tx));

        Ok(Self {
            cmd_tx,
            request_rx,
            local_peer_id,
            _task: task,
        })
    }

    /// The local peer identity (Ed25519 public key encoded as a `PeerId`).
    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    /// Start listening on the given multiaddress.
    pub async fn listen_on(&self, addr: Multiaddr) -> Result<ListenerId> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(Command::Listen { addr, reply: tx })
            .await
            .map_err(|_| NetworkError::ChannelClosed)?;
        rx.await.map_err(|_| NetworkError::ChannelClosed)?
    }

    /// Send a request to a remote peer and await the response bytes.
    ///
    /// If `target_addr` is provided it is added to Kademlia's address book so
    /// the swarm can dial the peer. libp2p's request-response layer manages
    /// the dial and queues the request internally until the connection is up.
    pub async fn request(
        &self,
        target_peer: PeerId,
        target_addr: Option<Multiaddr>,
        bytes: Vec<u8>,
    ) -> Result<Vec<u8>> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(Command::Request {
                target_peer,
                target_addr,
                bytes,
                reply: tx,
            })
            .await
            .map_err(|_| NetworkError::ChannelClosed)?;
        rx.await.map_err(|_| NetworkError::ChannelClosed)?
    }

    /// Receive the next inbound request, or `None` if the event loop has stopped.
    pub async fn next_request(&mut self) -> Option<InboundRequest> {
        self.request_rx.recv().await
    }

    /// Send a response to an inbound request.
    pub async fn respond(&self, handle: ResponseHandle, bytes: Vec<u8>) -> Result<()> {
        self.cmd_tx
            .send(Command::Respond {
                channel: handle.inner,
                bytes,
            })
            .await
            .map_err(|_| NetworkError::ChannelClosed)
    }

    /// Fire-and-forget send. Sends a request to the peer and discards the response.
    ///
    /// If `target_addr` is provided it is registered so the swarm can dial the peer.
    pub async fn send_to(
        &self,
        target_peer: PeerId,
        target_addr: Option<Multiaddr>,
        bytes: Vec<u8>,
    ) -> Result<()> {
        let _ = self.request(target_peer, target_addr, bytes).await?;
        Ok(())
    }

    /// Publish an `AddressRecord` to the Kademlia DHT.
    pub async fn put_address_record(&self, record: AddressRecord) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(Command::PutAddressRecord { record, reply: tx })
            .await
            .map_err(|_| NetworkError::ChannelClosed)?;
        rx.await.map_err(|_| NetworkError::ChannelClosed)?
    }

    /// Look up an `AddressRecord` for the given Ghost identity from the DHT.
    pub async fn get_address_record(&self, ghost_id: GhostId) -> Result<Option<AddressRecord>> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(Command::GetAddressRecord {
                ghost_id,
                reply: tx,
            })
            .await
            .map_err(|_| NetworkError::ChannelClosed)?;
        rx.await.map_err(|_| NetworkError::ChannelClosed)?
    }

    /// Return the multiaddresses the local node is currently listening on.
    pub async fn local_addrs(&self) -> Vec<Multiaddr> {
        let (tx, rx) = oneshot::channel();
        if self
            .cmd_tx
            .send(Command::LocalAddrs { reply: tx })
            .await
            .is_err()
        {
            return Vec::new();
        }
        rx.await.unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Event loop (runs inside a tokio task)
// ---------------------------------------------------------------------------

/// A queued outbound request waiting for the connection to be established.
type PendingRequest = (Vec<u8>, oneshot::Sender<Result<Vec<u8>>>);

/// State tracked between iterations of the select! loop.
struct LoopState {
    local_addrs: Vec<Multiaddr>,
    /// Requests queued for peers we are currently dialling.
    /// Drained into `messages.send_request` on `ConnectionEstablished`.
    pending_dials: HashMap<PeerId, Vec<PendingRequest>>,
    /// Active outbound requests awaiting a response from the remote peer.
    pending_requests:
        HashMap<request_response::OutboundRequestId, oneshot::Sender<Result<Vec<u8>>>>,
    pending_puts: HashMap<kad::QueryId, oneshot::Sender<Result<()>>>,
    pending_gets: HashMap<kad::QueryId, oneshot::Sender<Result<Option<AddressRecord>>>>,
}

impl LoopState {
    fn new() -> Self {
        Self {
            local_addrs: Vec::new(),
            pending_dials: HashMap::new(),
            pending_requests: HashMap::new(),
            pending_puts: HashMap::new(),
            pending_gets: HashMap::new(),
        }
    }
}

async fn run_event_loop(
    mut swarm: Swarm<GhostBehaviour>,
    mut cmd_rx: mpsc::Receiver<Command>,
    request_tx: mpsc::Sender<InboundRequest>,
) {
    let mut state = LoopState::new();

    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => {
                let Some(cmd) = cmd else { break };
                handle_command(cmd, &mut swarm, &mut state);
            }
            event = swarm.select_next_some() => {
                handle_swarm_event(event, &mut swarm, &mut state, &request_tx).await;
            }
        }
    }
}

fn handle_command(cmd: Command, swarm: &mut Swarm<GhostBehaviour>, state: &mut LoopState) {
    match cmd {
        Command::Listen { addr, reply } => {
            let res = swarm
                .listen_on(addr)
                .map_err(|e| NetworkError::Transport(format!("listen_on: {e}")));
            let _ = reply.send(res);
        }

        Command::Request {
            target_peer,
            target_addr,
            bytes,
            reply,
        } => {
            if let Some(addr) = target_addr {
                // Register the address in Kademlia so `handle_pending_outbound_connection`
                // can supply it when request-response dials the peer.
                swarm.behaviour_mut().kad.add_address(&target_peer, addr);
            }
            // `send_request` manages the dial internally using addresses from
            // all behaviour's `handle_pending_outbound_connection`. Failures
            // come back as `Event::OutboundFailure` and are handled below.
            let request_id = swarm
                .behaviour_mut()
                .messages
                .send_request(&target_peer, bytes);
            state.pending_requests.insert(request_id, reply);
        }

        Command::Respond { channel, bytes } => {
            let _ = swarm.behaviour_mut().messages.send_response(channel, bytes);
        }

        Command::PutAddressRecord { record, reply } => {
            let key_bytes = AddressRecord::dht_key(&record.ghost_id);
            match record.to_cbor() {
                Err(e) => {
                    let _ = reply.send(Err(e));
                }
                Ok(value) => {
                    let kad_record = kad::Record {
                        key: kad::RecordKey::new(&key_bytes),
                        value,
                        publisher: None,
                        expires: None,
                    };
                    match swarm
                        .behaviour_mut()
                        .kad
                        .put_record(kad_record, kad::Quorum::One)
                    {
                        Ok(qid) => {
                            state.pending_puts.insert(qid, reply);
                        }
                        Err(e) => {
                            let _ =
                                reply.send(Err(NetworkError::DhtQuery(format!("put_record: {e}"))));
                        }
                    }
                }
            }
        }

        Command::GetAddressRecord { ghost_id, reply } => {
            let key_bytes = AddressRecord::dht_key(&ghost_id);
            let qid = swarm
                .behaviour_mut()
                .kad
                .get_record(kad::RecordKey::new(&key_bytes));
            state.pending_gets.insert(qid, reply);
        }

        Command::LocalAddrs { reply } => {
            let _ = reply.send(state.local_addrs.clone());
        }
    }
}

async fn handle_swarm_event(
    event: SwarmEvent<GhostBehaviourEvent>,
    swarm: &mut Swarm<GhostBehaviour>,
    state: &mut LoopState,
    request_tx: &mpsc::Sender<InboundRequest>,
) {
    match event {
        SwarmEvent::NewListenAddr { address, .. } => {
            state.local_addrs.push(address);
        }

        SwarmEvent::ConnectionEstablished {
            peer_id, endpoint, ..
        } => {
            // Flush queued requests only when OUR dial succeeded (we are the
            // dialer). Inbound connections (from the remote dialling us) do not
            // correspond to our `pending_dials` queue.
            if matches!(endpoint, ConnectedPoint::Dialer { .. }) {
                if let Some(queue) = state.pending_dials.remove(&peer_id) {
                    for (bytes, reply) in queue {
                        let request_id =
                            swarm.behaviour_mut().messages.send_request(&peer_id, bytes);
                        state.pending_requests.insert(request_id, reply);
                    }
                }
            }
        }

        // ----------------------------------------------------------------
        // request-response: inbound request from a remote peer
        // ----------------------------------------------------------------
        SwarmEvent::Behaviour(GhostBehaviourEvent::Messages(
            request_response::Event::Message {
                peer,
                message:
                    request_response::Message::Request {
                        request, channel, ..
                    },
                ..
            },
        )) => {
            let inbound = InboundRequest {
                sender: peer,
                payload: request,
                response: ResponseHandle { inner: channel },
            };
            let _ = request_tx.send(inbound).await;
        }

        // ----------------------------------------------------------------
        // request-response: response from a remote peer
        // ----------------------------------------------------------------
        SwarmEvent::Behaviour(GhostBehaviourEvent::Messages(
            request_response::Event::Message {
                message:
                    request_response::Message::Response {
                        request_id,
                        response,
                    },
                ..
            },
        )) => {
            if let Some(reply) = state.pending_requests.remove(&request_id) {
                let _ = reply.send(Ok(response));
            }
        }

        // ----------------------------------------------------------------
        // request-response: outbound failure
        // ----------------------------------------------------------------
        SwarmEvent::Behaviour(GhostBehaviourEvent::Messages(
            request_response::Event::OutboundFailure {
                request_id, error, ..
            },
        )) => {
            if let Some(reply) = state.pending_requests.remove(&request_id) {
                let _ = reply.send(Err(NetworkError::Transport(format!(
                    "outbound failure: {error}"
                ))));
            }
        }

        // ----------------------------------------------------------------
        // Kademlia: outbound query results
        // ----------------------------------------------------------------
        SwarmEvent::Behaviour(GhostBehaviourEvent::Kad(kad::Event::OutboundQueryProgressed {
            id,
            result,
            ..
        })) => {
            match result {
                kad::QueryResult::PutRecord(res) => {
                    if let Some(reply) = state.pending_puts.remove(&id) {
                        let mapped = res
                            .map(|_| ())
                            .map_err(|e| NetworkError::DhtQuery(format!("put_record: {e:?}")));
                        let _ = reply.send(mapped);
                    }
                }

                kad::QueryResult::GetRecord(res) => {
                    if let Some(reply) = state.pending_gets.remove(&id) {
                        let mapped = match res {
                            Ok(kad::GetRecordOk::FoundRecord(peer_record)) => {
                                AddressRecord::from_cbor(&peer_record.record.value).map(Some)
                            }
                            Ok(kad::GetRecordOk::FinishedWithNoAdditionalRecord { .. }) => Ok(None),
                            Err(e) => Err(NetworkError::DhtQuery(format!("get_record: {e:?}"))),
                        };
                        let _ = reply.send(mapped);
                    }
                }

                // Other query results (Bootstrap, GetClosestPeers, …) are
                // informational; ignore them for now.
                _ => {}
            }
        }

        // All other swarm events (dialling, connection closed, …) are
        // ignored at this level — add arms here as features are added.
        _ => {}
    }
}
