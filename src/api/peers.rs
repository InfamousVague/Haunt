//! Peer mesh API endpoints for server connectivity and ping status.

use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;

use crate::services::{PeerConnectionStatus, PeerStatus};
use crate::AppState;

/// Response for peer mesh status.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerMeshResponse {
    /// This server's ID.
    pub server_id: String,
    /// This server's region.
    pub server_region: String,
    /// Status of all peer connections.
    pub peers: Vec<PeerStatus>,
    /// Number of connected peers.
    pub connected_count: usize,
    /// Total number of configured peers.
    pub total_peers: usize,
    /// Timestamp of this response.
    pub timestamp: i64,
}

/// Server info for mesh discovery response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshServerInfo {
    /// Server ID.
    pub id: String,
    /// Server region.
    pub region: String,
    /// HTTP API URL.
    pub api_url: String,
    /// WebSocket URL.
    pub ws_url: String,
    /// Current connection status.
    pub status: String,
    /// Latency in milliseconds (if connected).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<f64>,
}

/// Response for mesh server discovery endpoint.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshDiscoveryResponse {
    /// This server's ID.
    pub self_id: String,
    /// This server's region.
    pub self_region: String,
    /// This server's API URL.
    pub self_api_url: String,
    /// This server's WebSocket URL.
    pub self_ws_url: String,
    /// All known servers in the mesh.
    pub servers: Vec<MeshServerInfo>,
    /// Partial hash of mesh key for verification.
    pub mesh_key_hash: String,
    /// Timestamp of this response.
    pub timestamp: i64,
}

/// Get the current peer mesh status.
async fn get_peers(State(state): State<AppState>) -> Json<PeerMeshResponse> {
    let peers = if let Some(ref mesh) = state.peer_mesh {
        mesh.get_all_statuses()
    } else {
        Vec::new()
    };

    let connected_count = peers
        .iter()
        .filter(|p| p.status == crate::services::PeerConnectionStatus::Connected)
        .count();

    let (server_id, server_region) = if let Some(ref mesh) = state.peer_mesh {
        (
            mesh.server_id().to_string(),
            mesh.server_region().to_string(),
        )
    } else {
        (
            state.config.server_id.clone(),
            state.config.server_region.clone(),
        )
    };

    Json(PeerMeshResponse {
        server_id,
        server_region,
        peers: peers.clone(),
        connected_count,
        total_peers: peers.len(),
        timestamp: chrono::Utc::now().timestamp_millis(),
    })
}

/// Get a specific peer's status.
async fn get_peer(
    State(state): State<AppState>,
    axum::extract::Path(peer_id): axum::extract::Path<String>,
) -> Json<Option<PeerStatus>> {
    let status = if let Some(ref mesh) = state.peer_mesh {
        mesh.get_peer_status(&peer_id)
    } else {
        None
    };

    Json(status)
}

/// Get all mesh servers (for frontend discovery).
/// This endpoint allows a frontend to discover all servers by connecting to just one.
async fn get_mesh_servers(State(state): State<AppState>) -> Json<MeshDiscoveryResponse> {
    let timestamp = chrono::Utc::now().timestamp_millis();

    let (self_id, self_region, self_api_url, self_ws_url, mesh_key_hash) =
        if let Some(ref mesh) = state.peer_mesh {
            (
                mesh.server_id().to_string(),
                mesh.server_region().to_string(),
                mesh.api_url().to_string(),
                mesh.ws_url().to_string(),
                // Create a partial hash for verification (first 8 chars of SHA256)
                {
                    use sha2::{Digest, Sha256};
                    let key = state.config.mesh_auth.shared_key.as_str();
                    if key.is_empty() {
                        "none".to_string()
                    } else {
                        let hash = Sha256::digest(key.as_bytes());
                        hex::encode(&hash[..4])
                    }
                },
            )
        } else {
            (
                state.config.server_id.clone(),
                state.config.server_region.clone(),
                format!("http://{}:{}", state.config.host, state.config.port),
                format!("ws://{}:{}/ws", state.config.host, state.config.port),
                "none".to_string(),
            )
        };

    let mut servers: Vec<MeshServerInfo> = Vec::new();

    // Add this server first
    servers.push(MeshServerInfo {
        id: self_id.clone(),
        region: self_region.clone(),
        api_url: self_api_url.clone(),
        ws_url: self_ws_url.clone(),
        status: "online".to_string(),
        latency_ms: Some(0.0),
    });

    // Add connected peers
    if let Some(ref mesh) = state.peer_mesh {
        for peer_status in mesh.get_all_statuses() {
            // Get the peer config for URLs
            let known_peers = mesh.get_known_peers();
            let peer_info = known_peers.iter().find(|p| p.id == peer_status.id);

            let (api_url, ws_url) = if let Some(info) = peer_info {
                (info.api_url.clone(), info.ws_url.clone())
            } else {
                // Fallback: try to get from peer servers config
                let peer_config = state
                    .config
                    .peer_servers
                    .iter()
                    .find(|p| p.id == peer_status.id);
                if let Some(config) = peer_config {
                    (config.api_url.clone(), config.ws_url.clone())
                } else {
                    continue; // Skip if we can't get URLs
                }
            };

            let status = match peer_status.status {
                PeerConnectionStatus::Connected => "online",
                PeerConnectionStatus::Connecting => "connecting",
                PeerConnectionStatus::Disconnected => "offline",
                PeerConnectionStatus::Failed => "offline",
            };

            servers.push(MeshServerInfo {
                id: peer_status.id,
                region: peer_status.region,
                api_url,
                ws_url,
                status: status.to_string(),
                latency_ms: peer_status.latency_ms,
            });
        }
    }

    Json(MeshDiscoveryResponse {
        self_id,
        self_region,
        self_api_url,
        self_ws_url,
        servers,
        mesh_key_hash,
        timestamp,
    })
}

