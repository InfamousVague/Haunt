//! Peer mesh service for multi-server connectivity and real-time ping monitoring.
//!
//! This module manages WebSocket connections between Haunt API servers,
//! providing real-time latency tracking and server health monitoring.

use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, RwLock};
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, warn};

// Re-export types from the types module
pub use crate::types::{PeerConfig, PeerConnectionStatus, PeerInfo, PeerMessage, PeerStatus};

/// Internal peer connection state.
struct PeerConnection {
    config: PeerConfig,
    status: PeerConnectionStatus,
    latency_history: Vec<f64>,
    ping_count: u64,
    failed_pings: u64,
    last_ping_at: Option<Instant>,
    last_attempt_at: Option<Instant>,
    connected_at: Option<Instant>,
}

impl PeerConnection {
    fn new(config: PeerConfig) -> Self {
        Self {
            config,
            status: PeerConnectionStatus::Disconnected,
            latency_history: Vec::with_capacity(60),
            ping_count: 0,
            failed_pings: 0,
            last_ping_at: None,
            last_attempt_at: None,
            connected_at: None,
        }
    }

    fn record_latency(&mut self, latency_ms: f64) {
        self.latency_history.push(latency_ms);
        // Keep only last 60 samples (about 1 minute at 1 ping/sec)
        if self.latency_history.len() > 60 {
            self.latency_history.remove(0);
        }
        self.ping_count += 1;
        self.last_ping_at = Some(Instant::now());
    }

    fn record_failure(&mut self) {
        self.failed_pings += 1;
    }

    fn get_status(&self) -> PeerStatus {
        let now = chrono::Utc::now().timestamp_millis();

        let (avg, min, max) = if !self.latency_history.is_empty() {
            let sum: f64 = self.latency_history.iter().sum();
            let avg = sum / self.latency_history.len() as f64;
            let min = self
                .latency_history
                .iter()
                .cloned()
                .fold(f64::INFINITY, f64::min);
            let max = self
                .latency_history
                .iter()
                .cloned()
                .fold(f64::NEG_INFINITY, f64::max);
            (Some(avg), Some(min), Some(max))
        } else {
            (None, None, None)
        };

        let total_pings = self.ping_count + self.failed_pings;
        let uptime = if total_pings > 0 {
            (self.ping_count as f64 / total_pings as f64) * 100.0
        } else {
            0.0
        };

        PeerStatus {
            id: self.config.id.clone(),
            region: self.config.region.clone(),
            status: self.status.clone(),
            latency_ms: self.latency_history.last().copied(),
            avg_latency_ms: avg,
            min_latency_ms: min,
            max_latency_ms: max,
            ping_count: self.ping_count,
            failed_pings: self.failed_pings,
            uptime_percent: uptime,
            last_ping_at: self.last_ping_at.map(|_| now),
            last_attempt_at: self.last_attempt_at.map(|_| now),
        }
    }
}

/// Type alias for HMAC-SHA256.
type HmacSha256 = Hmac<Sha256>;

/// Discovered peer from gossip protocol.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DiscoveredPeer {
    pub config: PeerConfig,
    pub last_seen: i64,
    pub discovered_from: String,
}

/// Manages peer mesh connections and real-time ping monitoring.
pub struct PeerMesh {
    /// This server's ID.
    server_id: String,
    /// This server's region.
    server_region: String,
    /// This server's WebSocket URL (for announcements).
    ws_url: String,
    /// This server's API URL (for announcements).
    api_url: String,
    /// Connected peers.
    peers: DashMap<String, RwLock<PeerConnection>>,
    /// Discovered peers from gossip (not yet connected).
    known_peers: DashMap<String, DiscoveredPeer>,
    /// Broadcast channel for peer status updates.
    status_tx: broadcast::Sender<Vec<PeerStatus>>,
    /// Pending ping timestamps for latency calculation.
    pending_pings: DashMap<String, Instant>,
    /// Shared key for mesh authentication.
    shared_key: Option<String>,
    /// Whether authentication is required.
    require_auth: bool,
}

