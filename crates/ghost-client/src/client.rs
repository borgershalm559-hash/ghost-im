//! Client orchestration: opens Identity + Database + Network + Server, runs background tasks.

use crate::Result;
use ghost_identity::{Identity, IdentityKey};
use ghost_network::Network;
use ghost_server::{PresenceState, Server};
use ghost_storage::{derive_master_key, Database};
use libp2p::{Multiaddr, PeerId};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

pub struct ClientConfig {
    pub listen_addr: Multiaddr,
    pub passphrase: Option<String>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            listen_addr: "/ip4/127.0.0.1/udp/0/quic-v1".parse().expect("valid multiaddr"),
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
    pub(crate) presence: Arc<Mutex<PresenceState>>,
    pub(crate) local_peer_id: PeerId,
    pub(crate) local_addrs: Vec<Multiaddr>,
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

        let db_path = ghost_identity::database_file()
            .map_err(ghost_identity::IdentityError::from)?;
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

        let server = Server::spawn(ik.clone(), network.clone(), presence.clone(), db.clone())?;

        Ok(Self {
            identity,
            ik,
            db,
            network,
            server: Mutex::new(Some(server)),
            presence,
            local_peer_id,
            local_addrs,
        })
    }

    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    pub fn local_addrs(&self) -> &[Multiaddr] {
        &self.local_addrs
    }

    pub fn ghost_id(&self) -> ghost_core::GhostId {
        self.ik.ghost_id()
    }
}

async fn wait_for_local_addrs(network: &Arc<Mutex<Network>>) -> Vec<Multiaddr> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let addrs = network.lock().await.local_addrs().await;
        if !addrs.is_empty() {
            return addrs;
        }
        if tokio::time::Instant::now() > deadline {
            return Vec::new();
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let _g = LOCK.lock().unwrap();
        let _dir = isolated_setup();

        let client = Client::open(ClientConfig::default()).await.unwrap();
        assert!(!client.local_addrs().is_empty());
        let _ = client.ghost_id();

        let _ = keystore::wipe_secret();
        std::env::remove_var("GHOST_HOME");
    }
}
