//! Storage management service for disk space monitoring and automatic cleanup.
//!
//! This service provides:
//! - Database size tracking and monitoring
//! - Per-table storage usage metrics
//! - Automatic cleanup of old data based on retention policies
//! - Storage threshold warnings and notifications

use crate::config::StorageConfig;
use crate::services::SqliteStore;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Storage metrics snapshot.
#[derive(Debug, Clone, Default)]
pub struct StorageMetrics {
    /// Total database size in bytes.
    pub total_size_bytes: u64,
    /// Total database size in MB.
    pub total_size_mb: f64,
    /// Total row count across all tables.
    pub total_row_count: u64,
    /// Per-table metrics.
    pub tables: Vec<TableMetrics>,
    /// Storage limit in MB (0 = unlimited).
    pub limit_mb: u64,
    /// Current usage percentage (0-100).
    pub usage_pct: f32,
    /// Warning threshold reached.
    pub warning_reached: bool,
    /// Critical threshold reached.
    pub critical_reached: bool,
    /// Timestamp of metrics collection.
    pub timestamp: i64,
}

/// Per-table storage metrics.
#[derive(Debug, Clone, Default)]
pub struct TableMetrics {
    /// Table name.
    pub name: String,
    /// Row count.
    pub row_count: u64,
    /// Estimated size in bytes.
    pub estimated_size_bytes: u64,
    /// Percentage of total storage.
    pub pct_of_total: f32,
}

/// Cleanup result summary.
#[derive(Debug, Clone, Default)]
pub struct CleanupResult {
    /// Total rows deleted.
    pub total_deleted: u64,
    /// Bytes freed (estimated).
    pub bytes_freed_estimated: u64,
    /// Per-table cleanup results.
    pub per_table: Vec<TableCleanupResult>,
    /// Duration of cleanup in milliseconds.
    pub duration_ms: i64,
    /// Errors encountered.
    pub errors: Vec<String>,
}

/// Per-table cleanup result.
#[derive(Debug, Clone)]
pub struct TableCleanupResult {
    pub table: String,
    pub rows_deleted: u64,
    pub cutoff_timestamp: i64,
    pub error: Option<String>,
}

/// Storage manager service.
pub struct StorageManager {
    /// SQLite store reference.
    sqlite_store: Arc<SqliteStore>,
    /// Storage configuration.
    config: StorageConfig,
    /// Current metrics (cached).
    metrics: RwLock<StorageMetrics>,
    /// Last cleanup timestamp.
    last_cleanup: RwLock<Option<i64>>,
}

impl StorageManager {
    /// Create a new storage manager.
    pub fn new(sqlite_store: Arc<SqliteStore>, config: StorageConfig) -> Arc<Self> {
        Arc::new(Self {
            sqlite_store,
            config,
            metrics: RwLock::new(StorageMetrics::default()),
            last_cleanup: RwLock::new(None),
        })
    }

    /// Start the storage manager background tasks.
    pub fn start(self: Arc<Self>) {
        info!("Starting storage manager (limit: {} MB)", self.config.limit_mb);

        // Spawn metrics collection task (every 5 minutes)
        let manager = self.clone();
        tokio::spawn(async move {
            manager.metrics_collection_loop().await;
        });

        // Spawn automatic cleanup task (every hour)
        let manager = self.clone();
        tokio::spawn(async move {
            manager.auto_cleanup_loop().await;
        });
    }

    /// Metrics collection loop.
    async fn metrics_collection_loop(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes

        loop {
            interval.tick().await;

            if let Err(e) = self.collect_metrics().await {
                error!("Failed to collect storage metrics: {}", e);
            }
        }
    }

