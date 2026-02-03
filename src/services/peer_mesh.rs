//! Peer Mesh Service
//!
//! Handles server-to-server communication for syncing user preferences
//! across multiple Haunt instances.

use crate::config::Config;
use crate::types::Profile;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Information about a peer server in the mesh.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo {
    /// Server ID
    pub server_id: String,
    /// Server region
    pub region: String,
    /// API URL
    pub api_url: String,
    /// WebSocket URL
    pub ws_url: Option<String>,
    /// Whether the peer is currently connected/reachable
    pub connected: bool,
    /// Last successful ping timestamp
    pub last_seen: i64,
    /// Latency in milliseconds
    pub latency_ms: Option<u64>,
}

/// Message types for mesh communication.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MeshMessage {
    /// Sync a user's preferences to peers
    PreferencesSync {
        public_key: String,
        profile: Profile,
        origin_server: String,
        timestamp: i64,
    },
    /// Request preferences for a user from peers
    PreferencesRequest {
        public_key: String,
        requesting_server: String,
    },
    /// Response to preferences request
    PreferencesResponse {
        public_key: String,
        profile: Option<Profile>,
        responding_server: String,
    },
    /// Ping to check peer health
    Ping {
        server_id: String,
        timestamp: i64,
    },
    /// Pong response to ping
    Pong {
        server_id: String,
        timestamp: i64,
        original_timestamp: i64,
    },
}

/// Peer mesh service for cross-server synchronization.
#[derive(Clone)]
pub struct PeerMesh {
    /// This server's ID
    server_id: String,
    /// This server's region
    server_region: String,
    /// Known peers
    peers: Arc<DashMap<String, PeerInfo>>,
    /// Shared authentication key
    shared_key: Option<String>,
    /// HTTP client for peer communication
    client: reqwest::Client,
    /// Pending sync operations
    pending_syncs: Arc<RwLock<Vec<MeshMessage>>>,
}

impl PeerMesh {
    /// Create a new peer mesh instance.
    pub fn new(config: &Config) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        let mesh = Self {
            server_id: config.server_id.clone(),
            server_region: config.server_region.clone(),
            peers: Arc::new(DashMap::new()),
            shared_key: config.mesh_shared_key.clone(),
            client,
            pending_syncs: Arc::new(RwLock::new(Vec::new())),
        };

        // Add configured peers
        for peer_url in &config.mesh_peers {
            if let Some(peer_info) = Self::parse_peer_url(peer_url) {
                mesh.peers.insert(peer_info.server_id.clone(), peer_info);
            }
        }

        info!(
            "Peer mesh initialized for server '{}' with {} peers",
            mesh.server_id,
            mesh.peers.len()
        );

