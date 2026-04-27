//! Client: typed wrappers over Network::request for each endpoint.

use crate::messages::{GhostRequest, GhostResponse};
use crate::{Result, ServerError};
use ghost_network::Network;
use libp2p::{Multiaddr, PeerId};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Typed wrappers over `Network::request`. Builds the typed `GhostRequest`,
/// sends it as CBOR bytes, decodes the typed `GhostResponse`.
pub struct Client {
    network: Arc<Mutex<Network>>,
}

impl Client {
    pub fn new(network: Arc<Mutex<Network>>) -> Self {
        Self { network }
    }

    async fn send_request(
        &self,
        peer: PeerId,
        addr: Option<Multiaddr>,
        req: GhostRequest,
    ) -> Result<GhostResponse> {
        let bytes = req.to_cbor()?;
        let net = self.network.lock().await;
        let response_bytes = net.request(peer, addr, bytes).await?;
        drop(net);
        let resp = GhostResponse::from_cbor(&response_bytes)?;
        Ok(resp)
    }

    pub async fn get_version(
        &self,
        peer: PeerId,
        addr: Option<Multiaddr>,
    ) -> Result<(String, String)> {
        let resp = self.send_request(peer, addr, GhostRequest::Version).await?;
        match resp.into_ok()? {
            GhostResponse::Version {
                protocol,
                min_compat,
            } => Ok((protocol, min_compat)),
            other => Err(ServerError::InvalidResponse(format!(
                "expected Version, got {other:?}"
            ))),
        }
    }

    pub async fn get_delivery_key(
        &self,
        peer: PeerId,
        addr: Option<Multiaddr>,
    ) -> Result<[u8; 32]> {
        let resp = self
            .send_request(peer, addr, GhostRequest::GetDeliveryKey)
            .await?;
        match resp.into_ok()? {
            GhostResponse::DeliveryKey { x25519_pub } => Ok(x25519_pub),
            other => Err(ServerError::InvalidResponse(format!(
                "expected DeliveryKey, got {other:?}"
            ))),
        }
    }

    pub async fn get_key_package(&self, peer: PeerId, addr: Option<Multiaddr>) -> Result<Vec<u8>> {
        let resp = self
            .send_request(peer, addr, GhostRequest::GetKeyPackage)
            .await?;
        match resp.into_ok()? {
            GhostResponse::KeyPackage { bytes } => Ok(bytes),
            other => Err(ServerError::InvalidResponse(format!(
                "expected KeyPackage, got {other:?}"
            ))),
        }
    }

    pub async fn get_presence(&self, peer: PeerId, addr: Option<Multiaddr>) -> Result<(bool, u64)> {
        let resp = self
            .send_request(peer, addr, GhostRequest::GetPresence)
            .await?;
        match resp.into_ok()? {
            GhostResponse::Presence { online, last_seen } => Ok((online, last_seen)),
            other => Err(ServerError::InvalidResponse(format!(
                "expected Presence, got {other:?}"
            ))),
        }
    }

    pub async fn send_inbox(
        &self,
        peer: PeerId,
        addr: Option<Multiaddr>,
        envelope: Vec<u8>,
    ) -> Result<()> {
        let resp = self
            .send_request(peer, addr, GhostRequest::InboxMessage { envelope })
            .await?;
        match resp.into_ok()? {
            GhostResponse::InboxAck => Ok(()),
            other => Err(ServerError::InvalidResponse(format!(
                "expected InboxAck, got {other:?}"
            ))),
        }
    }
}
