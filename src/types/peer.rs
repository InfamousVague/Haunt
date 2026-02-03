//! Peer mesh types for multi-server connectivity.

use serde::{Deserialize, Serialize};

/// Peer server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerConfig {
    /// Unique server ID.
    pub id: String,
    /// Server region/location name.
    pub region: String,
    /// WebSocket URL for peer connection.
    pub ws_url: String,
    /// HTTP API URL for health checks.
    pub api_url: String,
}

/// Current status of a peer connection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PeerConnectionStatus {
    Connected,
    Connecting,
    Disconnected,
    Failed,
}

/// Lightweight peer info for gossip protocol sharing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo {
    /// Unique server ID.
    pub id: String,
    /// Server region/location name.
    pub region: String,
    /// WebSocket URL for peer connection.
    pub ws_url: String,
    /// HTTP API URL for health checks.
    pub api_url: String,
    /// Last seen timestamp (milliseconds since epoch).
    pub last_seen: i64,
    /// Current connection status as observed by the sharing peer.
    pub status: PeerConnectionStatus,
    /// Optional latency observed by the sharing peer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<f64>,
}

/// Real-time peer status with latency information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerStatus {
    /// Peer server ID.
    pub id: String,
    /// Peer server region.
    pub region: String,
    /// Connection status.
    pub status: PeerConnectionStatus,
    /// Current latency in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<f64>,
    /// Average latency over last 60 seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_latency_ms: Option<f64>,
    /// Minimum latency observed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_latency_ms: Option<f64>,
    /// Maximum latency observed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_latency_ms: Option<f64>,
    /// Number of successful pings.
    pub ping_count: u64,
    /// Number of failed pings.
    pub failed_pings: u64,
    /// Uptime percentage (0-100).
    pub uptime_percent: f64,
    /// Last successful ping timestamp (milliseconds since epoch).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_ping_at: Option<i64>,
    /// Last connection attempt timestamp (milliseconds since epoch).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_attempt_at: Option<i64>,
}

/// Message sent between peer servers for ping/pong.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PeerMessage {
    /// Authentication request with HMAC signature.
    Auth {
        id: String,
        region: String,
        timestamp: i64,
        /// HMAC-SHA256 signature of "id:region:timestamp" using shared key.
        signature: String,
    },
    /// Authentication response.
    AuthResponse {
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// Ping request with timestamp.
    Ping {
        from_id: String,
        from_region: String,
        timestamp: i64,
    },
    /// Pong response.
    Pong {
        from_id: String,
        from_region: String,
        original_timestamp: i64,
    },
    /// Server identification on connect (for backwards compatibility).
    Identify {
        id: String,
        region: String,
        version: String,
    },
    /// Peer status broadcast.
    StatusBroadcast {
        peers: Vec<PeerStatus>,
    },
    /// Server announces itself to the mesh (gossip protocol).
    Announce {
        /// Server ID.
        id: String,
        /// Server region.
        region: String,
        /// WebSocket URL for peer connections.
        ws_url: String,
        /// HTTP API URL.
        api_url: String,
        /// HMAC-SHA256 signature of "announce:id:region:timestamp" for verification.
        signature: String,
        /// Timestamp when announcement was created.
        timestamp: i64,
    },
    /// Share known peers with another server (gossip protocol).
    SharePeers {
        /// List of known peers.
        peers: Vec<PeerInfo>,
    },
    /// Request peer list from a connected server (gossip protocol).
    RequestPeers,
}
