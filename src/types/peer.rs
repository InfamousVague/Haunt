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
    StatusBroadcast { peers: Vec<PeerStatus> },
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
    /// Sync data message (distributed data synchronization).
    SyncData {
        /// Source node ID.
        from_id: String,
        /// Serialized sync message (JSON).
        data: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // PeerConfig Tests
    // =========================================================================

    #[test]
    fn test_peer_config_creation() {
        let config = PeerConfig {
            id: "osaka".to_string(),
            region: "Asia Pacific".to_string(),
            ws_url: "wss://osaka.example.com/ws".to_string(),
            api_url: "https://osaka.example.com".to_string(),
        };

        assert_eq!(config.id, "osaka");
        assert_eq!(config.region, "Asia Pacific");
    }

    #[test]
    fn test_peer_config_serialization() {
        let config = PeerConfig {
            id: "test".to_string(),
            region: "Test Region".to_string(),
            ws_url: "wss://test.com/ws".to_string(),
            api_url: "https://test.com".to_string(),
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"id\":\"test\""));

        let parsed: PeerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, config.id);
    }

    // =========================================================================
    // PeerConnectionStatus Tests
    // =========================================================================

    #[test]
    fn test_peer_connection_status_serialization() {
        let connected = PeerConnectionStatus::Connected;
        let connecting = PeerConnectionStatus::Connecting;
        let disconnected = PeerConnectionStatus::Disconnected;
        let failed = PeerConnectionStatus::Failed;

        let connected_json = serde_json::to_string(&connected).unwrap();
        let connecting_json = serde_json::to_string(&connecting).unwrap();
        let disconnected_json = serde_json::to_string(&disconnected).unwrap();
        let failed_json = serde_json::to_string(&failed).unwrap();

        assert_eq!(connected_json, "\"connected\"");
        assert_eq!(connecting_json, "\"connecting\"");
        assert_eq!(disconnected_json, "\"disconnected\"");
        assert_eq!(failed_json, "\"failed\"");
    }

    #[test]
    fn test_peer_connection_status_equality() {
        assert_eq!(
            PeerConnectionStatus::Connected,
            PeerConnectionStatus::Connected
        );
        assert_ne!(
            PeerConnectionStatus::Connected,
            PeerConnectionStatus::Disconnected
        );
    }

    // =========================================================================
    // PeerInfo Tests
    // =========================================================================

    #[test]
    fn test_peer_info_creation() {
        let info = PeerInfo {
            id: "tokyo".to_string(),
            region: "Asia Pacific".to_string(),
            ws_url: "wss://tokyo.example.com/ws".to_string(),
            api_url: "https://tokyo.example.com".to_string(),
            last_seen: 1704067200000,
            status: PeerConnectionStatus::Connected,
            latency_ms: Some(45.5),
        };

        assert_eq!(info.id, "tokyo");
        assert_eq!(info.status, PeerConnectionStatus::Connected);
        assert_eq!(info.latency_ms, Some(45.5));
    }

    #[test]
    fn test_peer_info_optional_latency() {
        let info = PeerInfo {
            id: "unknown".to_string(),
            region: "Unknown".to_string(),
            ws_url: "wss://unknown.com/ws".to_string(),
            api_url: "https://unknown.com".to_string(),
            last_seen: 0,
            status: PeerConnectionStatus::Disconnected,
            latency_ms: None,
        };

        assert!(info.latency_ms.is_none());
    }

    // =========================================================================
    // PeerStatus Tests
    // =========================================================================

    #[test]
    fn test_peer_status_creation() {
        let status = PeerStatus {
            id: "sydney".to_string(),
            region: "Australia".to_string(),
            status: PeerConnectionStatus::Connected,
            latency_ms: Some(150.0),
            avg_latency_ms: Some(145.0),
            min_latency_ms: Some(120.0),
            max_latency_ms: Some(180.0),
            ping_count: 1000,
            failed_pings: 5,
            uptime_percent: 99.5,
            last_ping_at: Some(1704067200000),
            last_attempt_at: Some(1704067190000),
        };

        assert_eq!(status.id, "sydney");
        assert_eq!(status.ping_count, 1000);
        assert!(status.uptime_percent > 99.0);
    }

    #[test]
    fn test_peer_status_uptime_calculation() {
        let status = PeerStatus {
            id: "test".to_string(),
            region: "Test".to_string(),
            status: PeerConnectionStatus::Connected,
            latency_ms: None,
            avg_latency_ms: None,
            min_latency_ms: None,
            max_latency_ms: None,
            ping_count: 95,
            failed_pings: 5,
            uptime_percent: 95.0,
            last_ping_at: None,
            last_attempt_at: None,
        };

        assert_eq!(status.uptime_percent, 95.0);
    }

    // =========================================================================
    // PeerMessage Tests
    // =========================================================================

    #[test]
    fn test_peer_message_ping_serialization() {
        let ping = PeerMessage::Ping {
            from_id: "local".to_string(),
            from_region: "Local".to_string(),
            timestamp: 1704067200000,
        };

        let json = serde_json::to_string(&ping).unwrap();
        assert!(json.contains("\"type\":\"ping\""));
        assert!(json.contains("\"from_id\":\"local\""));

        let parsed: PeerMessage = serde_json::from_str(&json).unwrap();
        if let PeerMessage::Ping { from_id, .. } = parsed {
            assert_eq!(from_id, "local");
        } else {
            panic!("Expected Ping message");
        }
    }

    #[test]
    fn test_peer_message_pong_serialization() {
        let pong = PeerMessage::Pong {
            from_id: "remote".to_string(),
            from_region: "Remote".to_string(),
            original_timestamp: 1704067200000,
        };

        let json = serde_json::to_string(&pong).unwrap();
        assert!(json.contains("\"type\":\"pong\""));
    }

    #[test]
    fn test_peer_message_identify_serialization() {
        let identify = PeerMessage::Identify {
            id: "server1".to_string(),
            region: "US East".to_string(),
            version: "1.0.0".to_string(),
        };

        let json = serde_json::to_string(&identify).unwrap();
        assert!(json.contains("\"type\":\"identify\""));
        assert!(json.contains("\"version\":\"1.0.0\""));
    }

    #[test]
    fn test_peer_message_auth_serialization() {
        let auth = PeerMessage::Auth {
            id: "server".to_string(),
            region: "Region".to_string(),
            timestamp: 1704067200000,
            signature: "abc123signature".to_string(),
        };

        let json = serde_json::to_string(&auth).unwrap();
        assert!(json.contains("\"type\":\"auth\""));
        assert!(json.contains("\"signature\""));
    }

    #[test]
    fn test_peer_message_request_peers_serialization() {
        let request = PeerMessage::RequestPeers;

        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(json, "{\"type\":\"request_peers\"}");

        let parsed: PeerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, PeerMessage::RequestPeers));
    }
}
