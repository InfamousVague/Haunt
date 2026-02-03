//! Sync API endpoints for cross-server preference synchronization.

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::services::peer_mesh::{MeshMessage, PeerInfo};
use crate::types::ProfileSettings;
use crate::AppState;

/// Create the sync router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/status", get(sync_status))
        .route("/preferences", get(get_preferences).post(update_preferences))
        .route("/mesh", post(handle_mesh_message))
        .route("/peers", get(get_peers))
}

/// Sync status response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatusResponse {
    pub server_id: String,
    pub server_region: String,
    pub connected_peers: usize,
    pub total_peers: usize,
    pub peers: Vec<PeerInfo>,
}

/// Get sync status.
async fn sync_status(State(state): State<AppState>) -> impl IntoResponse {
    let peers = state.peer_mesh.get_peers();
    let connected = peers.iter().filter(|p| p.connected).count();

    Json(SyncStatusResponse {
        server_id: state.peer_mesh.server_id().to_string(),
        server_region: state.peer_mesh.server_region().to_string(),
        connected_peers: connected,
        total_peers: peers.len(),
        peers,
    })
}

/// Get preferences request.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPreferencesQuery {
    pub public_key: String,
}

/// Get preferences response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferencesResponse {
    pub public_key: String,
    pub settings: ProfileSettings,
    pub synced_from: String,
}

/// Get user preferences (will sync from peers if not found locally).
async fn get_preferences(
    State(state): State<AppState>,
    Query(query): Query<GetPreferencesQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    // Try to get from local storage first
    if let Some(profile) = state.auth_service.get_profile(&query.public_key).await {
        return Ok(Json(PreferencesResponse {
            public_key: query.public_key,
            settings: profile.settings,
            synced_from: state.peer_mesh.server_id().to_string(),
        }));
    }

    // Try to sync from peers
    if let Some(profile) = state.peer_mesh.request_profile(&query.public_key).await {
        // Save locally
        let _ = state.auth_service.update_profile(profile.clone()).await;

        return Ok(Json(PreferencesResponse {
            public_key: query.public_key,
            settings: profile.settings,
            synced_from: "peer".to_string(),
        }));
    }

    Err(StatusCode::NOT_FOUND)
}

/// Update preferences request.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePreferencesRequest {
    pub public_key: String,
    pub settings: ProfileSettings,
}

/// Update user preferences and sync to peers.
async fn update_preferences(
    State(state): State<AppState>,
    Json(request): Json<UpdatePreferencesRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    // Get or create profile
    let mut profile = state
        .auth_service
        .get_profile(&request.public_key)
        .await
        .unwrap_or_else(|| crate::types::Profile::new(request.public_key.clone()));

    // Update settings with new timestamp
    let mut settings = request.settings;
    settings.updated_at = chrono::Utc::now().timestamp_millis();
    profile.settings = settings;

    // Save locally
    state
        .auth_service
        .update_profile(profile.clone())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Sync to peers (fire and forget)
    let peer_mesh = state.peer_mesh.clone();
    let profile_clone = profile.clone();
    tokio::spawn(async move {
        peer_mesh.sync_profile(&profile_clone).await;
    });

    Ok(Json(PreferencesResponse {
        public_key: profile.public_key,
        settings: profile.settings,
        synced_from: state.peer_mesh.server_id().to_string(),
    }))
}

/// Handle incoming mesh message from peer.
async fn handle_mesh_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(message): Json<MeshMessage>,
) -> Result<impl IntoResponse, StatusCode> {
    // Verify mesh key if configured
    if let Some(ref expected_key) = state.config.mesh_shared_key {
        let provided_key = headers
            .get("X-Mesh-Key")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if provided_key != expected_key {
            warn!("Invalid mesh key from peer");
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    let origin = headers
        .get("X-Origin-Server")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    debug!("Received mesh message from {}: {:?}", origin, message);

    // Handle the message
    if let Some(response) = state
        .peer_mesh
        .handle_message(message, &state.auth_service)
        .await
    {
        Ok(Json(response))
    } else {
        Ok(Json(MeshMessage::Pong {
            server_id: state.peer_mesh.server_id().to_string(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            original_timestamp: 0,
        }))
    }
}

/// Get list of peers.
async fn get_peers(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.peer_mesh.get_peers())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preferences_response_serialization() {
        let response = PreferencesResponse {
            public_key: "abc123".to_string(),
            settings: ProfileSettings::default(),
            synced_from: "osaka".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"publicKey\":\"abc123\""));
        assert!(json.contains("\"syncedFrom\":\"osaka\""));
    }
}
