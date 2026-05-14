//! Client orchestration: opens Identity + Database + Network + Server, runs background tasks.

use crate::error::ClientError;
use crate::invite::GhostInvite;
use crate::Result;
use ghost_core::Fingerprint;
use ghost_identity::{Identity, IdentityKey};
use ghost_network::{peer_id_from_ghost_id, Network};
use ghost_protocol::{
    generate_key_package, new_provider, unwrap_message_lenient, wrap_message, GhostMlsProvider,
    MlsSession, MsgType, PayloadType, UnwrappedMessage,
};
use ghost_server::Client as ServerClient;
use ghost_server::{PresenceState, Server};
use ghost_storage::{
    derive_master_key, Contact, Database, Direction, MessageRow, MessageStatus, MlsGroupRow,
    MyKeyPackageRow, Verification,
};
use libp2p::{Multiaddr, PeerId};
use openmls::prelude::tls_codec::{Deserialize as TlsDeserialize, Serialize as TlsSerialize};
use openmls::prelude::{KeyPackageIn, MlsMessageIn};
use openmls_traits::OpenMlsProvider;
use rand::RngCore;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use uuid::Uuid;

const KEYPACKAGE_REFILL_THRESHOLD: usize = 3;
const KEYPACKAGE_BATCH: u32 = 5;

pub struct ClientConfig {
    pub listen_addr: Multiaddr,
    pub passphrase: Option<String>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            // Listen on ALL interfaces (0.0.0.0) so peers on the same LAN
            // can dial us, and so our external (NAT-mapped) address becomes
            // routable. Listening on 127.0.0.1 only makes the node reachable
            // from the same machine — which is useless for a P2P messenger.
            listen_addr: "/ip4/0.0.0.0/udp/0/quic-v1"
                .parse()
                .expect("valid multiaddr"),
            passphrase: None,
        }
    }
}

pub struct Client {
    pub(crate) identity: Identity,
    pub(crate) ik: Arc<IdentityKey>,
    pub(crate) db: Arc<Database>,
    pub(crate) network: Arc<Mutex<Network>>,
    pub(crate) server: Mutex<Option<Server>>,
    #[allow(dead_code)]
    pub(crate) presence: Arc<Mutex<PresenceState>>,
    pub(crate) local_peer_id: PeerId,
    pub(crate) local_addrs: Vec<Multiaddr>,
    pub(crate) mls_provider: Arc<Mutex<GhostMlsProvider>>,
}

impl Client {
    /// Open the Client. Loads the Identity from the standard path, opens the
    /// encrypted Database, spawns Network + Server, listens, returns once at
    /// least one address has bound.
    pub async fn open(config: ClientConfig) -> Result<Self> {
        let identity = Identity::load_default(config.passphrase.as_deref())?;
        let ik = Arc::new(IdentityKey::from_secret_bytes(
            identity.identity_key.secret_bytes(),
        ));

        let db_path =
            ghost_identity::database_file().map_err(ghost_identity::IdentityError::from)?;
        let master_key = derive_master_key(&ik);
        let db = Database::open_encrypted(&db_path, &master_key)?;
        db.migrate()?;
        let db = Arc::new(db);

        let network = Network::spawn(&ik).await?;
        let local_peer_id = network.local_peer_id();
        let network = Arc::new(Mutex::new(network));
        network.lock().await.listen_on(config.listen_addr).await?;

        let local_addrs = wait_for_local_addrs(&network).await;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let presence = Arc::new(Mutex::new(PresenceState {
            online: true,
            last_seen: now,
        }));

        // Split the inbox receiver out of the network before spawning the server
        // so the server loop can drain requests without holding the network mutex.
        let inbox = network.lock().await.split_inbox();
        let server = Server::spawn(
            ik.clone(),
            inbox,
            network.clone(),
            presence.clone(),
            db.clone(),
        )?;

        let mls_provider = Arc::new(Mutex::new(new_provider()));

        let client = Self {
            identity,
            ik,
            db,
            network,
            server: Mutex::new(Some(server)),
            presence,
            local_peer_id,
            local_addrs,
            mls_provider,
        };

        spawn_retention_scrubber(client.db.clone());
        spawn_address_publisher(client.network.clone(), client.ik.clone());
        client.ensure_keypackages().await?;
        Ok(client)
    }

