//! Sync service for distributed data synchronization across the mesh.

use crate::services::{PeerMesh, SqliteStore};
use crate::types::{
    ConflictStrategy, EntityType, NodeMetrics, PeerMessage, SyncConflict, SyncMessage,
    SyncOperation, SyncQueueItem, SyncState, SyncUpdateResult,
};
use dashmap::DashMap;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};

/// Sync service manages data synchronization across the mesh network.
pub struct SyncService {
    /// SQLite store for persistent data.
    sqlite_store: Arc<SqliteStore>,
    /// Peer mesh for communication.
    peer_mesh: Arc<PeerMesh>,
    /// This node's ID.
    node_id: String,
    /// Whether this node is the primary (Osaka).
    is_primary: bool,
    /// Broadcast channel for sync messages.
    sync_tx: broadcast::Sender<SyncMessage>,
    /// Pending sync operations (entity_id -> version).
    pending_syncs: DashMap<String, u64>,
    /// Sync state.
    state: RwLock<SyncState>,
    /// Sync metrics.
    metrics: RwLock<SyncMetrics>,
}

/// Sync metrics tracked in memory.
#[derive(Debug, Clone, Default)]
struct SyncMetrics {
    synced_entities_1m: u32,
    sync_errors_1m: u32,
    last_sync_time: Option<i64>,
}

impl SyncService {
    /// Create a new sync service.
    pub fn new(
        sqlite_store: Arc<SqliteStore>,
        peer_mesh: Arc<PeerMesh>,
        node_id: String,
        is_primary: bool,
    ) -> Arc<Self> {
        let (sync_tx, _) = broadcast::channel(1024);

        // Load sync state from database
        let state = sqlite_store
            .get_sync_state()
            .unwrap_or_else(|| SyncState {
                last_full_sync_at: 0,
                last_incremental_sync_at: 0,
                sync_cursor_position: 0,
                pending_sync_count: 0,
                failed_sync_count: 0,
                total_synced_entities: 0,
                sync_enabled: true,
            });

        Arc::new(Self {
            sqlite_store,
            peer_mesh,
            node_id,
            is_primary,
            sync_tx,
            pending_syncs: DashMap::new(),
            state: RwLock::new(state),
            metrics: RwLock::new(SyncMetrics::default()),
        })
    }

    /// Start the sync service background tasks.
    pub fn start(self: Arc<Self>) {
        info!("Starting sync service for node {}", self.node_id);

        // Spawn sync queue processor
        let service = self.clone();
        tokio::spawn(async move {
            service.process_sync_queue().await;
        });

        // Spawn sync message handler
        let service = self.clone();
        tokio::spawn(async move {
            service.handle_sync_messages().await;
        });

        // Spawn metrics collector
        let service = self.clone();
        tokio::spawn(async move {
            service.collect_metrics().await;
        });

        // Spawn reconciliation task (every 5 minutes)
        let service = self.clone();
        tokio::spawn(async move {
            service.periodic_reconciliation().await;
        });
    }

    /// Subscribe to sync messages.
    pub fn subscribe(&self) -> broadcast::Receiver<SyncMessage> {
        self.sync_tx.subscribe()
    }

    /// Queue an entity for sync.
    pub fn queue_sync(
        &self,
        entity_type: EntityType,
        entity_id: String,
        operation: SyncOperation,
        target_nodes: Option<Vec<String>>,
    ) -> Result<(), String> {
        let now = chrono::Utc::now().timestamp_millis();
        let priority = entity_type.priority();

        let item = SyncQueueItem {
            id: uuid::Uuid::new_v4().to_string(),
            entity_type,
            entity_id,
            operation,
            priority,
            target_nodes,
            retry_count: 0,
            created_at: now,
            scheduled_at: now,
            attempted_at: None,
            completed_at: None,
            error: None,
        };

        self.sqlite_store
            .insert_sync_queue_item(&item)
            .map_err(|e| format!("Failed to queue sync: {}", e))?;

        debug!(
            "Queued sync for {:?} {} with priority {}",
            entity_type, item.entity_id, priority
        );

        Ok(())
    }

    /// Process the sync queue continuously.
    async fn process_sync_queue(&self) {
        let mut interval = tokio::time::interval(Duration::from_millis(100));

        loop {
            interval.tick().await;

            // Get pending sync items (limit 10 per batch)
            let items = match self.sqlite_store.get_pending_sync_items(10) {
                Ok(items) => items,
                Err(e) => {
                    error!("Failed to get pending sync items: {}", e);
                    continue;
                }
            };

            if items.is_empty() {
                continue;
            }

            debug!("Processing {} pending sync items", items.len());

            for item in items {
                if let Err(e) = self.process_sync_item(&item).await {
                    error!(
                        "Failed to process sync item {:?} {}: {}",
                        item.entity_type, item.entity_id, e
                    );

                    // Update retry count
                    let _ = self.sqlite_store.update_sync_queue_item_error(
                        &item.id,
                        &e,
                        item.retry_count + 1,
                    );
                }
            }
        }
    }

