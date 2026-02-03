//! Peer mesh API endpoints for server connectivity and ping status.

use axum::{
    extract::State,
    routing::get,
    Json, Router,
};
use serde::Serialize;

use crate::services::PeerStatus;
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
        (mesh.server_id().to_string(), mesh.server_region().to_string())
    } else {
        (state.config.server_id.clone(), state.config.server_region.clone())
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

/// Create the peers router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_peers))
        .route("/:peer_id", get(get_peer))
}