    /// Test-only / programmatic open. Skips the file-based identity-load + OS-keystore
    /// path. Database is opened at the given path with master-key derived from the
    /// supplied Identity.
    pub async fn open_with_in_memory_identity(
        identity: Identity,
        db_path: std::path::PathBuf,
        listen_addr: Multiaddr,
    ) -> Result<Self> {
        let ik = Arc::new(IdentityKey::from_secret_bytes(
            identity.identity_key.secret_bytes(),
        ));
        let master_key = derive_master_key(&ik);
        let db = Database::open_encrypted(&db_path, &master_key)?;
        db.migrate()?;
        let db = Arc::new(db);

        let network = Network::spawn(&ik).await?;
        let local_peer_id = network.local_peer_id();
        let network = Arc::new(Mutex::new(network));
        network.lock().await.listen_on(listen_addr).await?;
        let local_addrs = wait_for_local_addrs(&network).await;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let presence = Arc::new(Mutex::new(PresenceState {
            online: true,
            last_seen: now,
        }));

        // Split the inbox receiver out of the network before spawning the server
        // so the server loop can drain requests without holding the network mutex.
        let inbox = network.lock().await.split_inbox();
        let server = Server::spawn(
            ik.clone(),
            inbox,
            network.clone(),
            presence.clone(),
            db.clone(),
        )?;
        let mls_provider = Arc::new(Mutex::new(new_provider()));

        let client = Self {
            identity,
            ik,
            db,
            network,
            server: Mutex::new(Some(server)),
            presence,
            local_peer_id,
            local_addrs,
            mls_provider,
        };

        spawn_retention_scrubber(client.db.clone());
        client.ensure_keypackages().await?;
        Ok(client)
    }

    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    pub fn local_addrs(&self) -> &[Multiaddr] {
        &self.local_addrs
    }

    /// Query the running Network for the *current* listen addresses. Picks up
    /// addresses libp2p added after Client::open returned (e.g. NAT-mapped
    /// external addresses from identify/AutoNAT). Filters loopback.
    pub async fn live_local_addrs(&self) -> Vec<Multiaddr> {
        let mut addrs = self.network.lock().await.local_addrs().await;
        addrs.retain(|a| !is_loopback_multiaddr(a));
        addrs
    }

    pub fn ghost_id(&self) -> ghost_core::GhostId {
        self.ik.ghost_id()
    }