impl PeerMesh {
    /// Create a new peer mesh manager.
    pub fn new(
        server_id: String,
        server_region: String,
        ws_url: String,
        api_url: String,
        shared_key: Option<String>,
        require_auth: bool,
    ) -> Arc<Self> {
        let (status_tx, _) = broadcast::channel(256);

        Arc::new(Self {
            server_id,
            server_region,
            ws_url,
            api_url,
            peers: DashMap::new(),
            known_peers: DashMap::new(),
            status_tx,
            pending_pings: DashMap::new(),
            shared_key,
            require_auth,
        })
    }

    /// Get this server's WebSocket URL.
    pub fn ws_url(&self) -> &str {
        &self.ws_url
    }

    /// Get this server's API URL.
    pub fn api_url(&self) -> &str {
        &self.api_url
    }

    /// Generate an HMAC signature for authentication.
    fn generate_signature(&self, message: &str) -> Option<String> {
        self.shared_key.as_ref().map(|key| {
            let mut mac =
                HmacSha256::new_from_slice(key.as_bytes()).expect("HMAC can take key of any size");
            mac.update(message.as_bytes());
            let result = mac.finalize();
            hex::encode(result.into_bytes())
        })
    }

    /// Verify an HMAC signature.
    fn verify_signature(&self, message: &str, signature: &str) -> bool {
        match &self.shared_key {
            Some(key) => {
                let mut mac = HmacSha256::new_from_slice(key.as_bytes())
                    .expect("HMAC can take key of any size");
                mac.update(message.as_bytes());

                if let Ok(expected) = hex::decode(signature) {
                    mac.verify_slice(&expected).is_ok()
                } else {
                    false
                }
            }
            None => !self.require_auth, // If no key and auth not required, pass
        }
    }

    /// Subscribe to peer status updates.
    pub fn subscribe(&self) -> broadcast::Receiver<Vec<PeerStatus>> {
        self.status_tx.subscribe()
    }

    /// Get this server's ID.
    pub fn server_id(&self) -> &str {
        &self.server_id
    }

    /// Get this server's region.
    pub fn server_region(&self) -> &str {
        &self.server_region
    }

    /// Add a peer server to the mesh.
    pub fn add_peer(&self, config: PeerConfig) {
        let peer_id = config.id.clone();
        self.peers
            .insert(peer_id, RwLock::new(PeerConnection::new(config)));
    }

    /// Get all peer statuses.
    pub fn get_all_statuses(&self) -> Vec<PeerStatus> {
        let mut statuses = Vec::new();
        for entry in self.peers.iter() {
            if let Ok(peer) = entry.value().try_read() {
                statuses.push(peer.get_status());
            }
        }
        statuses
    }

    /// Get a specific peer's status.
    pub fn get_peer_status(&self, peer_id: &str) -> Option<PeerStatus> {
        self.peers
            .get(peer_id)
            .and_then(|entry| entry.try_read().ok().map(|peer| peer.get_status()))
    }

    /// Broadcast current peer statuses to all subscribers.
    pub fn broadcast_statuses(&self) {
        let statuses = self.get_all_statuses();
        let _ = self.status_tx.send(statuses);
    }

    /// Get all known peers (discovered via gossip).
    pub fn get_known_peers(&self) -> Vec<PeerInfo> {
        let now = chrono::Utc::now().timestamp_millis();
        self.known_peers
            .iter()
            .map(|entry| {
                let discovered = entry.value();
                PeerInfo {
                    id: discovered.config.id.clone(),
                    region: discovered.config.region.clone(),
                    ws_url: discovered.config.ws_url.clone(),
                    api_url: discovered.config.api_url.clone(),
                    last_seen: discovered.last_seen,
                    status: if now - discovered.last_seen < 120_000 {
                        PeerConnectionStatus::Connected
                    } else {
                        PeerConnectionStatus::Disconnected
                    },
                    latency_ms: None,
                }
            })
            .collect()
    }

    /// Generate announcement signature.
    fn generate_announce_signature(&self, timestamp: i64) -> Option<String> {
        self.shared_key.as_ref().map(|key| {
            let message = format!(
                "announce:{}:{}:{}",
                self.server_id, self.server_region, timestamp
            );
            let mut mac =
                HmacSha256::new_from_slice(key.as_bytes()).expect("HMAC can take key of any size");
            mac.update(message.as_bytes());
            hex::encode(mac.finalize().into_bytes())
        })
    }

