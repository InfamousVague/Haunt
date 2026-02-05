//! Storage management API endpoints.
//!
//! Provides endpoints for:
//! - Storage metrics and usage statistics
//! - Manual cleanup operations
//! - Retention policy configuration

use crate::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

/// Create storage API router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/metrics", get(get_storage_metrics))
        .route("/config", get(get_storage_config))
        .route("/tables", get(get_table_stats))
        .route("/cleanup", post(run_cleanup))
        .route("/cleanup/aggressive", post(run_aggressive_cleanup))
}

/// Storage metrics response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageMetricsResponse {
    pub total_size_bytes: u64,
    pub total_size_mb: f64,
    pub total_row_count: u64,
    pub limit_mb: u64,
    pub usage_pct: f32,
    pub warning_reached: bool,
    pub critical_reached: bool,
    pub tables: Vec<TableInfo>,
    pub timestamp: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableInfo {
    pub name: String,
    pub row_count: u64,
    pub estimated_size_bytes: u64,
    pub pct_of_total: f32,
}

/// Get current storage metrics.
pub async fn get_storage_metrics(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let storage_manager = state.storage_manager.as_ref()
        .ok_or_else(|| (StatusCode::SERVICE_UNAVAILABLE, "Storage manager not available".to_string()))?;

    let metrics = storage_manager.get_metrics().await;

    let tables: Vec<TableInfo> = metrics.tables
        .iter()
        .map(|t| TableInfo {
            name: t.name.clone(),
            row_count: t.row_count,
            estimated_size_bytes: t.estimated_size_bytes,
            pct_of_total: t.pct_of_total,
        })
        .collect();

    Ok(Json(StorageMetricsResponse {
        total_size_bytes: metrics.total_size_bytes,
        total_size_mb: metrics.total_size_mb,
        total_row_count: metrics.total_row_count,
        limit_mb: metrics.limit_mb,
        usage_pct: metrics.usage_pct,
        warning_reached: metrics.warning_reached,
        critical_reached: metrics.critical_reached,
        tables,
        timestamp: metrics.timestamp,
    }))
}

/// Storage configuration response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageConfigResponse {
    pub limit_mb: u64,
    pub warning_threshold_pct: u8,
    pub critical_threshold_pct: u8,
    pub auto_cleanup_enabled: bool,
    pub retention: RetentionConfigResponse,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetentionConfigResponse {
    pub trades_days: u32,
    pub portfolio_snapshots_days: u32,
    pub prediction_history_days: u32,
    pub sync_queue_days: u32,
    pub node_metrics_days: u32,
    pub funding_payments_days: u32,
    pub margin_history_days: u32,
    pub sync_data_days: u32,
}

/// Get storage configuration.
pub async fn get_storage_config(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let storage_manager = state.storage_manager.as_ref()
        .ok_or_else(|| (StatusCode::SERVICE_UNAVAILABLE, "Storage manager not available".to_string()))?;

    let config = storage_manager.get_config();

    Ok(Json(StorageConfigResponse {
        limit_mb: config.limit_mb,
        warning_threshold_pct: config.warning_threshold_pct,
        critical_threshold_pct: config.critical_threshold_pct,
        auto_cleanup_enabled: config.auto_cleanup_enabled,
        retention: RetentionConfigResponse {
            trades_days: config.retention.trades_days,
            portfolio_snapshots_days: config.retention.portfolio_snapshots_days,
            prediction_history_days: config.retention.prediction_history_days,
            sync_queue_days: config.retention.sync_queue_days,
            node_metrics_days: config.retention.node_metrics_days,
            funding_payments_days: config.retention.funding_payments_days,
            margin_history_days: config.retention.margin_history_days,
            sync_data_days: config.retention.sync_data_days,
        },
    }))
}

/// Table stats response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableStatsResponse {
    pub tables: Vec<TableStatInfo>,
    pub total_rows: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableStatInfo {
    pub name: String,
    pub row_count: u64,
}

/// Get per-table storage statistics.
pub async fn get_table_stats(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let stats = state.sqlite_store.get_detailed_storage_stats()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to get stats: {}", e)))?;

    let total_rows: u64 = stats.iter().map(|(_, count)| count).sum();

    let tables: Vec<TableStatInfo> = stats
        .into_iter()
        .map(|(name, row_count)| TableStatInfo { name, row_count })
        .collect();

    Ok(Json(TableStatsResponse { tables, total_rows }))
}

/// Cleanup result response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResponse {
    pub total_deleted: u64,
    pub bytes_freed_estimated: u64,
    pub duration_ms: i64,
    pub per_table: Vec<TableCleanupInfo>,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableCleanupInfo {
    pub table: String,
    pub rows_deleted: u64,
    pub cutoff_timestamp: i64,
    pub error: Option<String>,
}

/// Run manual cleanup.
pub async fn run_cleanup(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let storage_manager = state.storage_manager.as_ref()
        .ok_or_else(|| (StatusCode::SERVICE_UNAVAILABLE, "Storage manager not available".to_string()))?;

    let result = storage_manager.force_cleanup().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Cleanup failed: {}", e)))?;

    let per_table: Vec<TableCleanupInfo> = result.per_table
        .iter()
        .map(|t| TableCleanupInfo {
            table: t.table.clone(),
            rows_deleted: t.rows_deleted,
            cutoff_timestamp: t.cutoff_timestamp,
            error: t.error.clone(),
        })
        .collect();

    Ok(Json(CleanupResponse {
        total_deleted: result.total_deleted,
        bytes_freed_estimated: result.bytes_freed_estimated,
        duration_ms: result.duration_ms,
        per_table,
        errors: result.errors,
    }))
}

/// Run aggressive cleanup (shorter retention periods).
pub async fn run_aggressive_cleanup(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let storage_manager = state.storage_manager.as_ref()
        .ok_or_else(|| (StatusCode::SERVICE_UNAVAILABLE, "Storage manager not available".to_string()))?;

    let result = storage_manager.force_aggressive_cleanup().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Cleanup failed: {}", e)))?;

    let per_table: Vec<TableCleanupInfo> = result.per_table
        .iter()
        .map(|t| TableCleanupInfo {
            table: t.table.clone(),
            rows_deleted: t.rows_deleted,
            cutoff_timestamp: t.cutoff_timestamp,
            error: t.error.clone(),
        })
        .collect();

    Ok(Json(CleanupResponse {
        total_deleted: result.total_deleted,
        bytes_freed_estimated: result.bytes_freed_estimated,
        duration_ms: result.duration_ms,
        per_table,
        errors: result.errors,
    }))
}