    /// Build a fresh invite advertising our current addresses.
    pub fn create_invite(&self, ttl_seconds: u64) -> Result<GhostInvite> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut token = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut token);
        let addresses = self
            .local_addrs
            .iter()
            .map(|a| a.to_string())
            .collect::<Vec<_>>();
        Ok(GhostInvite::new(
            &self.ik,
            addresses,
            token,
            now,
            ttl_seconds,
        ))
    }

    /// Ensure at least `KEYPACKAGE_REFILL_THRESHOLD` available KeyPackages exist
    /// in `my_keypackages`. If fewer, generate a batch and insert them.
    ///
    /// Uses the long-lived `mls_provider` so that the init private keys registered
    /// during KeyPackage generation remain alive when `handle_mls_handshake` later
    /// calls `MlsSession::join_via_welcome` against the same provider.
    pub async fn ensure_keypackages(&self) -> Result<()> {
        let provider = self.mls_provider.lock().await;
        let available = self.db.my_keypackages().list_available_one_time()?;
        if available.len() >= KEYPACKAGE_REFILL_THRESHOLD {
            return Ok(());
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        for _ in 0..KEYPACKAGE_BATCH {
            let kp = generate_key_package(
                &provider,
                &self.identity.identity_key,
                &self.identity.device_key,
            )
            .map_err(crate::error::ClientError::Protocol)?;
            let kp_bytes = kp
                .tls_serialize_detached()
                .map_err(|e| crate::error::ClientError::Internal(format!("kp serialize: {e}")))?;
            let pkg_id = *blake3::hash(&kp_bytes).as_bytes();
            self.db.my_keypackages().insert(&MyKeyPackageRow {
                package_id: pkg_id,
                package_blob: kp_bytes,
                private_key: vec![],
                created_at: now as i64,
                consumed_at: None,
                is_last_resort: false,
            })?;
        }
        Ok(())
    }

    /// Add a contact via an invite. Connects to the peer, fetches a KeyPackage,
    /// creates an MLS group, sends a Welcome envelope, and persists state.
    pub async fn add_contact(&self, invite_str: &str) -> Result<()> {
        let invite = GhostInvite::from_bech32(invite_str)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        invite.verify(now)?;

        // Reject adding self — common UX confusion in MVP-1 where a user
        // pastes their own invite into the Add-contact field.
        if invite.ghost_id == self.identity.identity_key.ghost_id() {
            return Err(crate::error::ClientError::Invalid(
                "cannot add yourself as a contact".into(),
            ));
        }

        let peer_id = peer_id_from_ghost_id(&invite.ghost_id)?;
        let invite_addr: Option<Multiaddr> = invite
            .addresses
            .first()
            .and_then(|a| a.parse::<Multiaddr>().ok());

        // Try direct dial via the invite-embedded address first. If that fails
        // (peer moved networks, NAT changed, address stale), fall back to the
        // DHT — the peer may have re-published a fresh `AddressRecord`.
        let server_client = ServerClient::new(self.network.clone());
        let address = self
            .resolve_peer_address(&invite.ghost_id, peer_id, invite_addr)
            .await?;
        let kp_bytes = server_client
            .get_key_package(peer_id, Some(address.clone()))
            .await
            .map_err(|e| map_dial_error(e.into()))?;

        // Validate KeyPackage and create MLS group + Welcome.
        let provider = new_provider();
        let kp_in = KeyPackageIn::tls_deserialize(&mut kp_bytes.as_slice())
            .map_err(|e| crate::error::ClientError::Internal(format!("kp deserialize: {e}")))?;
        let kp = kp_in
            .validate(provider.crypto(), openmls::versions::ProtocolVersion::Mls10)
            .map_err(|e| crate::error::ClientError::Internal(format!("kp validate: {e}")))?;

        let mut session = MlsSession::create(
            &provider,
            &self.identity.identity_key,
            &self.identity.device_key,
        )
        .map_err(crate::error::ClientError::Protocol)?;
        let signer = MlsSession::signer_from_dk(&self.identity.device_key);
        let invite_result = session
            .add_member(&provider, &signer, kp)
            .map_err(crate::error::ClientError::Protocol)?;

        let welcome_bytes = invite_result
            .welcome
            .tls_serialize_detached()
            .map_err(|e| crate::error::ClientError::Internal(format!("welcome serialize: {e}")))?;

        let peer_delivery_pub_bytes = server_client
            .get_delivery_key(peer_id, Some(address.clone()))
            .await?;
        let peer_delivery_pub = x25519_dalek::PublicKey::from(peer_delivery_pub_bytes);

        let envelope_bytes = wrap_message(
            &self.identity.identity_key,
            &self.identity.device_key,
            invite.ghost_id,
            &peer_delivery_pub,
            MsgType::MlsHandshake,
            PayloadType::MlsHandshake,
            welcome_bytes,
            now,
        )
        .map_err(crate::error::ClientError::Protocol)?;

        server_client
            .send_inbox(peer_id, Some(address.clone()), envelope_bytes)
            .await?;

        // Persist contact + MLS state.
        let fingerprint = Fingerprint::of(&invite.ghost_id).to_string();
        let contact = Contact {
            ghost_id: invite.ghost_id,
            display_name: None,
            local_alias: None,
            fingerprint,
            added_at: now as i64,
            last_endpoint: Some(address.to_string()),
            verification: Verification::Unverified,
            notes: None,
            blocked: false,
            dk_pub: None,
            last_read_at: 0,
            pinned: false,
            muted: false,
            retention_seconds: None,
        };
        let existing = self.db.contacts().get(&invite.ghost_id)?;
        if existing.is_some() {
            self.db.contacts().update(&contact)?;
        } else {
            self.db.contacts().insert(&contact)?;
        }

        let state_blob = session
            .serialize_state(&provider)
            .map_err(crate::error::ClientError::Protocol)?;
        let group_id = session.group_id_bytes();
        self.db.mls_groups().upsert(&MlsGroupRow {
            group_id: bytes_to_array_32(&group_id),
            contact_id: invite.ghost_id,
            state_blob,
            current_epoch: session.current_epoch(),
            created_at: now as i64,
            last_updated: now as i64,
        })?;

        Ok(())
    }

    /// List all contacts in the database.
    pub fn list_contacts(&self) -> Result<Vec<Contact>> {
        Ok(self.db.contacts().list()?)
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self.db.settings().get(key)?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        Ok(self.db.settings().set(key, value)?)
    }

    pub fn set_pinned(&self, id: &ghost_core::GhostId, pinned: bool) -> Result<()> {
        Ok(self.db.contacts().set_pinned(id, pinned)?)
    }

    pub fn set_muted(&self, id: &ghost_core::GhostId, muted: bool) -> Result<()> {
        Ok(self.db.contacts().set_muted(id, muted)?)
    }

    pub fn set_verified(&self, id: &ghost_core::GhostId, verified: bool) -> Result<()> {
        Ok(self.db.contacts().set_verified(id, verified)?)
    }

    pub fn set_retention(&self, id: &ghost_core::GhostId, seconds: Option<i64>) -> Result<()> {
        Ok(self.db.contacts().set_retention(id, seconds)?)
    }

    pub fn mark_chat_read(&self, id: &ghost_core::GhostId) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Ok(self.db.contacts().set_last_read_at(id, now)?)
    }

    pub fn last_message_for_contact(
        &self,
        contact_id: &ghost_core::GhostId,
    ) -> Result<Option<MessageRow>> {
        let mut all = self.db.messages().list_for_contact(contact_id, 100_000, 0)?;
        Ok(all.pop())
    }

    pub fn unread_count(&self, contact_id: &ghost_core::GhostId) -> Result<i64> {
        let last_read_at = self
            .db
            .contacts()
            .get(contact_id)?
            .map(|c| c.last_read_at)
            .unwrap_or(0);
        Ok(self.db.messages().unread_count(contact_id, last_read_at)?)
    }

    // Folder CRUD ───────────────────────────────────────────────────────────

    pub fn list_folders(&self) -> Result<Vec<ghost_storage::Folder>> {
        Ok(self.db.folders().list()?)
    }

    pub fn create_folder(&self, name: &str, icon: Option<&str>, now: i64) -> Result<i64> {
        Ok(self.db.folders().create(name, icon, 0, now)?)
    }

    pub fn rename_folder(&self, id: i64, new_name: &str) -> Result<()> {
        Ok(self.db.folders().rename(id, new_name)?)
    }

    pub fn delete_folder(&self, id: i64) -> Result<bool> {
        Ok(self.db.folders().delete(id)?)
    }

    pub fn add_contact_to_folder(
        &self,
        folder_id: i64,
        contact_id: &ghost_core::GhostId,
    ) -> Result<()> {
        Ok(self.db.folders().add_contact(folder_id, contact_id)?)
    }

    pub fn remove_contact_from_folder(
        &self,
        folder_id: i64,
        contact_id: &ghost_core::GhostId,
    ) -> Result<()> {
        Ok(self.db.folders().remove_contact(folder_id, contact_id)?)
    }

    pub fn contacts_for_folder(
        &self,
        folder_id: i64,
    ) -> Result<Vec<ghost_core::GhostId>> {
        Ok(self.db.folders().contacts_in_folder(folder_id)?)
    }

    /// Encrypt and send a text message to a contact.
    pub async fn send_message(&self, contact_id: ghost_core::GhostId, text: &str) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Load MLS state.
        let mls_row = self
            .db
            .mls_groups()
            .load_for_contact(&contact_id)?
            .ok_or_else(|| crate::error::ClientError::MlsGroupNotFound(format!("{contact_id}")))?;

        let (provider, mut session) = MlsSession::deserialize_state(&mls_row.state_blob)
            .map_err(crate::error::ClientError::Protocol)?;

        // Re-store the signer in the restored provider's storage.
        let signer = MlsSession::signer_from_dk(&self.identity.device_key);
        signer
            .store(provider.storage())
            .map_err(|e| crate::error::ClientError::Internal(format!("store signer: {e}")))?;

        // MLS-encrypt.
        let mls_ct = session
            .encrypt_app_message(&provider, &signer, text.as_bytes())
            .map_err(crate::error::ClientError::Protocol)?;

        // Find peer's address (optional — may be None for contacts added via Welcome,
        // where we know the peer ID but not the listening address; libp2p will use
        // the active connection if one exists).
        let contact =
            self.db.contacts().get(&contact_id)?.ok_or_else(|| {
                crate::error::ClientError::ContactNotFound(format!("{contact_id}"))
            })?;
        let address: Option<Multiaddr> = contact
            .last_endpoint
            .as_ref()
            .and_then(|ep| ep.parse::<Multiaddr>().ok());
        let peer_id = peer_id_from_ghost_id(&contact_id)?;

        // Fetch peer's delivery key (use address hint if available; otherwise rely
        // on an existing libp2p connection established during Welcome exchange).
        let server_client = ServerClient::new(self.network.clone());
        let peer_delivery_bytes = server_client
            .get_delivery_key(peer_id, address.clone())
            .await?;
        let peer_delivery_pub = x25519_dalek::PublicKey::from(peer_delivery_bytes);

        // Wrap envelope.
        let envelope_bytes = wrap_message(
            &self.identity.identity_key,
            &self.identity.device_key,
            contact_id,
            &peer_delivery_pub,
            MsgType::AppMessage,
            PayloadType::AppText,
            mls_ct,
            now,
        )
        .map_err(crate::error::ClientError::Protocol)?;

        // Send.
        server_client
            .send_inbox(peer_id, address, envelope_bytes)
            .await?;

        // Persist outgoing message. Stamp expires_at from contact retention.
        let msg_uuid = *Uuid::now_v7().as_bytes();
        self.db.messages().insert(&MessageRow {
            msg_uuid,
            contact_id,
            direction: Direction::Outgoing,
            content_type: 0,
            content: text.to_string(),
            sent_at: now as i64,
            received_at: None,
            status: MessageStatus::Sent,
            reply_to: None,
            expires_at: contact.retention_seconds.map(|s| now as i64 + s),
        })?;

        // Persist advanced MLS state.
        let new_state = session
            .serialize_state(&provider)
            .map_err(crate::error::ClientError::Protocol)?;
        self.db.mls_groups().upsert(&MlsGroupRow {
            group_id: mls_row.group_id,
            contact_id,
            state_blob: new_state,
            current_epoch: session.current_epoch(),
            created_at: mls_row.created_at,
            last_updated: now as i64,
        })?;

        Ok(())
    }

    /// List messages for a contact, oldest first, paginated.
    pub fn list_messages(
        &self,
        contact_id: &ghost_core::GhostId,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<MessageRow>> {
        Ok(self
            .db
            .messages()
            .list_for_contact(contact_id, limit, offset)?)
    }

    /// Spawn a background task that drains the Server's inbox and processes incoming envelopes.
    ///
    /// Takes ownership of the inner `Server`. Returns a `JoinHandle`; dropping it aborts the task.
    /// Individual envelope errors are logged via `eprintln!` — one bad message does not kill the
    /// processor loop.
    pub async fn start_inbox_processor(&self) -> Result<tokio::task::JoinHandle<()>> {
        let server = {
            let mut guard = self.server.lock().await;
            guard
                .take()
                .ok_or_else(|| ClientError::Internal("server already taken for inbox".into()))?
        };

        let db = self.db.clone();
        let ik = self.ik.clone();
        let mls_provider = self.mls_provider.clone();

        let handle = tokio::spawn(async move {
            let mut server = server;
            loop {
                let envelope = match server.next_inbox().await {
                    Some(e) => e,
                    None => break,
                };
                if let Err(e) = process_envelope(&db, &ik, &mls_provider, &envelope.envelope).await
                {
                    eprintln!("inbox process error: {e}");
                }
            }
        });
        Ok(handle)
    }
}