        mesh
    }

    /// Parse a peer URL into PeerInfo.
    fn parse_peer_url(url: &str) -> Option<PeerInfo> {
        // Expected format: https://server-id.haunt.st or https://server-id.haunt.st|region
        let parts: Vec<&str> = url.split('|').collect();
        let api_url = parts[0].trim().to_string();
        let region = parts.get(1).map(|r| r.trim().to_string());

        // Extract server ID from URL
        if let Some(host) = api_url
            .strip_prefix("https://")
            .or_else(|| api_url.strip_prefix("http://"))
        {
            let server_id = host.split('.').next()?.to_string();
            let ws_url = api_url.replace("https://", "wss://").replace("http://", "ws://");

            Some(PeerInfo {
                server_id: server_id.clone(),
                region: region.unwrap_or_else(|| "Unknown".to_string()),
                api_url,
                ws_url: Some(format!("{}/ws", ws_url)),
                connected: false,
                last_seen: 0,
                latency_ms: None,
            })
        } else {
            warn!("Invalid peer URL format: {}", url);
            None
        }
    }

    /// Get this server's ID.
    pub fn server_id(&self) -> &str {
        &self.server_id
    }

    /// Get this server's region.
    pub fn server_region(&self) -> &str {
        &self.server_region
    }

    /// Get list of all known peers.
    pub fn get_peers(&self) -> Vec<PeerInfo> {
        self.peers.iter().map(|r| r.value().clone()).collect()
    }

    /// Get connected peers only.
    pub fn get_connected_peers(&self) -> Vec<PeerInfo> {
        self.peers
            .iter()
            .filter(|r| r.value().connected)
            .map(|r| r.value().clone())
            .collect()
    }

    /// Sync a profile to all connected peers.
    pub async fn sync_profile(&self, profile: &Profile) {
        let message = MeshMessage::PreferencesSync {
            public_key: profile.public_key.clone(),
            profile: profile.clone(),
            origin_server: self.server_id.clone(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        let peers: Vec<PeerInfo> = self.peers.iter().map(|r| r.value().clone()).collect();

        for peer in peers {
            if let Err(e) = self.send_to_peer(&peer, &message).await {
                debug!("Failed to sync profile to {}: {}", peer.server_id, e);
            }
        }
    }

    /// Request a profile from all peers (for initial sync on login).
    pub async fn request_profile(&self, public_key: &str) -> Option<Profile> {
        let message = MeshMessage::PreferencesRequest {
            public_key: public_key.to_string(),
            requesting_server: self.server_id.clone(),
        };

        let peers: Vec<PeerInfo> = self.peers.iter().map(|r| r.value().clone()).collect();
        let mut newest_profile: Option<Profile> = None;
        let mut newest_timestamp: i64 = 0;

        for peer in peers {
            match self.send_to_peer_with_response::<MeshMessage>(&peer, &message).await {
                Ok(MeshMessage::PreferencesResponse { profile: Some(profile), .. }) => {
                    let profile_timestamp = profile.settings.updated_at;
                    if profile_timestamp > newest_timestamp {
                        newest_timestamp = profile_timestamp;
                        newest_profile = Some(profile);
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    debug!("Failed to request profile from {}: {}", peer.server_id, e);
                }
            }
        }

        newest_profile
    }

    /// Send a message to a peer.
    async fn send_to_peer(&self, peer: &PeerInfo, message: &MeshMessage) -> Result<(), String> {
        let url = format!("{}/api/sync/mesh", peer.api_url);

        let mut request = self.client.post(&url).json(message);

        if let Some(ref key) = self.shared_key {
            request = request.header("X-Mesh-Key", key);
        }
        request = request.header("X-Origin-Server", &self.server_id);

        let response = request
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status().is_success() {
            // Update peer as connected
            if let Some(mut peer_ref) = self.peers.get_mut(&peer.server_id) {
                peer_ref.connected = true;
                peer_ref.last_seen = chrono::Utc::now().timestamp_millis();
            }
            Ok(())
        } else {
            Err(format!("Peer returned status: {}", response.status()))
        }
    }

    /// Send a message to a peer and wait for response.
    async fn send_to_peer_with_response<T: for<'de> Deserialize<'de>>(
        &self,
        peer: &PeerInfo,
        message: &MeshMessage,
    ) -> Result<T, String> {
        let url = format!("{}/api/sync/mesh", peer.api_url);

        let mut request = self.client.post(&url).json(message);

        if let Some(ref key) = self.shared_key {
            request = request.header("X-Mesh-Key", key);
        }
        request = request.header("X-Origin-Server", &self.server_id);

        let response = request
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status().is_success() {
            // Update peer as connected
            if let Some(mut peer_ref) = self.peers.get_mut(&peer.server_id) {
                peer_ref.connected = true;
                peer_ref.last_seen = chrono::Utc::now().timestamp_millis();
            }

            response
                .json::<T>()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e))
        } else {
            Err(format!("Peer returned status: {}", response.status()))
        }
    }

    /// Ping all peers to check connectivity.
    pub async fn ping_all_peers(&self) {
        let now = chrono::Utc::now().timestamp_millis();
        let peers: Vec<PeerInfo> = self.peers.iter().map(|r| r.value().clone()).collect();

        for peer in peers {
            let message = MeshMessage::Ping {
                server_id: self.server_id.clone(),
                timestamp: now,
            };

            let start = std::time::Instant::now();
            match self.send_to_peer_with_response::<MeshMessage>(&peer, &message).await {
                Ok(MeshMessage::Pong { original_timestamp, .. }) => {
                    let latency = start.elapsed().as_millis() as u64;
                    if let Some(mut peer_ref) = self.peers.get_mut(&peer.server_id) {
                        peer_ref.connected = true;
                        peer_ref.last_seen = chrono::Utc::now().timestamp_millis();
                        peer_ref.latency_ms = Some(latency);
                    }
                    debug!("Ping to {} successful: {}ms", peer.server_id, latency);
                }
                Ok(_) => {
                    warn!("Unexpected response from {}", peer.server_id);
                }
                Err(e) => {
                    if let Some(mut peer_ref) = self.peers.get_mut(&peer.server_id) {
                        peer_ref.connected = false;
                    }
                    debug!("Ping to {} failed: {}", peer.server_id, e);
                }
            }
        }
    }

    /// Handle an incoming mesh message.
    pub async fn handle_message(
        &self,
        message: MeshMessage,
        auth_service: &crate::services::AuthService,
    ) -> Option<MeshMessage> {
        match message {
            MeshMessage::PreferencesSync {
                public_key,
                profile,
                origin_server,
                timestamp,
            } => {
                debug!(
                    "Received preferences sync from {} for user {}",
                    origin_server,
                    &public_key[..16.min(public_key.len())]
                );

                // Check if we should accept this update (newer timestamp wins)
                if let Some(existing) = auth_service.get_profile(&public_key).await {
                    if existing.settings.updated_at >= timestamp {
                        debug!("Ignoring older preferences from {}", origin_server);
                        return None;
                    }
                }

                // Update local profile
                if let Err(e) = auth_service.update_profile(profile).await {
                    error!("Failed to update profile from sync: {}", e);
                }

                None
            }

            MeshMessage::PreferencesRequest {
                public_key,
                requesting_server,
            } => {
                debug!(
                    "Received preferences request from {} for user {}",
                    requesting_server,
                    &public_key[..16.min(public_key.len())]
                );

                let profile = auth_service.get_profile(&public_key).await;

                Some(MeshMessage::PreferencesResponse {
                    public_key,
                    profile,
                    responding_server: self.server_id.clone(),
                })
            }

            MeshMessage::Ping {
                server_id,
                timestamp,
            } => {
                debug!("Received ping from {}", server_id);
                Some(MeshMessage::Pong {
                    server_id: self.server_id.clone(),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    original_timestamp: timestamp,
                })
            }

            MeshMessage::Pong { .. } => {
                // Pong is handled in the request flow
                None
            }

            MeshMessage::PreferencesResponse { .. } => {
                // Response is handled in the request flow
                None
            }
        }
    }

    /// Start the background peer health check task.
    pub fn start_health_check(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                self.ping_all_peers().await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_peer_url() {
        let peer = PeerMesh::parse_peer_url("https://osaka.haunt.st|Asia Pacific").unwrap();
        assert_eq!(peer.server_id, "osaka");
        assert_eq!(peer.region, "Asia Pacific");
        assert_eq!(peer.api_url, "https://osaka.haunt.st");
        assert_eq!(peer.ws_url, Some("wss://osaka.haunt.st/ws".to_string()));
    }

    #[test]
    fn test_parse_peer_url_no_region() {
        let peer = PeerMesh::parse_peer_url("https://seoul.haunt.st").unwrap();
        assert_eq!(peer.server_id, "seoul");
        assert_eq!(peer.region, "Unknown");
    }

    #[test]
    fn test_mesh_message_serialization() {
        let message = MeshMessage::Ping {
            server_id: "test".to_string(),
            timestamp: 1234567890,
        };

        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("\"type\":\"ping\""));

        let parsed: MeshMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            MeshMessage::Ping { server_id, timestamp } => {
                assert_eq!(server_id, "test");
                assert_eq!(timestamp, 1234567890);
            }
            _ => panic!("Wrong message type"),
        }
    }
}
