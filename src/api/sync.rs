//! Sync API endpoints for monitoring distributed data synchronization.

use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Create sync API router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(get_sync_health))
        .route("/mesh-status", get(get_sync_mesh_status))
        .route("/metrics", get(get_node_metrics))
        .route("/queue", get(get_sync_queue))
        // Phase 3: Historical sync endpoints
        .route("/historical/sessions", get(get_historical_sessions))
        .route("/historical/sessions/{session_id}", get(get_session_progress))
        .route("/historical/request", post(request_historical_sync))
        .route("/historical/sessions/{session_id}/pause", post(pause_session))
        .route("/historical/sessions/{session_id}/resume", post(resume_session))
        .route("/historical/sessions/{session_id}/cancel", post(cancel_session))
        .route("/historical/progress/{session_id}", get(get_progress_history))
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
    pub sync_cursor_position: u64,
    pub pending_sync_count: u32,
    pub failed_sync_count: u32,
    pub total_synced_entities: u64,
    pub database_size_mb: f64,
    pub database_row_count: u32,
}

async fn build_local_sync_health(state: &AppState) -> Result<SyncHealthResponse, (StatusCode, String)> {
    let sync_state = state
        .sqlite_store
        .get_sync_state()
        .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get sync state".to_string()))?;

    let (db_size_mb, db_row_count) = state
        .sqlite_store
        .get_database_stats()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to get database stats: {}", e)))?;

    let is_primary = state.config.server_id == "osaka";

    Ok(SyncHealthResponse {
        node_id: state.config.server_id.clone(),
        is_primary,
        sync_enabled: sync_state.sync_enabled,
        last_full_sync_at: sync_state.last_full_sync_at,
        last_incremental_sync_at: sync_state.last_incremental_sync_at,
        sync_cursor_position: sync_state.sync_cursor_position,
        pending_sync_count: sync_state.pending_sync_count,
        failed_sync_count: sync_state.failed_sync_count,
        total_synced_entities: sync_state.total_synced_entities,
        database_size_mb: db_size_mb,
        database_row_count: db_row_count,
    })
}

/// Get sync health status.
pub async fn get_sync_health(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    Ok(Json(build_local_sync_health(&state).await?))
}

/// A single node's sync health as observed by a mesh status request.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshNodeSyncHealth {
    pub node_id: String,
    pub region: String,
    pub api_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<SyncHealthResponse>,
}

/// Mesh sync status response.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncMeshStatusResponse {
    pub server_id: String,
    pub timestamp: i64,
    pub nodes: Vec<MeshNodeSyncHealth>,
}

/// Get sync health across this node and its configured peers.
pub async fn get_sync_mesh_status(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let client = Client::builder()
        .timeout(Duration::from_millis(800))
        .build()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create HTTP client: {}", e)))?;

    let local_health = build_local_sync_health(&state).await?;
    let mut nodes: Vec<MeshNodeSyncHealth> = Vec::new();

    nodes.push(MeshNodeSyncHealth {
        node_id: state.config.server_id.clone(),
        region: state.config.server_region.clone(),
        api_url: state.config.public_api_url.clone(),
        health: Some(local_health),
    });

    for peer in &state.config.peer_servers {
        let url = format!("{}/api/sync/health", peer.api_url.trim_end_matches('/'));
        let health = match client.get(url).send().await {
            Ok(resp) => resp.json::<SyncHealthResponse>().await.ok(),
            Err(_) => None,
        };

        nodes.push(MeshNodeSyncHealth {
            node_id: peer.id.clone(),
            region: peer.region.clone(),
            api_url: peer.api_url.clone(),
            health,
        });
    }

    Ok(Json(SyncMeshStatusResponse {
        server_id: state.config.server_id.clone(),
        timestamp: chrono::Utc::now().timestamp_millis(),
        nodes,
    }))
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

// ========== Phase 3: Historical Sync Endpoints ==========

/// Historical sync session info.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalSessionInfo {
    pub id: String,
    pub source_node_id: String,
    pub target_node_id: String,
    pub status: String,
    pub total_entities: u64,
    pub synced_entities: u64,
    pub failed_entities: u64,
    pub progress_percent: f32,
    pub current_entity_type: Option<String>,
    pub current_batch: u32,
    pub total_batches: u32,
    pub bytes_transferred: u64,
    pub compression_savings_bytes: u64,
    pub started_at: i64,
    pub updated_at: i64,
    pub completed_at: Option<i64>,
    pub error: Option<String>,
}

/// Historical sessions list response.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalSessionsResponse {
    pub sessions: Vec<HistoricalSessionInfo>,
    pub total: usize,
}

/// Get all historical sync sessions.
pub async fn get_historical_sessions(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let sessions = state.sqlite_store.get_active_sync_sessions()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to get sessions: {}", e)))?;

    let session_infos: Vec<HistoricalSessionInfo> = sessions
        .iter()
        .map(|s| HistoricalSessionInfo {
            id: s.id.clone(),
            source_node_id: s.source_node_id.clone(),
            target_node_id: s.target_node_id.clone(),
            status: format!("{:?}", s.status).to_lowercase(),
            total_entities: s.total_entities,
            synced_entities: s.synced_entities,
            failed_entities: s.failed_entities,
            progress_percent: s.progress_percent,
            current_entity_type: s.current_entity_type.map(|et| format!("{:?}", et).to_lowercase()),
            current_batch: s.current_batch,
            total_batches: s.total_batches,
            bytes_transferred: s.bytes_transferred,
            compression_savings_bytes: s.compression_savings_bytes,
            started_at: s.started_at,
            updated_at: s.updated_at,
            completed_at: s.completed_at,
            error: s.error.clone(),
        })
        .collect();

    let total = session_infos.len();

    Ok(Json(HistoricalSessionsResponse { sessions: session_infos, total }))
}