    /// Process a single sync item.
    async fn process_sync_item(&self, item: &SyncQueueItem) -> Result<(), String> {
        // Get entity data from database
        let data = self
            .sqlite_store
            .get_entity_data(item.entity_type, &item.entity_id)
            .map_err(|e| format!("Failed to get entity data: {}", e))?;

        if data.is_empty() {
            // Entity doesn't exist (might have been deleted)
            if matches!(item.operation, SyncOperation::Delete) {
                // Mark as completed
                self.sqlite_store
                    .complete_sync_queue_item(&item.id)
                    .map_err(|e| format!("Failed to complete sync item: {}", e))?;
                return Ok(());
            } else {
                return Err("Entity not found".to_string());
            }
        }

        // Get version and timestamp
        let (version, timestamp) = match self
            .sqlite_store
            .get_entity_version(item.entity_type, &item.entity_id)
            .map_err(|e| format!("Failed to get entity version: {}", e))?
        {
            Some((v, t)) => (v, t),
            None => {
                // Entity exists but no version info yet, use defaults
                (1u64, chrono::Utc::now().timestamp_millis())
            }
        };

        // Calculate checksum
        let checksum = Self::calculate_checksum(&data);

        // Create sync message
        let sync_msg = SyncMessage::DataUpdate {
            entity_type: item.entity_type,
            entity_id: item.entity_id.clone(),
            version,
            timestamp,
            node_id: self.node_id.clone(),
            checksum,
            data,
        };

        // Send to target nodes or all nodes
        self.broadcast_sync_message(sync_msg, item.target_nodes.clone())
            .await?;

        // Record in sync_versions table
        self.sqlite_store
            .record_sync_version(
                item.entity_type,
                &item.entity_id,
                &self.node_id,
                version,
                timestamp,
                &Self::calculate_checksum(&[]),
            )
            .map_err(|e| format!("Failed to record sync version: {}", e))?;

        // Mark as completed
        self.sqlite_store
            .complete_sync_queue_item(&item.id)
            .map_err(|e| format!("Failed to complete sync item: {}", e))?;

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.synced_entities_1m += 1;
        metrics.last_sync_time = Some(chrono::Utc::now().timestamp_millis());

        Ok(())
    }

    /// Broadcast sync message to peers.
    async fn broadcast_sync_message(
        &self,
        msg: SyncMessage,
        target_nodes: Option<Vec<String>>,
    ) -> Result<(), String> {
        // Serialize message
        let json = serde_json::to_string(&msg).map_err(|e| format!("Serialization error: {}", e))?;

        // Create PeerMessage
        let peer_msg = PeerMessage::SyncData {
            from_id: self.node_id.clone(),
            data: json,
        };

        // Send to all connected peers
        // Note: PeerMesh needs to be extended to support SyncData message type
        // For now, we'll just broadcast to the sync channel
        let _ = self.sync_tx.send(msg);

        Ok(())
    }