/// Spawn a 60s-tick background task that deletes messages whose `expires_at`
/// is past now. The JoinHandle is intentionally dropped — the task lives for
/// the process lifetime (the underlying DB Arc keeps it alive as long as the
/// Client exists; when the Client drops, the DB Arc count drops and the next
/// purge call errors, which simply ends the task).
/// Resolve a peer's reachable Multiaddr. Tries (in order):
///   1. The address embedded in the invite (if provided).
///   2. The most recent `AddressRecord` published in the DHT.
///
/// Returns an error if neither path yields a probe-able address.
impl Client {
    async fn resolve_peer_address(
        &self,
        ghost_id: &ghost_core::GhostId,
        peer_id: libp2p::PeerId,
        invite_addr: Option<Multiaddr>,
    ) -> Result<Multiaddr> {
        // Path 1: invite address. We optimistically use it without probing —
        // request-response queues the request and surfaces a dial failure if
        // it's stale, which we map to a friendly error downstream.
        if let Some(addr) = invite_addr.clone() {
            return Ok(addr);
        }
        // Path 2: DHT lookup.
        let net = self.network.lock().await;
        let record = net
            .get_address_record(*ghost_id)
            .await
            .map_err(crate::error::ClientError::Network)?;
        drop(net);
        if let Some(rec) = record {
            // Verify signature is intact + check expiry.
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if rec.expires_at > now {
                if let Some(addr_str) = rec.endpoints.first() {
                    if let Ok(addr) = addr_str.parse::<Multiaddr>() {
                        tracing::info!(
                            target: "ghost-client",
                            %peer_id, "resolved peer address via DHT",
                        );
                        return Ok(addr);
                    }
                }
            }
        }
        Err(crate::error::ClientError::PeerUnreachable(
            "peer has no reachable address (not in DHT, no invite endpoint)".into(),
        ))
    }
}

