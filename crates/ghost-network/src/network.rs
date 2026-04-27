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
    core::transport::ListenerId, kad, request_response, swarm::SwarmEvent, Multiaddr, PeerId,
    Swarm, SwarmBuilder,
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
    SendBytes {
        target_peer: PeerId,
        target_addr: Option<Multiaddr>,
        bytes: Vec<u8>,
        reply: oneshot::Sender<Result<()>>,
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
// Public inbound event type
// ---------------------------------------------------------------------------

/// Events delivered from the network to the application layer.
#[derive(Debug)]
pub enum InboundEvent {
    /// A raw byte message arrived from a remote peer.
    Message { sender: PeerId, payload: Vec<u8> },
}

// ---------------------------------------------------------------------------
// Network handle
// ---------------------------------------------------------------------------

/// Handle to the running Ghost network stack.
///
/// Dropping this value stops the event loop.
pub struct Network {
    cmd_tx: mpsc::Sender<Command>,
    inbound_rx: mpsc::Receiver<InboundEvent>,
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
        let (inbound_tx, inbound_rx) = mpsc::channel::<InboundEvent>(64);

        let task = tokio::spawn(run_event_loop(swarm, cmd_rx, inbound_tx));

        Ok(Self {
            cmd_tx,
            inbound_rx,
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

    /// Send raw bytes to a remote peer.
    ///
    /// If `target_addr` is provided, it is added to Kademlia's address book and
    /// the swarm will dial the peer before sending. If the peer is already
    /// connected, the request is sent immediately.
    pub async fn send_to(
        &self,
        target_peer: PeerId,
        target_addr: Option<Multiaddr>,
        bytes: Vec<u8>,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(Command::SendBytes {
                target_peer,
                target_addr,
                bytes,
                reply: tx,
            })
            .await
            .map_err(|_| NetworkError::ChannelClosed)?;
        rx.await.map_err(|_| NetworkError::ChannelClosed)?
    }

    /// Receive the next inbound event, or `None` if the event loop has stopped.
    pub async fn next_inbound(&mut self) -> Option<InboundEvent> {
        self.inbound_rx.recv().await
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

/// One outbound send that is waiting for the dialled peer to connect.
type PendingSend = (Vec<u8>, oneshot::Sender<Result<()>>);

/// State tracked between iterations of the select! loop.
struct LoopState {
    local_addrs: Vec<Multiaddr>,
    /// Bytes queued for peers we are currently dialling.
    /// On `ConnectionEstablished` we drain the queue and send the messages.
    pending_sends: HashMap<PeerId, Vec<PendingSend>>,
    pending_puts: HashMap<kad::QueryId, oneshot::Sender<Result<()>>>,
    pending_gets: HashMap<kad::QueryId, oneshot::Sender<Result<Option<AddressRecord>>>>,
}

impl LoopState {
    fn new() -> Self {
        Self {
            local_addrs: Vec::new(),
            pending_sends: HashMap::new(),
            pending_puts: HashMap::new(),
            pending_gets: HashMap::new(),
        }
    }
}

async fn run_event_loop(
    mut swarm: Swarm<GhostBehaviour>,
    mut cmd_rx: mpsc::Receiver<Command>,
    inbound_tx: mpsc::Sender<InboundEvent>,
) {
    let mut state = LoopState::new();

    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => {
                let Some(cmd) = cmd else { break };
                handle_command(cmd, &mut swarm, &mut state);
            }
            event = swarm.select_next_some() => {
                handle_swarm_event(event, &mut swarm, &mut state, &inbound_tx).await;
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

        Command::SendBytes {
            target_peer,
            target_addr,
            bytes,
            reply,
        } => {
            if let Some(addr) = target_addr {
                // Register the address in Kademlia and dial the peer.
                // The actual send is deferred until ConnectionEstablished.
                swarm.behaviour_mut().kad.add_address(&target_peer, addr);
                state
                    .pending_sends
                    .entry(target_peer)
                    .or_default()
                    .push((bytes, reply));
                if let Err(e) = swarm.dial(target_peer) {
                    // Drain the pending queue and report the error.
                    if let Some(queue) = state.pending_sends.remove(&target_peer) {
                        for (_, r) in queue {
                            let _ = r.send(Err(NetworkError::DialFailed {
                                peer: target_peer.to_string(),
                                detail: e.to_string(),
                            }));
                        }
                    }
                }
            } else {
                // Peer should already be connected; fire and forget.
                swarm
                    .behaviour_mut()
                    .messages
                    .send_request(&target_peer, bytes);
                let _ = reply.send(Ok(()));
            }
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
    inbound_tx: &mpsc::Sender<InboundEvent>,
) {
    match event {
        SwarmEvent::NewListenAddr { address, .. } => {
            state.local_addrs.push(address);
        }

        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
            // Flush any messages queued while dialling.
            if let Some(queue) = state.pending_sends.remove(&peer_id) {
                for (bytes, reply) in queue {
                    swarm.behaviour_mut().messages.send_request(&peer_id, bytes);
                    let _ = reply.send(Ok(()));
                }
            }
        }

        // ----------------------------------------------------------------
        // request-response: incoming message from a remote peer
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
            // Send an empty ACK response so the remote doesn't time out.
            let _ = swarm
                .behaviour_mut()
                .messages
                .send_response(channel, Vec::new());
            let _ = inbound_tx
                .send(InboundEvent::Message {
                    sender: peer,
                    payload: request,
                })
                .await;
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