    /// Handle incoming sync messages from peers.
    async fn handle_sync_messages(&self) {
        let mut rx = self.subscribe();

        loop {
            match rx.recv().await {
                Ok(msg) => {
                    if let Err(e) = self.handle_sync_message(msg).await {
                        error!("Failed to handle sync message: {}", e);
                        let mut metrics = self.metrics.write().await;
                        metrics.sync_errors_1m += 1;
                    }
                }
                Err(e) => {
                    error!("Sync message channel error: {}", e);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    /// Handle a single sync message.
    async fn handle_sync_message(&self, msg: SyncMessage) -> Result<(), String> {
        match msg {
            SyncMessage::DataUpdate {
                entity_type,
                entity_id,
                version,
                timestamp,
                node_id,
                checksum,
                data,
            } => {
                // Verify checksum
                let calculated_checksum = Self::calculate_checksum(&data);
                if checksum != calculated_checksum {
                    return Err(format!("Checksum mismatch for {:?} {}", entity_type, entity_id));
                }

                // Check if we already have this version
                if let Ok(Some((local_version, _))) =
                    self.sqlite_store.get_entity_version(entity_type, &entity_id)
                {
                    if local_version >= version {
                        // We already have this or a newer version
                        debug!(
                            "Skipping sync for {:?} {} (local version {} >= remote {})",
                            entity_type, entity_id, local_version, version
                        );
                        return Ok(());
                    }

                    // Check for conflict (concurrent updates)
                    if local_version == version - 1 {
                        // Normal sequential update
                        self.apply_sync_update(entity_type, &entity_id, version, timestamp, &node_id, &data)
                            .await?;
                    } else {
                        // Potential conflict - versions don't match
                        warn!(
                            "Conflict detected for {:?} {}: local version {}, remote version {}",
                            entity_type, entity_id, local_version, version
                        );
                        // TODO: Implement conflict resolution
                    }
                } else {
                    // New entity, insert it
                    self.apply_sync_update(entity_type, &entity_id, version, timestamp, &node_id, &data)
                        .await?;
                }
            }
            SyncMessage::ChecksumRequest {
                entity_type,
                entity_id,
            } => {
                // Respond with our checksum
                if let Ok(data) = self.sqlite_store.get_entity_data(entity_type, &entity_id) {
                    if let Ok(Some((version, _))) =
                        self.sqlite_store.get_entity_version(entity_type, &entity_id)
                    {
                        let checksum = Self::calculate_checksum(&data);
                        let response = SyncMessage::ChecksumResponse {
                            entity_type,
                            entity_id,
                            checksum,
                            version,
                        };
                        let _ = self.sync_tx.send(response);
                    }
                }
            }
            SyncMessage::SyncHealthCheck {
                node_id,
                sync_lag_ms,
                pending_syncs,
                error_count,
            } => {
                debug!(
                    "Health check from {}: lag={}ms, pending={}, errors={}",
                    node_id, sync_lag_ms, pending_syncs, error_count
                );
            }
            _ => {
                debug!("Unhandled sync message type");
            }
        }

        Ok(())
    }

    /// Apply a sync update to the local database.
    async fn apply_sync_update(
        &self,
        entity_type: EntityType,
        entity_id: &str,
        version: u64,
        timestamp: i64,
        node_id: &str,
        data: &[u8],
    ) -> Result<(), String> {
        // Update entity in database with new version
        let result = self.sqlite_store
            .update_entity_from_sync(entity_type, entity_id, data, version, timestamp, node_id)
            .map_err(|e| format!("Failed to update entity: {}", e))?;

        match result {
            SyncUpdateResult::Applied => {
                info!(
                    "Applied sync update for {:?} {} to version {}",
                    entity_type, entity_id, version
                );
            }
            SyncUpdateResult::Conflict {
                existing_version,
                existing_timestamp,
                existing_node,
                incoming_version,
                incoming_timestamp,
                incoming_node,
            } => {
                warn!(
                    "Conflict detected for {:?} {}: local v{} ({}), remote v{} ({})",
                    entity_type, entity_id, existing_version, existing_node, incoming_version, incoming_node
                );

                // Resolve conflict based on entity's conflict strategy
                let strategy = entity_type.conflict_strategy();
                let resolution_result = self.resolve_conflict(
                    entity_type,
                    entity_id,
                    existing_version,
                    existing_timestamp,
                    &existing_node,
                    incoming_version,
                    incoming_timestamp,
                    &incoming_node,
                    data,
                    strategy,
                ).await?;

                // Record conflict to database
                if let Err(e) = self.sqlite_store.insert_sync_conflict(
                    entity_type,
                    entity_id,
                    existing_version,
                    existing_timestamp,
                    &existing_node,
                    incoming_version,
                    incoming_timestamp,
                    &incoming_node,
                    &format!("{:?}", strategy),
                    Some(resolution_result),
                ) {
                    error!("Failed to record conflict: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Resolve a sync conflict based on the conflict strategy.
    async fn resolve_conflict(
        &self,
        entity_type: EntityType,
        entity_id: &str,
        local_version: u64,
        local_timestamp: i64,
        local_node: &str,
        remote_version: u64,
        remote_timestamp: i64,
        remote_node: &str,
        remote_data: &[u8],
        strategy: ConflictStrategy,
    ) -> Result<String, String> {
        match strategy {
            ConflictStrategy::PrimaryWins => {
                // Primary node (Osaka) always wins
                if self.is_primary {
                    Ok(format!(
                        "Primary node wins: kept local version {} over remote version {}",
                        local_version, remote_version
                    ))
                } else if remote_node == "osaka" {
                    // Remote is primary, accept remote update
                    self.sqlite_store
                        .update_entity_from_sync(
                            entity_type,
                            entity_id,
                            remote_data,
                            remote_version,
                            remote_timestamp,
                            remote_node,
                        )
                        .map_err(|e| format!("Failed to apply primary's update: {}", e))?;
                    
                    Ok(format!(
                        "Primary node wins: accepted remote version {} from primary over local version {}",
                        remote_version, local_version
                    ))
                } else {
                    // Neither is primary, keep local
                    Ok(format!(
                        "Primary wins strategy but neither is primary: kept local version {}",
                        local_version
                    ))
                }
            }
            ConflictStrategy::LastWriteWins => {
                // Compare timestamps, newer wins
                if remote_timestamp > local_timestamp {
                    // Remote is newer, apply it
                    self.sqlite_store
                        .update_entity_from_sync(
                            entity_type,
                            entity_id,
                            remote_data,
                            remote_version,
                            remote_timestamp,
                            remote_node,
                        )
                        .map_err(|e| format!("Failed to apply remote update: {}", e))?;
                    
                    Ok(format!(
                        "Last write wins: applied remote version {} (ts: {}) over local version {} (ts: {})",
                        remote_version, remote_timestamp, local_version, local_timestamp
                    ))
                } else {
                    Ok(format!(
                        "Last write wins: kept local version {} (ts: {}) over remote version {} (ts: {})",
                        local_version, local_timestamp, remote_version, remote_timestamp
                    ))
                }
            }
            ConflictStrategy::Merge => {
                // For append-only entities (Trade, Liquidation), both can coexist
                // Just insert if not exists (INSERT OR IGNORE handles this)
                self.sqlite_store
                    .update_entity_from_sync(
                        entity_type,
                        entity_id,
                        remote_data,
                        remote_version,
                        remote_timestamp,
                        remote_node,
                    )
                    .map_err(|e| format!("Failed to merge: {}", e))?;
                
                Ok(format!(
                    "Merge strategy: attempted to insert remote version {} alongside local version {}",
                    remote_version, local_version
                ))
            }
            ConflictStrategy::Reject => {
                // Reject remote update, keep local
                Ok(format!(
                    "Reject strategy: rejected remote version {} in favor of local version {}",
                    remote_version, local_version
                ))
            }
        }
    }

    /// Calculate SHA256 checksum of data.
    fn calculate_checksum(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// Collect and record node metrics.
    async fn collect_metrics(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(10));

        loop {
            interval.tick().await;

            let metrics = self.gather_node_metrics().await;
            if let Err(e) = self.sqlite_store.insert_node_metrics(&metrics) {
                error!("Failed to insert node metrics: {}", e);
            }
        }
    }

    /// Gather current node metrics.
    async fn gather_node_metrics(&self) -> NodeMetrics {
        let state = self.state.read().await;
        let metrics_data = self.metrics.read().await;

        let sync_lag_ms = if let Some(last_sync) = metrics_data.last_sync_time {
            chrono::Utc::now().timestamp_millis() - last_sync
        } else {
            0
        };

        // Get database size and counts
        let (db_size_mb, db_row_count) = self
            .sqlite_store
            .get_database_stats()
            .unwrap_or((0.0, 0));

        // Get active counts
        let (active_portfolios, open_orders, open_positions) = self
            .sqlite_store
            .get_active_counts()
            .unwrap_or((0, 0, 0));

        NodeMetrics {
            id: uuid::Uuid::new_v4().to_string(),
            node_id: self.node_id.clone(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            sync_lag_ms,
            pending_sync_count: state.pending_sync_count,
            synced_entities_1m: metrics_data.synced_entities_1m,
            sync_errors_1m: metrics_data.sync_errors_1m,
            sync_throughput_mbps: 0.0, // TODO: Calculate actual throughput
            db_size_mb,
            db_row_count,
            db_write_rate: 0.0, // TODO: Track write rate
            db_read_rate: 0.0,  // TODO: Track read rate
            cpu_usage_pct: None,
            memory_usage_mb: None,
            disk_usage_pct: None,
            network_rx_mbps: None,
            network_tx_mbps: None,
            active_users: 0, // TODO: Track active users
            active_portfolios,
            open_orders,
            open_positions,
        }
    }

    /// Periodic reconciliation to ensure consistency.
    async fn periodic_reconciliation(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes

        loop {
            interval.tick().await;

            info!("Starting periodic reconciliation");

            // For now, just log
            // TODO: Implement checksum comparison with peers
            // TODO: Request missing data
            // TODO: Fix inconsistencies
        }
    }

    /// Check if this node is the primary node.
    pub fn is_primary(&self) -> bool {
        self.is_primary
    }
}