/// Translate libp2p dial failures into a user-friendly error variant.
fn map_dial_error(e: crate::error::ClientError) -> crate::error::ClientError {
    let msg = e.to_string();
    if msg.contains("outbound failure")
        || msg.contains("Failed to dial")
        || msg.contains("DialFailure")
        || msg.contains("Timeout")
    {
        crate::error::ClientError::PeerUnreachable(
            "не удалось дозвониться до контакта — собеседник не запущен или не в одной сети".into(),
        )
    } else {
        e
    }
}

/// Publish our own AddressRecord to the DHT every 5 minutes (per spec §5).
/// Each tick re-queries the network for the current listen addresses (libp2p
/// adds observed/external addresses after startup via identify + AutoNAT, so
/// a startup snapshot would be stale).
///
/// First publish runs after a short delay (~10s) to let identify exchange
/// with the bootstrap nodes settle, so observed_addr arrives before the
/// initial DHT publish.
fn spawn_address_publisher(network: Arc<Mutex<Network>>, ik: Arc<IdentityKey>) {
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            // Re-query so we pick up addresses libp2p discovered after startup
            // (NAT-mapped external addr from identify, observed_addr, etc).
            let live_addrs = network.lock().await.local_addrs().await;
            let endpoints: Vec<String> = live_addrs
                .iter()
                .filter(|a| !is_loopback_multiaddr(a))
                .map(|a| a.to_string())
                .collect();
            if endpoints.is_empty() {
                tracing::debug!(target: "ghost-client", "no non-loopback addresses to publish yet");
            } else {
                let record = match ghost_network::AddressRecord::new(&ik, endpoints, now, 600) {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!(target: "ghost-client", "address record sign failed: {e}");
                        tick.tick().await;
                        continue;
                    }
                };
                let net = network.lock().await;
                match net.put_address_record(record).await {
                    Ok(()) => tracing::info!(target: "ghost-client", "published AddressRecord"),
                    Err(e) => tracing::warn!(
                        target: "ghost-client", "DHT put_address_record failed: {e}"
                    ),
                }
            }
            tick.tick().await;
        }
    });
}