    /// Verify announcement signature.
    fn verify_announce_signature(
        &self,
        id: &str,
        region: &str,
        timestamp: i64,
        signature: &str,
    ) -> bool {
        match &self.shared_key {
            Some(key) => {
                let message = format!("announce:{}:{}:{}", id, region, timestamp);
                let mut mac = HmacSha256::new_from_slice(key.as_bytes())
                    .expect("HMAC can take key of any size");
                mac.update(message.as_bytes());
                if let Ok(expected) = hex::decode(signature) {
                    mac.verify_slice(&expected).is_ok()
                } else {
                    false
                }
            }
            None => !self.require_auth,
        }
    }

    /// Create an announcement message for this server.
    pub fn create_announce(&self) -> Option<PeerMessage> {
        let timestamp = chrono::Utc::now().timestamp_millis();
        let signature = self.generate_announce_signature(timestamp)?;

        Some(PeerMessage::Announce {
            id: self.server_id.clone(),
            region: self.server_region.clone(),
            ws_url: self.ws_url.clone(),
            api_url: self.api_url.clone(),
            signature,
            timestamp,
        })
    }

    /// Handle an announce message - add peer to known peers and optionally connect.
    pub fn handle_announce(
        &self,
        id: String,
        region: String,
        ws_url: String,
        api_url: String,
        signature: String,
        timestamp: i64,
    ) -> bool {
        // Don't process our own announcements
        if id == self.server_id {
            return false;
        }

        // Verify signature
        if !self.verify_announce_signature(&id, &region, timestamp, &signature) {
            warn!("Invalid announce signature from {}", id);
            return false;
        }

        // Check timestamp freshness (within 5 minutes)
        let now = chrono::Utc::now().timestamp_millis();
        if (now - timestamp).abs() > 300_000 {
            warn!(
                "Stale announce from {} (timestamp {} vs now {})",
                id, timestamp, now
            );
            return false;
        }

        let config = PeerConfig {
            id: id.clone(),
            region: region.clone(),
            ws_url,
            api_url,
        };

        // Add to known peers
        let is_new = !self.known_peers.contains_key(&id);
        self.known_peers.insert(
            id.clone(),
            DiscoveredPeer {
                config: config.clone(),
                last_seen: now,
                discovered_from: "announce".to_string(),
            },
        );

        if is_new {
            info!("Discovered new peer via announce: {} ({})", id, region);
            // Auto-connect to new peers if not already connected
            if !self.peers.contains_key(&id) {
                self.add_peer(config);
            }
        }

        true
    }

    /// Handle received peer list from SharePeers message.
    pub fn handle_share_peers(&self, peers: Vec<PeerInfo>, from_peer: &str) {
        let now = chrono::Utc::now().timestamp_millis();

        for peer_info in peers {
            // Don't add ourselves
            if peer_info.id == self.server_id {
                continue;
            }

            let is_new = !self.known_peers.contains_key(&peer_info.id);

            // Update or add to known peers
            self.known_peers.insert(
                peer_info.id.clone(),
                DiscoveredPeer {
                    config: PeerConfig {
                        id: peer_info.id.clone(),
                        region: peer_info.region.clone(),
                        ws_url: peer_info.ws_url.clone(),
                        api_url: peer_info.api_url.clone(),
                    },
                    last_seen: now,
                    discovered_from: from_peer.to_string(),
                },
            );

            if is_new {
                info!(
                    "Discovered new peer via gossip from {}: {} ({})",
                    from_peer, peer_info.id, peer_info.region
                );
            }
        }
    }

    /// Create a SharePeers message with our known peers.
    pub fn create_share_peers(&self) -> PeerMessage {
        let now = chrono::Utc::now().timestamp_millis();
        let mut peers: Vec<PeerInfo> = Vec::new();

        // Add connected peers
        for entry in self.peers.iter() {
            if let Ok(peer) = entry.value().try_read() {
                let status = peer.get_status();
                peers.push(PeerInfo {
                    id: peer.config.id.clone(),
                    region: peer.config.region.clone(),
                    ws_url: peer.config.ws_url.clone(),
                    api_url: peer.config.api_url.clone(),
                    last_seen: now,
                    status: status.status,
                    latency_ms: status.latency_ms,
                });
            }
        }

        // Add ourselves
        peers.push(PeerInfo {
            id: self.server_id.clone(),
            region: self.server_region.clone(),
            ws_url: self.ws_url.clone(),
            api_url: self.api_url.clone(),
            last_seen: now,
            status: PeerConnectionStatus::Connected,
            latency_ms: Some(0.0),
        });

        PeerMessage::SharePeers { peers }
    }