/// Get progress for a specific session.
pub async fn get_session_progress(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let session = state.sqlite_store.get_historical_sync_session(&session_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    let info = HistoricalSessionInfo {
        id: session.id,
        source_node_id: session.source_node_id,
        target_node_id: session.target_node_id,
        status: format!("{:?}", session.status).to_lowercase(),
        total_entities: session.total_entities,
        synced_entities: session.synced_entities,
        failed_entities: session.failed_entities,
        progress_percent: session.progress_percent,
        current_entity_type: session.current_entity_type.map(|et| format!("{:?}", et).to_lowercase()),
        current_batch: session.current_batch,
        total_batches: session.total_batches,
        bytes_transferred: session.bytes_transferred,
        compression_savings_bytes: session.compression_savings_bytes,
        started_at: session.started_at,
        updated_at: session.updated_at,
        completed_at: session.completed_at,
        error: session.error,
    };

    Ok(Json(info))
}

/// Request for historical sync.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalSyncRequest {
    pub entity_types: Option<Vec<String>>,
    pub batch_size: Option<u32>,
}

/// Response for historical sync request.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalSyncRequestResponse {
    pub session_id: String,
    pub status: String,
    pub message: String,
}

/// Request a new historical sync.
pub async fn request_historical_sync(
    State(state): State<AppState>,
    Json(request): Json<HistoricalSyncRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let entity_types: Vec<crate::types::EntityType> = request.entity_types
        .unwrap_or_default()
        .iter()
        .filter_map(|s| parse_entity_type_str(s))
        .collect();

    let batch_size = request.batch_size.unwrap_or(500).clamp(100, 1000);

    let sync_service = state.sync_service.as_ref()
        .ok_or_else(|| (StatusCode::SERVICE_UNAVAILABLE, "Sync service not available".to_string()))?;

    let session_id = sync_service.request_historical_sync(entity_types, batch_size).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to request sync: {}", e)))?;

    Ok(Json(HistoricalSyncRequestResponse {
        session_id,
        status: "pending".to_string(),
        message: "Historical sync request sent to primary node".to_string(),
    }))
}

/// Pause a historical sync session.
pub async fn pause_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let sync_service = state.sync_service.as_ref()
        .ok_or_else(|| (StatusCode::SERVICE_UNAVAILABLE, "Sync service not available".to_string()))?;

    sync_service.pause_historical_sync(&session_id).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to pause: {}", e)))?;

    Ok(Json(serde_json::json!({
        "sessionId": session_id,
        "status": "paused"
    })))
}

/// Resume a paused historical sync session.
pub async fn resume_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let sync_service = state.sync_service.as_ref()
        .ok_or_else(|| (StatusCode::SERVICE_UNAVAILABLE, "Sync service not available".to_string()))?;

    sync_service.resume_historical_sync(&session_id).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to resume: {}", e)))?;

    Ok(Json(serde_json::json!({
        "sessionId": session_id,
        "status": "resuming"
    })))
}

/// Cancel a historical sync session.
pub async fn cancel_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let sync_service = state.sync_service.as_ref()
        .ok_or_else(|| (StatusCode::SERVICE_UNAVAILABLE, "Sync service not available".to_string()))?;

    sync_service.cancel_historical_sync(&session_id).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to cancel: {}", e)))?;

    Ok(Json(serde_json::json!({
        "sessionId": session_id,
        "status": "cancelled"
    })))
}

/// Progress history query params.
#[derive(Debug, Deserialize)]
pub struct ProgressHistoryQuery {
    pub limit: Option<usize>,
}

/// Progress history response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressHistoryResponse {
    pub session_id: String,
    pub progress: Vec<crate::types::SyncProgressUpdate>,
    pub total: usize,
}

/// Get progress history for a session.
pub async fn get_progress_history(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Query(query): Query<ProgressHistoryQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let limit = query.limit.unwrap_or(100).clamp(1, 1000);

    let progress = state.sqlite_store.get_sync_progress_history(&session_id, limit)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to get progress: {}", e)))?;

    let total = progress.len();

    Ok(Json(ProgressHistoryResponse {
        session_id,
        progress,
        total,
    }))
}

/// Parse entity type from string.
fn parse_entity_type_str(s: &str) -> Option<crate::types::EntityType> {
    match s.to_lowercase().as_str() {
        "profile" => Some(crate::types::EntityType::Profile),
        "portfolio" => Some(crate::types::EntityType::Portfolio),
        "order" => Some(crate::types::EntityType::Order),
        "position" => Some(crate::types::EntityType::Position),
        "trade" => Some(crate::types::EntityType::Trade),
        "options_position" | "optionsposition" => Some(crate::types::EntityType::OptionsPosition),
        "strategy" => Some(crate::types::EntityType::Strategy),
        "funding_payment" | "fundingpayment" => Some(crate::types::EntityType::FundingPayment),
        "liquidation" => Some(crate::types::EntityType::Liquidation),
        "margin_history" | "marginhistory" => Some(crate::types::EntityType::MarginHistory),
        "portfolio_snapshot" | "portfoliosnapshot" => Some(crate::types::EntityType::PortfolioSnapshot),
        "insurance_fund" | "insurancefund" => Some(crate::types::EntityType::InsuranceFund),
        "prediction_history" | "predictionhistory" => Some(crate::types::EntityType::PredictionHistory),
        _ => None,
    }
}