fn spawn_retention_scrubber(db: Arc<Database>) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            tick.tick().await;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            if let Err(e) = db.messages().purge_expired(now) {
                tracing::warn!(target: "ghost-client", "purge_expired failed: {e}");
            }
        }
    });
}

async fn process_envelope(
    db: &Arc<Database>,
    ik: &Arc<IdentityKey>,
    mls_provider: &Arc<Mutex<GhostMlsProvider>>,
    envelope_bytes: &[u8],
) -> Result<()> {
    // Use lenient unwrap that skips sender-DK signature verification. Required for first-contact
    // flows where we don't yet have the sender's DK. MLS authentication is the primary security
    // mechanism; the outer Ed25519 signature is defense-in-depth to revisit in MVP-2.
    let unwrapped = unwrap_message_lenient(envelope_bytes, ik).map_err(ClientError::Protocol)?;

    match unwrapped.payload_type {
        PayloadType::AppText => handle_app_text(db, &unwrapped).await,
        PayloadType::MlsHandshake => handle_mls_handshake(db, mls_provider, &unwrapped).await,
        other => Err(ClientError::Internal(format!(
            "unsupported payload type: {other:?}"
        ))),
    }
}

async fn handle_app_text(db: &Arc<Database>, unwrapped: &UnwrappedMessage) -> Result<()> {
    let sender_id = unwrapped.sender_id;
    let now = current_unix_secs();

    let mls_row = db
        .mls_groups()
        .load_for_contact(&sender_id)?
        .ok_or_else(|| ClientError::MlsGroupNotFound(format!("{sender_id}")))?;

    let (provider, mut session) =
        MlsSession::deserialize_state(&mls_row.state_blob).map_err(ClientError::Protocol)?;

    let plaintext = session
        .decrypt_app_message(&provider, &unwrapped.payload)
        .map_err(ClientError::Protocol)?;
    let text = String::from_utf8_lossy(&plaintext).to_string();

    let msg_uuid = *unwrapped.msg_uuid.as_bytes();
    db.messages().insert(&MessageRow {
        msg_uuid,
        contact_id: sender_id,
        direction: Direction::Incoming,
        content_type: 0,
        content: text,
        sent_at: now as i64,
        received_at: Some(now as i64),
        status: MessageStatus::Delivered,
        reply_to: None,
        expires_at: None,
    })?;

    let new_state = session
        .serialize_state(&provider)
        .map_err(ClientError::Protocol)?;
    db.mls_groups().upsert(&MlsGroupRow {
        group_id: mls_row.group_id,
        contact_id: sender_id,
        state_blob: new_state,
        current_epoch: session.current_epoch(),
        created_at: mls_row.created_at,
        last_updated: now as i64,
    })?;

    Ok(())
}