/// Create the peers router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_peers))
        .route("/:peer_id", get(get_peer))
}

/// Create the mesh router (for /api/mesh endpoints).
pub fn mesh_router() -> Router<AppState> {
    Router::new().route("/servers", get(get_mesh_servers))
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // MeshServerInfo Tests
    // =========================================================================

    #[test]
    fn test_mesh_server_info_serialization() {
        let info = MeshServerInfo {
            id: "us-east".to_string(),
            region: "US East".to_string(),
            api_url: "https://us.example.com".to_string(),
            ws_url: "wss://us.example.com/ws".to_string(),
            status: "online".to_string(),
            latency_ms: Some(15.5),
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"id\":\"us-east\""));
        assert!(json.contains("\"region\":\"US East\""));
        assert!(json.contains("\"apiUrl\":\"https://us.example.com\""));
        assert!(json.contains("\"wsUrl\":\"wss://us.example.com/ws\""));
        assert!(json.contains("\"status\":\"online\""));
        assert!(json.contains("\"latencyMs\":15.5"));
    }

    #[test]
    fn test_mesh_server_info_without_latency() {
        let info = MeshServerInfo {
            id: "eu-west".to_string(),
            region: "EU West".to_string(),
            api_url: "https://eu.example.com".to_string(),
            ws_url: "wss://eu.example.com/ws".to_string(),
            status: "offline".to_string(),
            latency_ms: None,
        };

        let json = serde_json::to_string(&info).unwrap();
        // latencyMs should be skipped when None
        assert!(!json.contains("latencyMs"));
        assert!(json.contains("\"status\":\"offline\""));
    }

    #[test]
    fn test_mesh_server_info_debug() {
        let info = MeshServerInfo {
            id: "test".to_string(),
            region: "Test".to_string(),
            api_url: "http://test".to_string(),
            ws_url: "ws://test".to_string(),
            status: "online".to_string(),
            latency_ms: None,
        };

        let debug_str = format!("{:?}", info);
        assert!(debug_str.contains("MeshServerInfo"));
        assert!(debug_str.contains("test"));
    }

    // =========================================================================
    // PeerMeshResponse Tests
    // =========================================================================

    #[test]
    fn test_peer_mesh_response_serialization() {
        let response = PeerMeshResponse {
            server_id: "us-east".to_string(),
            server_region: "US East".to_string(),
            peers: vec![],
            connected_count: 0,
            total_peers: 0,
            timestamp: 1700000000000,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"serverId\":\"us-east\""));
        assert!(json.contains("\"serverRegion\":\"US East\""));
        assert!(json.contains("\"connectedCount\":0"));
        assert!(json.contains("\"totalPeers\":0"));
        assert!(json.contains("\"timestamp\":1700000000000"));
    }

    #[test]
    fn test_peer_mesh_response_with_peers() {
        let response = PeerMeshResponse {
            server_id: "us-east".to_string(),
            server_region: "US East".to_string(),
            peers: vec![
                PeerStatus {
                    id: "eu-west".to_string(),
                    region: "EU West".to_string(),
                    status: PeerConnectionStatus::Connected,
                    latency_ms: Some(50.0),
                    avg_latency_ms: Some(52.5),
                    min_latency_ms: Some(45.0),
                    max_latency_ms: Some(65.0),
                    ping_count: 100,
                    failed_pings: 2,
                    uptime_percent: 98.0,
                    last_ping_at: Some(1700000000000),
                    last_attempt_at: Some(1700000000000),
                },
                PeerStatus {
                    id: "asia".to_string(),
                    region: "Asia Pacific".to_string(),
                    status: PeerConnectionStatus::Disconnected,
                    latency_ms: None,
                    avg_latency_ms: None,
                    min_latency_ms: None,
                    max_latency_ms: None,
                    ping_count: 50,
                    failed_pings: 10,
                    uptime_percent: 83.3,
                    last_ping_at: None,
                    last_attempt_at: Some(1700000000000),
                },
            ],
            connected_count: 1,
            total_peers: 2,
            timestamp: 1700000000000,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"connectedCount\":1"));
        assert!(json.contains("\"totalPeers\":2"));
    }

    #[test]
    fn test_peer_mesh_response_debug() {
        let response = PeerMeshResponse {
            server_id: "test".to_string(),
            server_region: "Test".to_string(),
            peers: vec![],
            connected_count: 0,
            total_peers: 0,
            timestamp: 0,
        };

        let debug_str = format!("{:?}", response);
        assert!(debug_str.contains("PeerMeshResponse"));
    }

    // =========================================================================
    // MeshDiscoveryResponse Tests
    // =========================================================================

    #[test]
    fn test_mesh_discovery_response_serialization() {
        let response = MeshDiscoveryResponse {
            self_id: "us-east".to_string(),
            self_region: "US East".to_string(),
            self_api_url: "https://us.example.com".to_string(),
            self_ws_url: "wss://us.example.com/ws".to_string(),
            servers: vec![],
            mesh_key_hash: "abcd1234".to_string(),
            timestamp: 1700000000000,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"selfId\":\"us-east\""));
        assert!(json.contains("\"selfRegion\":\"US East\""));
        assert!(json.contains("\"selfApiUrl\":\"https://us.example.com\""));
        assert!(json.contains("\"selfWsUrl\":\"wss://us.example.com/ws\""));
        assert!(json.contains("\"meshKeyHash\":\"abcd1234\""));
        assert!(json.contains("\"timestamp\":1700000000000"));
    }

    #[test]
    fn test_mesh_discovery_response_with_servers() {
        let response = MeshDiscoveryResponse {
            self_id: "us-east".to_string(),
            self_region: "US East".to_string(),
            self_api_url: "https://us.example.com".to_string(),
            self_ws_url: "wss://us.example.com/ws".to_string(),
            servers: vec![
                MeshServerInfo {
                    id: "us-east".to_string(),
                    region: "US East".to_string(),
                    api_url: "https://us.example.com".to_string(),
                    ws_url: "wss://us.example.com/ws".to_string(),
                    status: "online".to_string(),
                    latency_ms: Some(0.0),
                },
                MeshServerInfo {
                    id: "eu-west".to_string(),
                    region: "EU West".to_string(),
                    api_url: "https://eu.example.com".to_string(),
                    ws_url: "wss://eu.example.com/ws".to_string(),
                    status: "online".to_string(),
                    latency_ms: Some(85.0),
                },
            ],
            mesh_key_hash: "none".to_string(),
            timestamp: 1700000000000,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"servers\":["));
        assert!(json.contains("\"id\":\"eu-west\""));
    }

    #[test]
    fn test_mesh_discovery_response_debug() {
        let response = MeshDiscoveryResponse {
            self_id: "test".to_string(),
            self_region: "Test".to_string(),
            self_api_url: "http://test".to_string(),
            self_ws_url: "ws://test".to_string(),
            servers: vec![],
            mesh_key_hash: "none".to_string(),
            timestamp: 0,
        };

        let debug_str = format!("{:?}", response);
        assert!(debug_str.contains("MeshDiscoveryResponse"));
    }

    #[test]
    fn test_mesh_discovery_response_no_key() {
        let response = MeshDiscoveryResponse {
            self_id: "solo".to_string(),
            self_region: "Standalone".to_string(),
            self_api_url: "http://localhost:3001".to_string(),
            self_ws_url: "ws://localhost:3001/ws".to_string(),
            servers: vec![],
            mesh_key_hash: "none".to_string(),
            timestamp: 1700000000000,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"meshKeyHash\":\"none\""));
    }
}