    /// Automatic cleanup loop.
    async fn auto_cleanup_loop(&self) {
        // Initial delay to allow startup to complete
        tokio::time::sleep(Duration::from_secs(60)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(3600)); // 1 hour

        loop {
            interval.tick().await;

            if !self.config.auto_cleanup_enabled {
                debug!("Auto cleanup disabled, skipping");
                continue;
            }

            // Check if cleanup is needed
            let metrics = self.metrics.read().await.clone();

            if metrics.critical_reached {
                info!("Critical storage threshold reached ({:.1}%), running cleanup", metrics.usage_pct);
                if let Err(e) = self.run_cleanup(true).await {
                    error!("Automatic cleanup failed: {}", e);
                }
            } else if metrics.warning_reached {
                info!("Warning storage threshold reached ({:.1}%), running light cleanup", metrics.usage_pct);
                // Run lighter cleanup at warning threshold
                if let Err(e) = self.run_cleanup(false).await {
                    error!("Light cleanup failed: {}", e);
                }
            }
        }
    }

    /// Collect current storage metrics.
    pub async fn collect_metrics(&self) -> Result<StorageMetrics, String> {
        let (db_size_mb, total_rows) = self.sqlite_store
            .get_database_stats()
            .map_err(|e| format!("Failed to get database stats: {}", e))?;

        let db_size_bytes = (db_size_mb * 1024.0 * 1024.0) as u64;

        // Get per-table metrics
        let table_names = vec![
            "profiles", "portfolios", "orders", "positions", "trades",
            "options_positions", "strategies", "funding_payments", "liquidations",
            "margin_history", "portfolio_snapshots", "prediction_history",
            "sync_queue", "sync_versions", "sync_conflicts", "node_metrics",
            "historical_sync_sessions", "sync_checkpoints", "sync_progress", "sync_batch_acks",
        ];

        let mut tables = Vec::new();
        for table_name in table_names {
            if let Ok(count) = self.get_table_row_count(table_name) {
                // Rough estimate: average row size varies by table
                let avg_row_size = match table_name {
                    "trades" | "prediction_history" => 500,
                    "portfolio_snapshots" => 1000,
                    "node_metrics" | "sync_progress" => 400,
                    _ => 300,
                };
                let estimated_size = count * avg_row_size;
                let pct_of_total = if db_size_bytes > 0 {
                    (estimated_size as f32 / db_size_bytes as f32) * 100.0
                } else {
                    0.0
                };

                tables.push(TableMetrics {
                    name: table_name.to_string(),
                    row_count: count,
                    estimated_size_bytes: estimated_size,
                    pct_of_total,
                });
            }
        }

        // Sort tables by size
        tables.sort_by(|a, b| b.estimated_size_bytes.cmp(&a.estimated_size_bytes));

        // Calculate usage percentage
        let usage_pct = if self.config.limit_mb > 0 {
            (db_size_mb / self.config.limit_mb as f64 * 100.0) as f32
        } else {
            0.0
        };

        let warning_reached = usage_pct >= self.config.warning_threshold_pct as f32;
        let critical_reached = usage_pct >= self.config.critical_threshold_pct as f32;

        if warning_reached && !critical_reached {
            warn!("Storage warning: {:.1}% used ({:.1} MB / {} MB)",
                usage_pct, db_size_mb, self.config.limit_mb);
        } else if critical_reached {
            error!("Storage critical: {:.1}% used ({:.1} MB / {} MB)",
                usage_pct, db_size_mb, self.config.limit_mb);
        }

        let metrics = StorageMetrics {
            total_size_bytes: db_size_bytes,
            total_size_mb: db_size_mb,
            total_row_count: total_rows as u64,
            tables,
            limit_mb: self.config.limit_mb,
            usage_pct,
            warning_reached,
            critical_reached,
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        // Cache metrics
        *self.metrics.write().await = metrics.clone();

        Ok(metrics)
    }

    /// Get row count for a specific table.
    fn get_table_row_count(&self, table_name: &str) -> Result<u64, String> {
        // Use the sqlite_store connection
        // We need to add this method to SqliteStore, but for now we'll estimate
        // based on the get_database_stats pattern
        match table_name {
            _ => {
                // Try to get count via existing methods or estimate
                Ok(0) // Placeholder - will be implemented via SqliteStore method
            }
        }
    }

    /// Run data cleanup based on retention policies.
    pub async fn run_cleanup(&self, aggressive: bool) -> Result<CleanupResult, String> {
        let start = std::time::Instant::now();
        let now = chrono::Utc::now().timestamp_millis();

        info!("Starting storage cleanup (aggressive: {})", aggressive);

        let mut result = CleanupResult::default();
        let retention = &self.config.retention;

        // Cleanup trade history
        let trades_cutoff = now - (retention.trades_days as i64 * 24 * 60 * 60 * 1000);
        let trades_cutoff_days = if aggressive { retention.trades_days / 2 } else { retention.trades_days };
        let trades_cutoff_final = now - (trades_cutoff_days as i64 * 24 * 60 * 60 * 1000);

        match self.sqlite_store.cleanup_old_trades(trades_cutoff_final) {
            Ok(count) => {
                result.per_table.push(TableCleanupResult {
                    table: "trades".to_string(),
                    rows_deleted: count as u64,
                    cutoff_timestamp: trades_cutoff_final,
                    error: None,
                });
                result.total_deleted += count as u64;
            }
            Err(e) => {
                result.per_table.push(TableCleanupResult {
                    table: "trades".to_string(),
                    rows_deleted: 0,
                    cutoff_timestamp: trades_cutoff_final,
                    error: Some(e.to_string()),
                });
                result.errors.push(format!("trades: {}", e));
            }
        }

        // Cleanup portfolio snapshots
        let snapshots_days = if aggressive { retention.portfolio_snapshots_days / 2 } else { retention.portfolio_snapshots_days };
        let snapshots_cutoff = now - (snapshots_days as i64 * 24 * 60 * 60 * 1000);

        match self.sqlite_store.cleanup_old_portfolio_snapshots(snapshots_cutoff) {
            Ok(count) => {
                result.per_table.push(TableCleanupResult {
                    table: "portfolio_snapshots".to_string(),
                    rows_deleted: count as u64,
                    cutoff_timestamp: snapshots_cutoff,
                    error: None,
                });
                result.total_deleted += count as u64;
            }
            Err(e) => {
                result.per_table.push(TableCleanupResult {
                    table: "portfolio_snapshots".to_string(),
                    rows_deleted: 0,
                    cutoff_timestamp: snapshots_cutoff,
                    error: Some(e.to_string()),
                });
                result.errors.push(format!("portfolio_snapshots: {}", e));
            }
        }

        // Cleanup prediction history (uses days, not timestamp)
        let predictions_days = if aggressive { retention.prediction_history_days / 2 } else { retention.prediction_history_days };
        let predictions_cutoff = now - (predictions_days as i64 * 24 * 60 * 60 * 1000);

        match self.sqlite_store.cleanup_old_predictions(predictions_days as i64) {
            Ok(count) => {
                result.per_table.push(TableCleanupResult {
                    table: "prediction_history".to_string(),
                    rows_deleted: count as u64,
                    cutoff_timestamp: predictions_cutoff,
                    error: None,
                });
                result.total_deleted += count as u64;
            }
            Err(e) => {
                result.per_table.push(TableCleanupResult {
                    table: "prediction_history".to_string(),
                    rows_deleted: 0,
                    cutoff_timestamp: predictions_cutoff,
                    error: Some(e.to_string()),
                });
                result.errors.push(format!("prediction_history: {}", e));
            }
        }

        // Cleanup node metrics (always use configured retention)
        let metrics_cutoff = now - (retention.node_metrics_days as i64 * 24 * 60 * 60 * 1000);

        match self.sqlite_store.cleanup_old_node_metrics(metrics_cutoff) {
            Ok(count) => {
                result.per_table.push(TableCleanupResult {
                    table: "node_metrics".to_string(),
                    rows_deleted: count as u64,
                    cutoff_timestamp: metrics_cutoff,
                    error: None,
                });
                result.total_deleted += count as u64;
            }
            Err(e) => {
                result.per_table.push(TableCleanupResult {
                    table: "node_metrics".to_string(),
                    rows_deleted: 0,
                    cutoff_timestamp: metrics_cutoff,
                    error: Some(e.to_string()),
                });
                result.errors.push(format!("node_metrics: {}", e));
            }
        }

        // Cleanup sync data (queue, progress, checkpoints)
        let sync_cutoff = now - (retention.sync_data_days as i64 * 24 * 60 * 60 * 1000);

        match self.sqlite_store.cleanup_old_sync_data(retention.sync_data_days as i64) {
            Ok(count) => {
                result.per_table.push(TableCleanupResult {
                    table: "sync_data".to_string(),
                    rows_deleted: count as u64,
                    cutoff_timestamp: sync_cutoff,
                    error: None,
                });
                result.total_deleted += count as u64;
            }
            Err(e) => {
                result.per_table.push(TableCleanupResult {
                    table: "sync_data".to_string(),
                    rows_deleted: 0,
                    cutoff_timestamp: sync_cutoff,
                    error: Some(e.to_string()),
                });
                result.errors.push(format!("sync_data: {}", e));
            }
        }

        // Cleanup funding payments (longer retention)
        let funding_days = if aggressive { retention.funding_payments_days / 2 } else { retention.funding_payments_days };
        let funding_cutoff = now - (funding_days as i64 * 24 * 60 * 60 * 1000);

        match self.sqlite_store.cleanup_old_funding_payments(funding_cutoff) {
            Ok(count) => {
                result.per_table.push(TableCleanupResult {
                    table: "funding_payments".to_string(),
                    rows_deleted: count as u64,
                    cutoff_timestamp: funding_cutoff,
                    error: None,
                });
                result.total_deleted += count as u64;
            }
            Err(e) => {
                result.per_table.push(TableCleanupResult {
                    table: "funding_payments".to_string(),
                    rows_deleted: 0,
                    cutoff_timestamp: funding_cutoff,
                    error: Some(e.to_string()),
                });
                result.errors.push(format!("funding_payments: {}", e));
            }
        }

        // Cleanup margin history
        let margin_days = if aggressive { retention.margin_history_days / 2 } else { retention.margin_history_days };
        let margin_cutoff = now - (margin_days as i64 * 24 * 60 * 60 * 1000);

        match self.sqlite_store.cleanup_old_margin_history(margin_cutoff) {
            Ok(count) => {
                result.per_table.push(TableCleanupResult {
                    table: "margin_history".to_string(),
                    rows_deleted: count as u64,
                    cutoff_timestamp: margin_cutoff,
                    error: None,
                });
                result.total_deleted += count as u64;
            }
            Err(e) => {
                result.per_table.push(TableCleanupResult {
                    table: "margin_history".to_string(),
                    rows_deleted: 0,
                    cutoff_timestamp: margin_cutoff,
                    error: Some(e.to_string()),
                });
                result.errors.push(format!("margin_history: {}", e));
            }
        }

        result.duration_ms = start.elapsed().as_millis() as i64;

        // Estimate bytes freed (rough: 500 bytes per row average)
        result.bytes_freed_estimated = result.total_deleted * 500;

        // Update last cleanup time
        *self.last_cleanup.write().await = Some(now);

        info!(
            "Storage cleanup complete: {} rows deleted, ~{:.1} MB freed in {}ms",
            result.total_deleted,
            result.bytes_freed_estimated as f64 / (1024.0 * 1024.0),
            result.duration_ms
        );

        // Vacuum the database to reclaim space (only if we deleted a lot)
        if result.total_deleted > 10000 {
            info!("Running VACUUM to reclaim disk space...");
            if let Err(e) = self.sqlite_store.vacuum() {
                warn!("VACUUM failed: {}", e);
            }
        }

        // Refresh metrics after cleanup
        let _ = self.collect_metrics().await;

        Ok(result)
    }

    /// Get current storage metrics (cached).
    pub async fn get_metrics(&self) -> StorageMetrics {
        self.metrics.read().await.clone()
    }

    /// Get storage configuration.
    pub fn get_config(&self) -> &StorageConfig {
        &self.config
    }

    /// Get last cleanup timestamp.
    pub async fn get_last_cleanup(&self) -> Option<i64> {
        *self.last_cleanup.read().await
    }

    /// Force immediate cleanup.
    pub async fn force_cleanup(&self) -> Result<CleanupResult, String> {
        self.run_cleanup(false).await
    }

    /// Force aggressive cleanup.
    pub async fn force_aggressive_cleanup(&self) -> Result<CleanupResult, String> {
        self.run_cleanup(true).await
    }
}