async fn handle_mls_handshake(
    db: &Arc<Database>,
    mls_provider: &Arc<Mutex<GhostMlsProvider>>,
    unwrapped: &UnwrappedMessage,
) -> Result<()> {
    let sender_id = unwrapped.sender_id;
    let now = current_unix_secs();

    let welcome_in = MlsMessageIn::tls_deserialize(&mut unwrapped.payload.as_slice())
        .map_err(|e| ClientError::Internal(format!("welcome deserialize: {e}")))?;

    // Lock the shared provider — it holds the init private keys registered during
    // ensure_keypackages. Without this, join_via_welcome cannot find the matching key.
    let provider = mls_provider.lock().await;
    let session =
        MlsSession::join_via_welcome(&provider, welcome_in).map_err(ClientError::Protocol)?;

    // Persist or update contact.
    let fingerprint = Fingerprint::of(&sender_id).to_string();
    let contact = Contact {
        ghost_id: sender_id,
        display_name: None,
        local_alias: None,
        fingerprint,
        added_at: now as i64,
        last_endpoint: None,
        verification: Verification::Unverified,
        notes: None,
        blocked: false,
        dk_pub: None,
        last_read_at: 0,
        pinned: false,
        muted: false,
        retention_seconds: None,
    };
    let existing = db.contacts().get(&sender_id)?;
    if existing.is_some() {
        db.contacts().update(&contact)?;
    } else {
        db.contacts().insert(&contact)?;
    }

    // Persist MLS group state using the same provider (still locked).
    let state_blob = session
        .serialize_state(&provider)
        .map_err(ClientError::Protocol)?;
    let group_id = session.group_id_bytes();
    db.mls_groups().upsert(&MlsGroupRow {
        group_id: bytes_to_array_32(&group_id),
        contact_id: sender_id,
        state_blob,
        current_epoch: session.current_epoch(),
        created_at: now as i64,
        last_updated: now as i64,
    })?;

    Ok(())
}

