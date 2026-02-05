//! Sync API endpoints for monitoring distributed data synchronization.

use crate::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

/// Create sync API router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(get_sync_health))
        .route("/metrics", get(get_node_metrics))
        .route("/queue", get(get_sync_queue))
}

/// Sync health response.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncHealthResponse {
    pub node_id: String,
    pub is_primary: bool,
    pub sync_enabled: bool,
    pub last_full_sync_at: i64,
    pub last_incremental_sync_at: i64,
    pub pending_sync_count: u32,
    pub failed_sync_count: u32,
    pub total_synced_entities: u64,
    pub database_size_mb: f64,
    pub database_row_count: u32,
}

/// Get sync health status.
pub async fn get_sync_health(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Get sync state from database
    let sync_state = state
        .sqlite_store
        .get_sync_state()
        .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get sync state".to_string()))?;

    // Get database stats
    let (db_size_mb, db_row_count) = state
        .sqlite_store
        .get_database_stats()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to get database stats: {}", e)))?;

    // Determine if this is the primary node
    let is_primary = state.config.server_id == "osaka";

    let response = SyncHealthResponse {
        node_id: state.config.server_id.clone(),
        is_primary,
        sync_enabled: sync_state.sync_enabled,
        last_full_sync_at: sync_state.last_full_sync_at,
        last_incremental_sync_at: sync_state.last_incremental_sync_at,
        pending_sync_count: sync_state.pending_sync_count,
        failed_sync_count: sync_state.failed_sync_count,
        total_synced_entities: sync_state.total_synced_entities,
        database_size_mb: db_size_mb,
        database_row_count: db_row_count,
    };

    Ok(Json(response))
}

/// Node metrics response.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeMetricsResponse {
    pub metrics: Vec<crate::types::NodeMetrics>,
    pub total: usize,
}

/// Get recent node metrics.
pub async fn get_node_metrics(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Get last 60 metrics (last 10 minutes at 10-second intervals)
    let metrics = state
        .sqlite_store
        .get_node_metrics(&state.config.server_id, 60)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to get metrics: {}", e)))?;

    let total = metrics.len();

    Ok(Json(NodeMetricsResponse { metrics, total }))
}

/// Sync queue status response.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncQueueResponse {
    pub pending_items: Vec<SyncQueueItemInfo>,
    pub total_pending: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncQueueItemInfo {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub operation: String,
    pub priority: u8,
    pub retry_count: u32,
    pub scheduled_at: i64,
    pub error: Option<String>,
}

/// Get sync queue status.
pub async fn get_sync_queue(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let items = state
        .sqlite_store
        .get_pending_sync_items(50)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to get sync queue: {}", e)))?;

    let pending_items: Vec<SyncQueueItemInfo> = items
        .iter()
        .map(|item| SyncQueueItemInfo {
            id: item.id.clone(),
            entity_type: format!("{:?}", item.entity_type),
            entity_id: item.entity_id.clone(),
            operation: format!("{:?}", item.operation),
            priority: item.priority,
            retry_count: item.retry_count,
            scheduled_at: item.scheduled_at,
            error: item.error.clone(),
        })
        .collect();

    let total_pending = pending_items.len();

    Ok(Json(SyncQueueResponse {
        pending_items,
        total_pending,
    }))
}
