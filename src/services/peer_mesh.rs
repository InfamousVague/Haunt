//! Peer mesh service for multi-server connectivity and real-time ping monitoring.
//!
//! This module manages WebSocket connections between Haunt API servers,
//! providing real-time latency tracking and server health monitoring.

use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, RwLock};
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info};

// Re-export types from the types module
pub use crate::types::{PeerConfig, PeerConnectionStatus, PeerMessage, PeerStatus};

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
            let min = self.latency_history.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = self.latency_history.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
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

/// Manages peer mesh connections and real-time ping monitoring.
pub struct PeerMesh {
    /// This server's ID.
    server_id: String,
    /// This server's region.
    server_region: String,
    /// Connected peers.
    peers: DashMap<String, RwLock<PeerConnection>>,
    /// Broadcast channel for peer status updates.
    status_tx: broadcast::Sender<Vec<PeerStatus>>,
    /// Pending ping timestamps for latency calculation.
    pending_pings: DashMap<String, Instant>,
}

impl PeerMesh {
    /// Create a new peer mesh manager.
    pub fn new(server_id: String, server_region: String) -> Arc<Self> {
        let (status_tx, _) = broadcast::channel(256);

        Arc::new(Self {
            server_id,
            server_region,
            peers: DashMap::new(),
            status_tx,
            pending_pings: DashMap::new(),
        })
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
        self.peers.insert(peer_id, RwLock::new(PeerConnection::new(config)));
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
        self.peers.get(peer_id).and_then(|entry| {
            entry.try_read().ok().map(|peer| peer.get_status())
        })
    }

    /// Broadcast current peer statuses to all subscribers.
    pub fn broadcast_statuses(&self) {
        let statuses = self.get_all_statuses();
        let _ = self.status_tx.send(statuses);
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
                    self.handle_peer_connection(&peer_id, ws).await;
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
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, Box<dyn std::error::Error + Send + Sync>> {
        let (ws, _) = connect_async(&config.ws_url).await?;
        Ok(ws)
    }

    /// Handle an active peer connection.
    async fn handle_peer_connection(
        &self,
        peer_id: &str,
        ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    ) {
        let (mut write, mut read) = ws.split();

        // Send identification
        let identify_msg = PeerMessage::Identify {
            id: self.server_id.clone(),
            region: self.server_region.clone(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        };

        if let Ok(json) = serde_json::to_string(&identify_msg) {
            if let Err(e) = write.send(tokio_tungstenite::tungstenite::Message::Text(json)).await {
                error!("Failed to send identify to {}: {}", peer_id, e);
                return;
            }
        }

        // Spawn ping task (every 1 second for real-time latency)
        let mesh = Arc::new(self.clone());
        let ping_peer_id = peer_id.to_string();
        let ping_mesh = mesh.clone();

        let (ping_tx, mut ping_rx) = tokio::sync::mpsc::channel::<String>(32);

        let ping_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
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
                    ping_mesh.pending_pings.insert(ping_peer_id.clone(), Instant::now());

                    if ping_tx.send(json).await.is_err() {
                        break;
                    }
                }
            }
        });

        // Spawn write task
        let write_task = tokio::spawn(async move {
            while let Some(msg) = ping_rx.recv().await {
                if write.send(tokio_tungstenite::tungstenite::Message::Text(msg)).await.is_err() {
                    break;
                }
            }
        });

        // Handle incoming messages
        while let Some(result) = read.next().await {
            match result {
                Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                    if let Ok(msg) = serde_json::from_str::<PeerMessage>(&text) {
                        self.handle_peer_message(peer_id, msg).await;
                    }
                }
                Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => {
                    debug!("Peer {} closed connection", peer_id);
                    break;
                }
                Err(e) => {
                    error!("WebSocket error from peer {}: {}", peer_id, e);
                    break;
                }
                _ => {}
            }
        }

        ping_task.abort();
        write_task.abort();
    }

    /// Handle an incoming peer message.
    async fn handle_peer_message(&self, peer_id: &str, msg: PeerMessage) {
        match msg {
            PeerMessage::Pong { .. } => {
                // Calculate latency
                if let Some((_, sent_at)) = self.pending_pings.remove(peer_id) {
                    let latency_ms = sent_at.elapsed().as_secs_f64() * 1000.0;

                    // Record latency
                    if let Some(entry) = self.peers.get(peer_id) {
                        let mut peer = entry.write().await;
                        peer.record_latency(latency_ms);
                    }

                    debug!("Peer {} latency: {:.2}ms", peer_id, latency_ms);
                }
            }
            PeerMessage::Ping { from_id, from_region, .. } => {
                // This would be handled by incoming peer connections (server-side)
                debug!("Received ping from {} ({})", from_id, from_region);
            }
            PeerMessage::Identify { id, region, version } => {
                info!("Peer {} identified: {} v{}", id, region, version);
            }
            PeerMessage::StatusBroadcast { peers } => {
                debug!("Received status broadcast with {} peers", peers.len());
            }
        }
    }
}

// Implement Clone for Arc reference
impl Clone for PeerMesh {
    fn clone(&self) -> Self {
        Self {
            server_id: self.server_id.clone(),
            server_region: self.server_region.clone(),
            peers: DashMap::new(),
            status_tx: self.status_tx.clone(),
            pending_pings: DashMap::new(),
        }
    }
}