fn current_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn bytes_to_array_32(b: &[u8]) -> [u8; 32] {
    *blake3::hash(b).as_bytes()
}

/// Wait for at least one listen address, then keep collecting for an additional
/// settling window so libp2p can emit a NewListenAddr per interface (typical
/// when listening on `/ip4/0.0.0.0/udp/0/...`).
async fn wait_for_local_addrs(network: &Arc<Mutex<Network>>) -> Vec<Multiaddr> {
    let initial_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    // Wait for the first address.
    loop {
        let addrs = network.lock().await.local_addrs().await;
        if !addrs.is_empty() {
            break;
        }
        if tokio::time::Instant::now() > initial_deadline {
            return Vec::new();
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // Settle: collect any additional NewListenAddr events for 800ms.
    tokio::time::sleep(Duration::from_millis(800)).await;
    let mut addrs = network.lock().await.local_addrs().await;
    // Filter loopback — it's never useful for a peer connection and clutters
    // the Diagnostics page + invite payload.
    addrs.retain(|a| !is_loopback_multiaddr(a));
    addrs
}

fn is_loopback_multiaddr(addr: &Multiaddr) -> bool {
    use libp2p::multiaddr::Protocol;
    for proto in addr.iter() {
        match proto {
            Protocol::Ip4(ip) if ip.is_loopback() => return true,
            Protocol::Ip6(ip) if ip.is_loopback() => return true,
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GhostInvite;
    use ghost_identity::{keystore, CreateOptions, Identity};
    use std::sync::Mutex as StdMutex;
    use tempfile::tempdir;

    static LOCK: StdMutex<()> = StdMutex::new(());

    fn isolated_setup() -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        std::env::set_var("GHOST_HOME", dir.path());
        let _ = keystore::wipe_secret();
        Identity::create(CreateOptions {
            display_name: Some("Test".to_string()),
            passphrase: None,
            overwrite: true,
        })
        .unwrap();
        dir
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn open_succeeds_with_seeded_identity() {
        let _dir = {
            let _g = LOCK.lock().unwrap();
            isolated_setup()
        };

        let client = Client::open(ClientConfig::default()).await.unwrap();
        assert!(!client.local_addrs().is_empty());
        let _ = client.ghost_id();

        let _ = keystore::wipe_secret();
        std::env::remove_var("GHOST_HOME");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn create_invite_round_trips_via_bech32() {
        let _dir = {
            let _g = LOCK.lock().unwrap();
            isolated_setup()
        };

        let client = Client::open(ClientConfig::default()).await.unwrap();
        let invite = client.create_invite(3600).unwrap();
        let s = invite.to_bech32().unwrap();
        let restored = GhostInvite::from_bech32(&s).unwrap();
        assert_eq!(restored.ghost_id, client.ghost_id());
        assert_eq!(restored.addresses.len(), client.local_addrs().len());
        restored.verify(invite.expires_at - 1).unwrap();

        let _ = keystore::wipe_secret();
        std::env::remove_var("GHOST_HOME");
    }
}