    /// Prune peers not seen in the given duration.
    pub fn prune_stale_peers(&self, max_age_ms: i64) {
        let now = chrono::Utc::now().timestamp_millis();
        let stale_keys: Vec<String> = self
            .known_peers
            .iter()
            .filter(|entry| now - entry.value().last_seen > max_age_ms)
            .map(|entry| entry.key().clone())
            .collect();

        for key in stale_keys {
            info!("Pruning stale peer: {}", key);
            self.known_peers.remove(&key);
        }
    }

    /// Connect to any known peers that we're not already connected to.
    pub fn connect_to_discovered_peers(self: &Arc<Self>) {
        for entry in self.known_peers.iter() {
            let peer_id = entry.key().clone();
            if !self.peers.contains_key(&peer_id) {
                let config = entry.value().config.clone();
                info!(
                    "Auto-connecting to discovered peer: {} ({})",
                    peer_id, config.region
                );
                self.add_peer(config);

                // Spawn connection task
                let mesh_clone = self.clone();
                let peer_id_clone = peer_id.clone();
                tokio::spawn(async move {
                    mesh_clone.manage_peer_connection(peer_id_clone).await;
                });
            }
        }
    }

    /// Start the peer mesh (connect to all peers and begin ping monitoring).
    pub fn start(self: Arc<Self>) {
        let mesh = self.clone();

        // Spawn connection tasks for each peer
        for entry in mesh.peers.iter() {
            let peer_id = entry.key().clone();
            let mesh_clone = mesh.clone();

            tokio::spawn(async move {
                mesh_clone.manage_peer_connection(peer_id).await;
            });
        }

        // Spawn status broadcast task (every 100ms for real-time updates)
        let mesh_broadcast = mesh.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            loop {
                interval.tick().await;
                mesh_broadcast.broadcast_statuses();
            }
        });

        // Spawn gossip task (every 30 seconds, share peers with random subset)
        let mesh_gossip = mesh.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                // Check for new discovered peers and connect
                mesh_gossip.connect_to_discovered_peers();
            }
        });

        // Spawn stale peer pruning task (every 5 minutes)
        let mesh_prune = mesh.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300));
            loop {
                interval.tick().await;
                // Prune peers not seen in 10 minutes
                mesh_prune.prune_stale_peers(600_000);
            }
        });
    }

    /// Manage a single peer connection with auto-reconnect.
    async fn manage_peer_connection(self: Arc<Self>, peer_id: String) {
        loop {
            // Update status to connecting
            if let Some(entry) = self.peers.get(&peer_id) {
                let mut peer = entry.write().await;
                peer.status = PeerConnectionStatus::Connecting;
                peer.last_attempt_at = Some(Instant::now());
            }

            // Get the peer config
            let config = {
                if let Some(entry) = self.peers.get(&peer_id) {
                    let peer = entry.read().await;
                    peer.config.clone()
                } else {
                    break; // Peer was removed
                }
            };

            info!("Connecting to peer {} at {}", peer_id, config.ws_url);

            // Attempt connection
            match self.connect_to_peer(&config).await {
                Ok(ws) => {
                    // Update status to connected
                    if let Some(entry) = self.peers.get(&peer_id) {
                        let mut peer = entry.write().await;
                        peer.status = PeerConnectionStatus::Connected;
                        peer.connected_at = Some(Instant::now());
                    }

                    info!("Connected to peer {}", peer_id);

                    // Handle the connection
                    self.clone().handle_peer_connection(&peer_id, ws).await;
                }
                Err(e) => {
                    error!("Failed to connect to peer {}: {}", peer_id, e);

                    // Update status to failed
                    if let Some(entry) = self.peers.get(&peer_id) {
                        let mut peer = entry.write().await;
                        peer.status = PeerConnectionStatus::Failed;
                        peer.record_failure();
                    }
                }
            }

            // Update status to disconnected before reconnect
            if let Some(entry) = self.peers.get(&peer_id) {
                let mut peer = entry.write().await;
                peer.status = PeerConnectionStatus::Disconnected;
            }

            // Wait before reconnect attempt
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    /// Connect to a peer server.
    async fn connect_to_peer(
        &self,
        config: &PeerConfig,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, Box<dyn std::error::Error + Send + Sync>>
    {
        let (ws, _) = connect_async(&config.ws_url).await?;
        Ok(ws)
    }

    /// Handle an active peer connection.
    async fn handle_peer_connection(
        self: Arc<Self>,
        peer_id: &str,
        ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    ) {
        let (mut write, mut read) = ws.split();

        // Send authentication if shared key is configured
        let timestamp = chrono::Utc::now().timestamp_millis();
        let auth_message = format!("{}:{}:{}", self.server_id, self.server_region, timestamp);

        if let Some(signature) = self.generate_signature(&auth_message) {
            let auth_msg = PeerMessage::Auth {
                id: self.server_id.clone(),
                region: self.server_region.clone(),
                timestamp,
                signature,
            };

            if let Ok(json) = serde_json::to_string(&auth_msg) {
                if let Err(e) = write
                    .send(tokio_tungstenite::tungstenite::Message::Text(json))
                    .await
                {
                    error!("Failed to send auth to {}: {}", peer_id, e);
                    return;
                }
            }

            // Wait for auth response with timeout
            let auth_timeout = tokio::time::timeout(Duration::from_secs(5), read.next()).await;
            match auth_timeout {
                Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text)))) => {
                    if let Ok(PeerMessage::AuthResponse { success, error }) =
                        serde_json::from_str(&text)
                    {
                        if !success {
                            error!("Auth failed for peer {}: {:?}", peer_id, error);
                            return;
                        }
                        info!("Authenticated with peer {}", peer_id);
                    } else {
                        warn!("Peer {} sent unexpected response, proceeding without auth confirmation", peer_id);
                    }
                }
                Ok(_) => {
                    warn!("Peer {} closed connection during auth", peer_id);
                    return;
                }
                Err(_) => {
                    warn!("Auth timeout for peer {}, proceeding without confirmation (peer may be running older version)", peer_id);
                }
            }
        } else {
            // No auth configured, send identification for backwards compatibility
            let identify_msg = PeerMessage::Identify {
                id: self.server_id.clone(),
                region: self.server_region.clone(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            };

            if let Ok(json) = serde_json::to_string(&identify_msg) {
                if let Err(e) = write
                    .send(tokio_tungstenite::tungstenite::Message::Text(json))
                    .await
                {
                    error!("Failed to send identify to {}: {}", peer_id, e);
                    return;
                }
            }
        }

        // Spawn ping task (every 1 second for real-time latency)
        let ping_peer_id = peer_id.to_string();
        let ping_mesh = self.clone();

        let (ping_tx, mut ping_rx) = tokio::sync::mpsc::channel::<String>(32);

        let ping_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            let mut ping_count = 0u32;
            loop {
                interval.tick().await;

                let timestamp = chrono::Utc::now().timestamp_millis();
                let ping_msg = PeerMessage::Ping {
                    from_id: ping_mesh.server_id.clone(),
                    from_region: ping_mesh.server_region.clone(),
                    timestamp,
                };

                if let Ok(json) = serde_json::to_string(&ping_msg) {
                    // Record pending ping
                    ping_mesh
                        .pending_pings
                        .insert(ping_peer_id.clone(), Instant::now());
                    ping_count += 1;

                    if ping_count <= 3 || ping_count.is_multiple_of(10) {
                        debug!("Sending ping #{} to {}", ping_count, ping_peer_id);
                    }

                    if ping_tx.send(json).await.is_err() {
                        info!(
                            "Ping channel closed for {}, stopping ping task",
                            ping_peer_id
                        );
                        break;
                    }
                }
            }
        });

        // Spawn write task
        let write_task = tokio::spawn(async move {
            while let Some(msg) = ping_rx.recv().await {
                if write
                    .send(tokio_tungstenite::tungstenite::Message::Text(msg))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });

        info!("Starting ping loop for peer {}", peer_id);

        // Handle incoming messages
        while let Some(result) = read.next().await {
            match result {
                Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                    debug!(
                        "Received from {}: {}",
                        peer_id,
                        &text[..text.len().min(100)]
                    );
                    if let Ok(msg) = serde_json::from_str::<PeerMessage>(&text) {
                        self.handle_peer_message(peer_id, msg).await;
                    } else {
                        debug!("Failed to parse message from {} as PeerMessage", peer_id);
                    }
                }
                Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => {
                    info!("Peer {} closed connection", peer_id);
                    break;
                }
                Ok(tokio_tungstenite::tungstenite::Message::Ping(_)) => {
                    debug!("Received WebSocket ping from {}", peer_id);
                }
                Err(e) => {
                    error!("WebSocket error from peer {}: {}", peer_id, e);
                    break;
                }
                _ => {}
            }
        }
        info!("Exiting message loop for peer {}", peer_id);

        ping_task.abort();
        write_task.abort();
    }

    /// Handle an incoming peer message.
    async fn handle_peer_message(&self, peer_id: &str, msg: PeerMessage) {
        match msg {
            PeerMessage::Auth {
                id,
                region,
                timestamp,
                signature,
            } => {
                // Verify incoming auth request
                let auth_message = format!("{}:{}:{}", id, region, timestamp);
                let success = self.verify_signature(&auth_message, &signature);

                if success {
                    info!("Peer {} authenticated successfully", id);
                } else {
                    warn!("Peer {} failed authentication", id);
                }

                // Note: Auth response would be sent if this were an incoming connection handler
            }
            PeerMessage::AuthResponse { success, error } => {
                if success {
                    debug!("Received auth success from {}", peer_id);
                } else {
                    warn!("Auth rejected by {}: {:?}", peer_id, error);
                }
            }
            PeerMessage::Pong { .. } => {
                debug!("Received Pong from peer {}", peer_id);
                // Calculate latency
                if let Some((_, sent_at)) = self.pending_pings.remove(peer_id) {
                    let latency_ms = sent_at.elapsed().as_secs_f64() * 1000.0;

                    // Record latency
                    if let Some(entry) = self.peers.get(peer_id) {
                        let mut peer = entry.write().await;
                        peer.record_latency(latency_ms);
                        info!(
                            "Recorded latency for {}: {:.2}ms (pingCount={})",
                            peer_id, latency_ms, peer.ping_count
                        );
                    } else {
                        warn!("No peer entry found for {} to record latency", peer_id);
                    }
                } else {
                    warn!("Received Pong from {} but no pending ping found", peer_id);
                }
            }
            PeerMessage::Ping {
                from_id,
                from_region,
                ..
            } => {
                // This would be handled by incoming peer connections (server-side)
                debug!("Received ping from {} ({})", from_id, from_region);
            }
            PeerMessage::Identify {
                id,
                region,
                version,
            } => {
                info!("Peer {} identified: {} v{}", id, region, version);
            }
            PeerMessage::StatusBroadcast { peers } => {
                debug!("Received status broadcast with {} peers", peers.len());
            }
            PeerMessage::Announce {
                id,
                region,
                ws_url,
                api_url,
                signature,
                timestamp,
            } => {
                self.handle_announce(id, region, ws_url, api_url, signature, timestamp);
            }
            PeerMessage::SharePeers { peers } => {
                self.handle_share_peers(peers, peer_id);
            }
            PeerMessage::RequestPeers => {
                debug!("Received peer request from {}", peer_id);
                // Response will be sent via the write channel
            }
            PeerMessage::SyncData { from_id, data } => {
                debug!("Received sync data from {} ({} bytes)", from_id, data.len());
                // TODO: Forward to SyncService for processing
                // For now, just log - actual handling will be implemented when SyncService is fully integrated
            }
        }
    }
}

// NOTE: PeerMesh should always be used via Arc<PeerMesh>.
// Do NOT implement Clone for PeerMesh as it would create a separate instance
// with empty peer/pending_pings maps, breaking shared state.
