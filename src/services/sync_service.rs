//! Sync service for distributed data synchronization across the mesh.

use crate::services::{PeerMesh, SqliteStore};
#[allow(unused_imports)]
use crate::types::{
    BatchAck, BatchUpdateItem, CompressionType, ConflictStrategy, EntityType,
    HistoricalSyncSession, HistoricalSyncStatus, NodeMetrics, PeerMessage,
    SyncCheckpoint, SyncConflict, SyncEntity, SyncMessage, SyncOperation,
    SyncProgressUpdate, SyncQueueItem, SyncState, SyncUpdateResult,
};
use dashmap::DashMap;
use flate2::write::GzEncoder;
use flate2::read::GzDecoder;
use flate2::Compression;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};
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
    /// Progress broadcast channel for UI updates.
    progress_tx: broadcast::Sender<SyncProgressUpdate>,
    /// Pending sync operations (entity_id -> version).
    pending_syncs: DashMap<String, u64>,
    /// Active historical sync sessions (session_id -> session).
    active_sessions: DashMap<String, HistoricalSyncSession>,
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
    /// Phase 3: Historical sync metrics
    historical_sync_bytes_sent: u64,
    historical_sync_bytes_received: u64,
    historical_sync_start_time: Option<Instant>,
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
        let (progress_tx, _) = broadcast::channel(256);

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

        // Load active sessions from database
        let active_sessions = DashMap::new();
        if let Ok(sessions) = sqlite_store.get_active_sync_sessions() {
            for session in sessions {
                active_sessions.insert(session.id.clone(), session);
            }
        }

        Arc::new(Self {
            sqlite_store,
            peer_mesh,
            node_id,
            is_primary,
            sync_tx,
            progress_tx,
            pending_syncs: DashMap::new(),
            active_sessions,
            state: RwLock::new(state),
            metrics: RwLock::new(SyncMetrics::default()),
        })
    }

    /// Subscribe to progress updates.
    pub fn subscribe_progress(&self) -> broadcast::Receiver<SyncProgressUpdate> {
        self.progress_tx.subscribe()
    }

    /// Start the sync service background tasks.
    pub fn start(self: Arc<Self>) {
        info!("Starting sync service for node {}", self.node_id);

        // CRITICAL: Subscribe to sync messages BEFORE starting the peer listener
        // to avoid race condition where messages arrive before subscription
        let message_rx = self.sync_tx.subscribe();

        // Spawn sync message handler with pre-subscribed receiver
        let service = self.clone();
        tokio::spawn(async move {
            service.handle_sync_messages_with_rx(message_rx).await;
        });

        // NOW safe to spawn peer sync data listener (handler is already subscribed)
        let service = self.clone();
        tokio::spawn(async move {
            service.listen_for_peer_sync_data().await;
        });

        // Spawn sync queue processor
        let service = self.clone();
        tokio::spawn(async move {
            service.process_sync_queue().await;
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

    /// Listen for incoming sync data from peer mesh.
    async fn listen_for_peer_sync_data(self: Arc<Self>) {
        let mut rx = self.peer_mesh.subscribe_sync_data();

        info!("Sync service listening for peer sync data");

        loop {
            match rx.recv().await {
                Ok((from_id, data)) => {
                    debug!("Received sync data from peer {} ({} bytes)", from_id, data.len());

                    // Parse the SyncMessage from JSON
                    match serde_json::from_str::<SyncMessage>(&data) {
                        Ok(msg) => {
                            // Forward to sync message handler via local channel
                            if let Err(e) = self.sync_tx.send(msg) {
                                debug!("No local subscribers for sync message: {}", e);
                            }
                        }
                        Err(e) => {
                            warn!("Failed to parse sync message from {}: {}", from_id, e);
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("Sync data receiver lagged by {} messages", n);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!("Peer sync data channel closed");
                    break;
                }
            }
        }
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

            // Get pending sync items (limit 50 for batching - Phase 3 optimization)
            let items = match self.sqlite_store.get_pending_sync_items(50) {
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

            // Phase 3: Batch processing - collect items for batching
            if items.len() > 5 {
                // Use batch update for efficiency
                if let Err(e) = self.process_sync_batch(&items).await {
                    error!("Failed to process sync batch: {}", e);
                    // Fall back to individual processing on error
                    for item in items {
                        if let Err(e) = self.process_sync_item(&item).await {
                            error!(
                                "Failed to process sync item {:?} {}: {}",
                                item.entity_type, item.entity_id, e
                            );
                            let _ = self.sqlite_store.update_sync_queue_item_error(
                                &item.id,
                                &e,
                                item.retry_count + 1,
                            );
                        }
                    }
                }
            } else {
                // Process individually for small batches
                for item in items {
                    if let Err(e) = self.process_sync_item(&item).await {
                        error!(
                            "Failed to process sync item {:?} {}: {}",
                            item.entity_type, item.entity_id, e
                        );
                        let _ = self.sqlite_store.update_sync_queue_item_error(
                            &item.id,
                            &e,
                            item.retry_count + 1,
                        );
                    }
                }
            }
        }
    }

    /// Process multiple sync items as a batch (Phase 3 optimization).
    async fn process_sync_batch(&self, items: &[SyncQueueItem]) -> Result<(), String> {
        let mut batch_items = Vec::new();
        let mut completed_ids = Vec::new();

        for item in items {
            // Get entity data
            let data = match self
                .sqlite_store
                .get_entity_data(item.entity_type, &item.entity_id)
            {
                Ok(d) if !d.is_empty() => d,
                _ => {
                    // Skip missing entities
                    if matches!(item.operation, SyncOperation::Delete) {
                        completed_ids.push(item.id.clone());
                    }
                    continue;
                }
            };

            // Get version and timestamp
            let (version, timestamp) = self
                .sqlite_store
                .get_entity_version(item.entity_type, &item.entity_id)
                .map_err(|e| format!("Failed to get entity version: {}", e))?
                .unwrap_or((1u64, chrono::Utc::now().timestamp_millis()));

            // Calculate checksum
            let checksum = Self::calculate_checksum(&data);

            batch_items.push(BatchUpdateItem {
                entity_type: item.entity_type,
                entity_id: item.entity_id.clone(),
                version,
                timestamp,
                node_id: self.node_id.clone(),
                checksum,
                data,
            });

            completed_ids.push(item.id.clone());
        }

        if batch_items.is_empty() {
            return Ok(());
        }

        // Determine if we should compress the batch
        let total_size: usize = batch_items.iter().map(|item| item.data.len()).sum();
        let compression = if total_size > 10240 {
            // Compress if batch is > 10KB
            Some(CompressionType::Gzip)
        } else {
            Some(CompressionType::None)
        };

        // Create batch update message
        let sync_msg = SyncMessage::BatchUpdate {
            updates: batch_items.clone(),
            compression,
        };

        // Broadcast to all nodes
        self.broadcast_sync_message(sync_msg, None).await?;

        // Mark all as completed
        for id in completed_ids {
            self.sqlite_store
                .complete_sync_queue_item(&id)
                .map_err(|e| format!("Failed to complete sync item: {}", e))?;
        }

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.synced_entities_1m += batch_items.len() as u32;
        metrics.last_sync_time = Some(chrono::Utc::now().timestamp_millis());

        info!("Processed batch sync of {} entities", batch_items.len());

        Ok(())
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

        // Also broadcast locally for any local subscribers
        let _ = self.sync_tx.send(msg);

        // Send to peers via PeerMesh
        let sent_count = match &target_nodes {
            Some(nodes) => self.peer_mesh.send_to_peers(peer_msg, nodes).await,
            None => self.peer_mesh.send_to_all_peers(peer_msg).await,
        };

        if sent_count > 0 {
            debug!("Broadcast sync message to {} peers", sent_count);
        } else {
            debug!("No peers available to send sync message");
        }

        Ok(())
    }

    /// Handle incoming sync messages from peers (with pre-subscribed receiver).
    /// This version accepts the receiver to avoid race conditions during startup.
    async fn handle_sync_messages_with_rx(&self, mut rx: broadcast::Receiver<SyncMessage>) {
        info!("Sync message handler started for node {}", self.node_id);

        loop {
            match rx.recv().await {
                Ok(msg) => {
                    if let Err(e) = self.handle_sync_message(msg).await {
                        error!("Failed to handle sync message: {}", e);
                        let mut metrics = self.metrics.write().await;
                        metrics.sync_errors_1m += 1;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("Sync message handler lagged by {} messages", n);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    error!("Sync message channel closed");
                    break;
                }
            }
        }
    }

    /// Handle incoming sync messages from peers (legacy - subscribes internally).
    #[allow(dead_code)]
    async fn handle_sync_messages(&self) {
        let rx = self.subscribe();
        self.handle_sync_messages_with_rx(rx).await;
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
            SyncMessage::BatchUpdate { updates, compression } => {
                // Phase 3: Handle batch updates
                info!("Received batch update with {} entities (compression: {:?})", updates.len(), compression);
                
                let mut successful = 0;
                let mut failed = 0;

                for item in updates {
                    // Decompress if needed
                    let data = if matches!(compression, Some(CompressionType::Gzip)) {
                        match Self::decompress_gzip(&item.data) {
                            Ok(d) => d,
                            Err(e) => {
                                warn!("Failed to decompress data for {:?} {}: {}", item.entity_type, item.entity_id, e);
                                failed += 1;
                                continue;
                            }
                        }
                    } else {
                        item.data.clone()
                    };

                    // Verify checksum
                    let calculated_checksum = Self::calculate_checksum(&data);
                    if item.checksum != calculated_checksum {
                        warn!("Checksum mismatch for {:?} {}", item.entity_type, item.entity_id);
                        failed += 1;
                        continue;
                    }

                    // Check version and apply update
                    if let Ok(Some((local_version, _))) =
                        self.sqlite_store.get_entity_version(item.entity_type, &item.entity_id)
                    {
                        if local_version >= item.version {
                            // Skip outdated update
                            debug!(
                                "Skipping batch item {:?} {} (local {} >= remote {})",
                                item.entity_type, item.entity_id, local_version, item.version
                            );
                            continue;
                        }
                    }

                    // Apply update
                    match self.apply_sync_update(
                        item.entity_type,
                        &item.entity_id,
                        item.version,
                        item.timestamp,
                        &item.node_id,
                        &data,
                    ).await {
                        Ok(_) => successful += 1,
                        Err(e) => {
                            warn!("Failed to apply batch update for {:?} {}: {}", item.entity_type, item.entity_id, e);
                            failed += 1;
                        }
                    }
                }

                info!("Batch update complete: {} successful, {} failed", successful, failed);
            }
            SyncMessage::DeltaUpdate { .. } => {
                // Phase 3: Delta updates (future optimization)
                debug!("Delta update received (not yet implemented)");
            }

            // ========== Phase 3: Historical Sync Messages ==========

            SyncMessage::HistoricalSyncRequest {
                session_id,
                from_node_id,
                entity_types,
                batch_size,
                resume_from_checkpoint,
            } => {
                info!(
                    "PRIMARY received HistoricalSyncRequest from '{}' for session {} ({} entity types, batch_size={})",
                    from_node_id, session_id, entity_types.len(), batch_size
                );
                debug!(
                    "Entity types requested: {:?}, resume_from: {:?}",
                    entity_types, resume_from_checkpoint
                );

                // Only primary node can provide historical sync
                if !self.is_primary {
                    let response = SyncMessage::HistoricalSyncResponse {
                        session_id,
                        accepted: false,
                        total_entities: 0,
                        estimated_batches: 0,
                        error: Some("Only primary node can provide historical sync".to_string()),
                    };
                    self.broadcast_sync_message(response, Some(vec![from_node_id])).await?;
                    return Ok(());
                }

                // Calculate total entities and batches
                let types_to_sync = if entity_types.is_empty() {
                    // All entity types
                    vec![
                        EntityType::Profile, EntityType::Portfolio, EntityType::Order,
                        EntityType::Position, EntityType::Trade, EntityType::Strategy,
                        EntityType::OptionsPosition, EntityType::FundingPayment,
                        EntityType::Liquidation, EntityType::MarginHistory,
                        EntityType::PortfolioSnapshot, EntityType::InsuranceFund,
                        EntityType::PredictionHistory,
                    ]
                } else {
                    entity_types.clone()
                };

                let mut total_entities = 0u64;
                for entity_type in &types_to_sync {
                    total_entities += self.sqlite_store.get_entity_count(*entity_type).unwrap_or(0);
                }

                let batch_size = batch_size.clamp(100, 1000);
                let estimated_batches = ((total_entities as f64) / (batch_size as f64)).ceil() as u32;

                // Create session
                let now = chrono::Utc::now().timestamp_millis();
                let session = HistoricalSyncSession {
                    id: session_id.clone(),
                    source_node_id: self.node_id.clone(),
                    target_node_id: from_node_id.clone(),
                    status: HistoricalSyncStatus::InProgress,
                    entity_types: types_to_sync.clone(),
                    total_entities,
                    synced_entities: 0,
                    failed_entities: 0,
                    progress_percent: 0.0,
                    current_entity_type: types_to_sync.first().copied(),
                    current_batch: 0,
                    total_batches: estimated_batches,
                    bytes_transferred: 0,
                    compression_savings_bytes: 0,
                    started_at: now,
                    updated_at: now,
                    completed_at: None,
                    error: None,
                };

                // Save session
                let _ = self.sqlite_store.create_historical_sync_session(&session);
                self.active_sessions.insert(session_id.clone(), session);

                // Send response
                let response = SyncMessage::HistoricalSyncResponse {
                    session_id: session_id.clone(),
                    accepted: true,
                    total_entities,
                    estimated_batches,
                    error: None,
                };
                self.broadcast_sync_message(response, Some(vec![from_node_id.clone()])).await?;

                // Start sending batches
                let service = Arc::new(self.clone_for_task());
                let session_id_clone = session_id.clone();
                let from_node_clone = from_node_id.clone();
                tokio::spawn(async move {
                    if let Err(e) = service.send_historical_sync_batches(
                        &session_id_clone,
                        &from_node_clone,
                        types_to_sync,
                        batch_size,
                        resume_from_checkpoint,
                    ).await {
                        error!("Historical sync failed for session {}: {}", session_id_clone, e);
                    }
                });
            }

            SyncMessage::HistoricalSyncResponse {
                session_id,
                accepted,
                total_entities,
                estimated_batches,
                error,
            } => {
                if accepted {
                    info!(
                        "Historical sync accepted for session {}: {} entities, {} batches",
                        session_id, total_entities, estimated_batches
                    );

                    // Update session with estimated totals
                    if let Some(mut session) = self.active_sessions.get_mut(&session_id) {
                        session.total_entities = total_entities;
                        session.total_batches = estimated_batches;
                        session.status = HistoricalSyncStatus::InProgress;
                        let _ = self.sqlite_store.update_historical_sync_session(&session);
                    }
                } else {
                    warn!(
                        "Historical sync rejected for session {}: {:?}",
                        session_id, error
                    );

                    // Mark session as failed
                    if let Some(mut session) = self.active_sessions.get_mut(&session_id) {
                        session.status = HistoricalSyncStatus::Failed;
                        session.error = error;
                        session.completed_at = Some(chrono::Utc::now().timestamp_millis());
                        let _ = self.sqlite_store.update_historical_sync_session(&session);
                    }
                }
            }

            SyncMessage::HistoricalSyncBatch {
                session_id,
                batch_id,
                entity_type,
                batch_number,
                total_batches,
                entities,
                compression,
                checksum,
                checkpoint_id,
            } => {
                info!(
                    "Received historical batch {}/{} for session {} ({} {:?} entities, compression={:?})",
                    batch_number, total_batches, session_id, entities.len(), entity_type, compression
                );

                let mut items_applied = 0u32;
                let mut items_skipped = 0u32;
                let mut items_failed = 0u32;
                let mut errors = Vec::new();

                // Apply each entity
                for entity in &entities {
                    // Decompress if needed
                    let data = if matches!(compression, CompressionType::Gzip) {
                        match Self::decompress_gzip(&entity.data) {
                            Ok(d) => d,
                            Err(e) => {
                                errors.push((entity.entity_id.clone(), e.clone()));
                                items_failed += 1;
                                continue;
                            }
                        }
                    } else {
                        entity.data.clone()
                    };

                    // Verify checksum
                    let calculated_checksum = Self::calculate_checksum(&data);
                    if entity.checksum != calculated_checksum {
                        errors.push((entity.entity_id.clone(), "Checksum mismatch".to_string()));
                        items_failed += 1;
                        continue;
                    }

                    // Check version
                    if let Ok(Some((local_version, _))) = self.sqlite_store.get_entity_version(entity_type, &entity.entity_id) {
                        if local_version >= entity.version {
                            items_skipped += 1;
                            continue;
                        }
                    }

                    // Apply update
                    match self.apply_sync_update(
                        entity_type,
                        &entity.entity_id,
                        entity.version,
                        entity.timestamp,
                        &self.node_id,
                        &data,
                    ).await {
                        Ok(_) => items_applied += 1,
                        Err(e) => {
                            errors.push((entity.entity_id.clone(), e));
                            items_failed += 1;
                        }
                    }
                }

                // Update session progress
                if let Some(mut session) = self.active_sessions.get_mut(&session_id) {
                    session.synced_entities += items_applied as u64;
                    session.failed_entities += items_failed as u64;
                    session.current_batch = batch_number;
                    session.current_entity_type = Some(entity_type);
                    session.progress_percent = if session.total_entities > 0 {
                        (session.synced_entities as f32 / session.total_entities as f32) * 100.0
                    } else {
                        0.0
                    };
                    session.updated_at = chrono::Utc::now().timestamp_millis();
                    let _ = self.sqlite_store.update_historical_sync_session(&session);

                    // Broadcast progress
                    let progress = SyncProgressUpdate {
                        session_id: session_id.clone(),
                        entity_type: Some(entity_type),
                        total_entities: session.total_entities,
                        synced_entities: session.synced_entities,
                        failed_entities: session.failed_entities,
                        progress_percent: session.progress_percent,
                        current_batch: batch_number,
                        total_batches,
                        bytes_per_second: 0.0,
                        estimated_remaining_ms: 0,
                        timestamp: session.updated_at,
                    };
                    let _ = self.progress_tx.send(progress.clone());
                    let _ = self.sqlite_store.record_sync_progress(&progress);
                }

                // Save checkpoint if provided
                if let Some(cp_id) = &checkpoint_id {
                    let last_entity = entities.last();
                    if let Some(entity) = last_entity {
                        let checkpoint = SyncCheckpoint {
                            id: cp_id.clone(),
                            session_id: session_id.clone(),
                            entity_type,
                            last_synced_id: entity.entity_id.clone(),
                            last_synced_rowid: 0, // Not known on receiver side
                            batch_number,
                            items_in_batch: entities.len() as u32,
                            checksum: checksum.clone(),
                            compression_type: compression,
                            compressed_size_bytes: 0,
                            uncompressed_size_bytes: 0,
                            created_at: chrono::Utc::now().timestamp_millis(),
                            acked_at: None,
                        };
                        let _ = self.sqlite_store.save_sync_checkpoint(&checkpoint);
                    }
                }

                // Send acknowledgment
                let ack = BatchAck {
                    batch_id: batch_id.clone(),
                    session_id: session_id.clone(),
                    items_received: entities.len() as u32,
                    items_applied,
                    items_skipped,
                    items_failed,
                    checksum_verified: true,
                    errors,
                    timestamp: chrono::Utc::now().timestamp_millis(),
                };

                let ack_msg = SyncMessage::HistoricalSyncBatchAck {
                    session_id,
                    batch_id,
                    ack: ack.clone(),
                };

                let _ = self.sqlite_store.save_batch_ack(&ack);
                // Broadcast ack back to sender
                let _ = self.sync_tx.send(ack_msg);
            }

            SyncMessage::HistoricalSyncBatchAck { session_id, batch_id, ack } => {
                debug!(
                    "Received batch ack for {} in session {}: {} applied, {} failed",
                    batch_id, session_id, ack.items_applied, ack.items_failed
                );

                // Save ack and mark checkpoint as acknowledged
                let _ = self.sqlite_store.save_batch_ack(&ack);

                // Ack the checkpoint for this batch
                if let Some(session) = self.active_sessions.get(&session_id) {
                    if let Some(current_type) = session.current_entity_type {
                        if let Some(checkpoint) = self.sqlite_store.get_latest_checkpoint(&session_id, current_type) {
                            let _ = self.sqlite_store.ack_checkpoint(&checkpoint.id);
                        }
                    }
                }
            }

            SyncMessage::HistoricalSyncProgress { session_id, progress } => {
                // Forward progress to local subscribers
                let _ = self.progress_tx.send(progress.clone());
                let _ = self.sqlite_store.record_sync_progress(&progress);

                // Update in-memory session
                if let Some(mut session) = self.active_sessions.get_mut(&session_id) {
                    session.synced_entities = progress.synced_entities;
                    session.failed_entities = progress.failed_entities;
                    session.progress_percent = progress.progress_percent;
                    session.current_batch = progress.current_batch;
                    session.updated_at = progress.timestamp;
                }
            }

            SyncMessage::HistoricalSyncPause { session_id, reason } => {
                info!("Pausing historical sync session {}: {:?}", session_id, reason);

                if let Some(mut session) = self.active_sessions.get_mut(&session_id) {
                    session.status = HistoricalSyncStatus::Paused;
                    session.updated_at = chrono::Utc::now().timestamp_millis();
                    let _ = self.sqlite_store.update_historical_sync_session(&session);
                }
            }

            SyncMessage::HistoricalSyncResume { session_id, from_checkpoint } => {
                info!("Resuming historical sync session {} from checkpoint {:?}", session_id, from_checkpoint);

                if let Some(mut session) = self.active_sessions.get_mut(&session_id) {
                    session.status = HistoricalSyncStatus::InProgress;
                    session.updated_at = chrono::Utc::now().timestamp_millis();
                    let _ = self.sqlite_store.update_historical_sync_session(&session);
                }
            }

            SyncMessage::HistoricalSyncCancel { session_id, reason } => {
                info!("Cancelling historical sync session {}: {:?}", session_id, reason);

                if let Some(mut session) = self.active_sessions.get_mut(&session_id) {
                    session.status = HistoricalSyncStatus::Cancelled;
                    session.completed_at = Some(chrono::Utc::now().timestamp_millis());
                    session.error = reason;
                    let _ = self.sqlite_store.update_historical_sync_session(&session);
                }
                self.active_sessions.remove(&session_id);
            }

            SyncMessage::HistoricalSyncComplete {
                session_id,
                total_entities,
                total_batches,
                duration_ms,
                bytes_transferred,
            } => {
                info!(
                    "Historical sync complete for session {}: {} entities in {} batches ({} ms, {} bytes)",
                    session_id, total_entities, total_batches, duration_ms, bytes_transferred
                );

                if let Some(mut session) = self.active_sessions.get_mut(&session_id) {
                    session.status = HistoricalSyncStatus::Completed;
                    session.completed_at = Some(chrono::Utc::now().timestamp_millis());
                    session.bytes_transferred = bytes_transferred;
                    let _ = self.sqlite_store.update_historical_sync_session(&session);
                }
                self.active_sessions.remove(&session_id);

                // Update sync state
                let mut state = self.state.write().await;
                state.last_full_sync_at = chrono::Utc::now().timestamp_millis();
                state.total_synced_entities += total_entities;
                let _ = self.sqlite_store.update_sync_state(&state);
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

    /// Compress data using gzip.
    fn compress_gzip(data: &[u8]) -> Result<Vec<u8>, String> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(data)
            .map_err(|e| format!("Compression failed: {}", e))?;
        encoder
            .finish()
            .map_err(|e| format!("Compression finalization failed: {}", e))
    }

    /// Decompress gzip data.
    fn decompress_gzip(data: &[u8]) -> Result<Vec<u8>, String> {
        let mut decoder = GzDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .map_err(|e| format!("Decompression failed: {}", e))?;
        Ok(decompressed)
    }

    /// Determine if data should be compressed (> 1KB).
    fn should_compress(data: &[u8]) -> bool {
        data.len() > 1024
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

    // ========== Phase 3: Historical Sync Methods ==========

    /// Create a copy of service data for spawned tasks.
    fn clone_for_task(&self) -> SyncServiceTask {
        SyncServiceTask {
            sqlite_store: self.sqlite_store.clone(),
            peer_mesh: self.peer_mesh.clone(),
            node_id: self.node_id.clone(),
            sync_tx: self.sync_tx.clone(),
            progress_tx: self.progress_tx.clone(),
            active_sessions: self.active_sessions.clone(),
        }
    }

    /// Request historical sync from primary node (for new nodes joining).
    pub async fn request_historical_sync(
        &self,
        entity_types: Vec<EntityType>,
        batch_size: u32,
    ) -> Result<String, String> {
        if self.is_primary {
            return Err("Primary node cannot request sync from itself".to_string());
        }

        let session_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();

        // Create local session to track progress
        let session = HistoricalSyncSession {
            id: session_id.clone(),
            source_node_id: "osaka".to_string(), // Primary node
            target_node_id: self.node_id.clone(),
            status: HistoricalSyncStatus::Pending,
            entity_types: entity_types.clone(),
            total_entities: 0,
            synced_entities: 0,
            failed_entities: 0,
            progress_percent: 0.0,
            current_entity_type: None,
            current_batch: 0,
            total_batches: 0,
            bytes_transferred: 0,
            compression_savings_bytes: 0,
            started_at: now,
            updated_at: now,
            completed_at: None,
            error: None,
        };

        // Save session
        self.sqlite_store.create_historical_sync_session(&session)
            .map_err(|e| format!("Failed to create sync session: {}", e))?;
        self.active_sessions.insert(session_id.clone(), session);

        // Send request to primary
        let request = SyncMessage::HistoricalSyncRequest {
            session_id: session_id.clone(),
            from_node_id: self.node_id.clone(),
            entity_types,
            batch_size,
            resume_from_checkpoint: None,
        };

        info!(
            "Sending HistoricalSyncRequest to primary (osaka) for session {}",
            session_id
        );
        self.broadcast_sync_message(request, Some(vec!["osaka".to_string()])).await?;

        info!("Successfully sent historical sync request for session {}", session_id);
        Ok(session_id)
    }

    /// Resume a paused historical sync session.
    pub async fn resume_historical_sync(&self, session_id: &str) -> Result<(), String> {
        let session = self.active_sessions.get(session_id)
            .ok_or_else(|| "Session not found".to_string())?;

        if session.status != HistoricalSyncStatus::Paused {
            return Err("Session is not paused".to_string());
        }

        // Find latest checkpoint
        let checkpoint_id = if let Some(current_type) = session.current_entity_type {
            self.sqlite_store.get_latest_checkpoint(session_id, current_type)
                .map(|cp| cp.id)
        } else {
            None
        };

        // Send resume message
        let resume = SyncMessage::HistoricalSyncResume {
            session_id: session_id.to_string(),
            from_checkpoint: checkpoint_id,
        };

        self.broadcast_sync_message(resume, Some(vec![session.source_node_id.clone()])).await?;

        Ok(())
    }

    /// Pause a historical sync session.
    pub async fn pause_historical_sync(&self, session_id: &str) -> Result<(), String> {
        if let Some(mut session) = self.active_sessions.get_mut(session_id) {
            session.status = HistoricalSyncStatus::Paused;
            session.updated_at = chrono::Utc::now().timestamp_millis();
            let _ = self.sqlite_store.update_historical_sync_session(&session);
        }

        let pause = SyncMessage::HistoricalSyncPause {
            session_id: session_id.to_string(),
            reason: Some("User requested pause".to_string()),
        };

        self.broadcast_sync_message(pause, None).await
    }

    /// Cancel a historical sync session.
    pub async fn cancel_historical_sync(&self, session_id: &str) -> Result<(), String> {
        if let Some(mut session) = self.active_sessions.get_mut(session_id) {
            session.status = HistoricalSyncStatus::Cancelled;
            session.completed_at = Some(chrono::Utc::now().timestamp_millis());
            let _ = self.sqlite_store.update_historical_sync_session(&session);
        }

        let cancel = SyncMessage::HistoricalSyncCancel {
            session_id: session_id.to_string(),
            reason: Some("User cancelled".to_string()),
        };

        self.broadcast_sync_message(cancel, None).await?;

        self.active_sessions.remove(session_id);
        Ok(())
    }

    /// Get current sync progress for a session.
    pub fn get_sync_progress(&self, session_id: &str) -> Option<SyncProgressUpdate> {
        self.active_sessions.get(session_id).map(|session| {
            SyncProgressUpdate {
                session_id: session_id.to_string(),
                entity_type: session.current_entity_type,
                total_entities: session.total_entities,
                synced_entities: session.synced_entities,
                failed_entities: session.failed_entities,
                progress_percent: session.progress_percent,
                current_batch: session.current_batch,
                total_batches: session.total_batches,
                bytes_per_second: 0.0,
                estimated_remaining_ms: 0,
                timestamp: session.updated_at,
            }
        })
    }

    /// Get all active sync sessions.
    pub fn get_active_sessions(&self) -> Vec<HistoricalSyncSession> {
        self.active_sessions.iter().map(|r| r.value().clone()).collect()
    }
}

/// Task-safe subset of SyncService for spawned background tasks.
struct SyncServiceTask {
    sqlite_store: Arc<SqliteStore>,
    peer_mesh: Arc<PeerMesh>,
    node_id: String,
    sync_tx: broadcast::Sender<SyncMessage>,
    progress_tx: broadcast::Sender<SyncProgressUpdate>,
    active_sessions: DashMap<String, HistoricalSyncSession>,
}

impl SyncServiceTask {
    /// Send historical sync batches to requesting node.
    async fn send_historical_sync_batches(
        &self,
        session_id: &str,
        target_node: &str,
        entity_types: Vec<EntityType>,
        batch_size: u32,
        _resume_from_checkpoint: Option<String>,
    ) -> Result<(), String> {
        let start_time = Instant::now();
        let mut total_bytes_transferred = 0u64;
        let mut total_entities_sent = 0u64;
        let mut batch_number = 0u32;

        for entity_type in entity_types {
            info!("Syncing {:?} for session {}", entity_type, session_id);

            let mut after_rowid = 0i64;

            loop {
                // Check if session is still active
                if let Some(session) = self.active_sessions.get(session_id) {
                    if session.status == HistoricalSyncStatus::Paused ||
                       session.status == HistoricalSyncStatus::Cancelled {
                        info!("Session {} is paused/cancelled, stopping batch send", session_id);
                        return Ok(());
                    }
                }

                // Get batch of entities
                debug!(
                    "Fetching batch for {:?}, after_rowid={}, batch_size={}",
                    entity_type, after_rowid, batch_size
                );
                let entities = self.sqlite_store.get_entities_for_sync(entity_type, after_rowid, batch_size)
                    .map_err(|e| format!("Failed to get entities: {}", e))?;

                debug!(
                    "Got {} entities for {:?} (after_rowid={})",
                    entities.len(), entity_type, after_rowid
                );

                if entities.is_empty() {
                    debug!("No more entities for {:?}, moving to next type", entity_type);
                    break;
                }

                let last_rowid = entities.last().map(|(rowid, _, _)| *rowid).unwrap_or(after_rowid);
                batch_number += 1;

                // Convert to SyncEntity format
                let mut sync_entities = Vec::with_capacity(entities.len());
                let mut uncompressed_size = 0usize;

                for (_, entity_id, data) in &entities {
                    let version = self.sqlite_store.get_entity_version(entity_type, entity_id)
                        .ok()
                        .flatten()
                        .map(|(v, _)| v)
                        .unwrap_or(1);

                    let checksum = Self::calculate_checksum(data);

                    sync_entities.push(SyncEntity {
                        entity_id: entity_id.clone(),
                        version,
                        timestamp: chrono::Utc::now().timestamp_millis(),
                        checksum,
                        data: data.clone(),
                    });

                    uncompressed_size += data.len();
                }

                // Determine compression
                let (compression, batch_data) = if uncompressed_size > 10240 {
                    // Compress each entity's data
                    let mut compressed_entities = sync_entities.clone();
                    for entity in &mut compressed_entities {
                        if let Ok(compressed) = Self::compress_gzip(&entity.data) {
                            if compressed.len() < entity.data.len() {
                                entity.data = compressed;
                            }
                        }
                    }
                    (CompressionType::Gzip, compressed_entities)
                } else {
                    (CompressionType::None, sync_entities)
                };

                let compressed_size: usize = batch_data.iter().map(|e| e.data.len()).sum();

                // Calculate batch checksum
                let mut batch_checksum = sha2::Sha256::new();
                for entity in &batch_data {
                    batch_checksum.update(&entity.data);
                }
                let batch_checksum_str = format!("{:x}", batch_checksum.finalize());

                // Create checkpoint
                let checkpoint_id = uuid::Uuid::new_v4().to_string();
                let checkpoint = SyncCheckpoint {
                    id: checkpoint_id.clone(),
                    session_id: session_id.to_string(),
                    entity_type,
                    last_synced_id: entities.last().map(|(_, id, _)| id.clone()).unwrap_or_default(),
                    last_synced_rowid: last_rowid,
                    batch_number,
                    items_in_batch: entities.len() as u32,
                    checksum: batch_checksum_str.clone(),
                    compression_type: compression,
                    compressed_size_bytes: compressed_size as u64,
                    uncompressed_size_bytes: uncompressed_size as u64,
                    created_at: chrono::Utc::now().timestamp_millis(),
                    acked_at: None,
                };
                let _ = self.sqlite_store.save_sync_checkpoint(&checkpoint);

                // Get total batches estimate
                let total_count = self.sqlite_store.get_entity_count(entity_type).unwrap_or(0);
                let total_batches = ((total_count as f64) / (batch_size as f64)).ceil() as u32;

                // Send batch
                let batch_msg = SyncMessage::HistoricalSyncBatch {
                    session_id: session_id.to_string(),
                    batch_id: uuid::Uuid::new_v4().to_string(),
                    entity_type,
                    batch_number,
                    total_batches,
                    entities: batch_data,
                    compression,
                    checksum: batch_checksum_str,
                    checkpoint_id: Some(checkpoint_id),
                };

                // Serialize and send
                let json = serde_json::to_string(&batch_msg)
                    .map_err(|e| format!("Serialization error: {}", e))?;

                let peer_msg = PeerMessage::SyncData {
                    from_id: self.node_id.clone(),
                    data: json,
                };

                // CRITICAL: Check if send succeeded - if no peer connection, fail the session
                let sent_count = self.peer_mesh.send_to_peers(peer_msg, &[target_node.to_string()]).await;
                if sent_count == 0 {
                    error!(
                        "Failed to send batch {} to {}: no active connection",
                        batch_number, target_node
                    );
                    if let Some(mut session) = self.active_sessions.get_mut(session_id) {
                        session.status = HistoricalSyncStatus::Failed;
                        session.error = Some(format!("No connection to target node {}", target_node));
                        session.updated_at = chrono::Utc::now().timestamp_millis();
                        let _ = self.sqlite_store.update_historical_sync_session(&session);
                    }
                    return Err(format!("No connection to target node {}", target_node));
                }

                info!(
                    "Sent batch {}/{} ({} entities, {} bytes) to {}",
                    batch_number, total_batches, entities.len(), compressed_size, target_node
                );

                // Update progress
                total_bytes_transferred += compressed_size as u64;
                total_entities_sent += entities.len() as u64;

                if let Some(mut session) = self.active_sessions.get_mut(session_id) {
                    session.synced_entities = total_entities_sent;
                    session.current_batch = batch_number;
                    session.current_entity_type = Some(entity_type);
                    session.bytes_transferred = total_bytes_transferred;
                    session.compression_savings_bytes += (uncompressed_size - compressed_size) as u64;
                    session.progress_percent = if session.total_entities > 0 {
                        (total_entities_sent as f32 / session.total_entities as f32) * 100.0
                    } else {
                        0.0
                    };
                    session.updated_at = chrono::Utc::now().timestamp_millis();
                    let _ = self.sqlite_store.update_historical_sync_session(&session);

                    // Broadcast progress
                    let elapsed = start_time.elapsed().as_secs_f64();
                    let bytes_per_second = if elapsed > 0.0 {
                        total_bytes_transferred as f64 / elapsed
                    } else {
                        0.0
                    };

                    let remaining_entities = session.total_entities.saturating_sub(total_entities_sent);
                    let estimated_remaining_ms = if bytes_per_second > 0.0 && remaining_entities > 0 {
                        // Estimate based on current throughput
                        let avg_entity_size = total_bytes_transferred as f64 / total_entities_sent as f64;
                        let remaining_bytes = remaining_entities as f64 * avg_entity_size;
                        ((remaining_bytes / bytes_per_second) * 1000.0) as i64
                    } else {
                        0
                    };

                    let progress = SyncProgressUpdate {
                        session_id: session_id.to_string(),
                        entity_type: Some(entity_type),
                        total_entities: session.total_entities,
                        synced_entities: total_entities_sent,
                        failed_entities: session.failed_entities,
                        progress_percent: session.progress_percent,
                        current_batch: batch_number,
                        total_batches,
                        bytes_per_second,
                        estimated_remaining_ms,
                        timestamp: session.updated_at,
                    };

                    let _ = self.progress_tx.send(progress.clone());
                    let _ = self.sqlite_store.record_sync_progress(&progress);
                }

                // Small delay to prevent overwhelming the network
                tokio::time::sleep(Duration::from_millis(10)).await;

                after_rowid = last_rowid;
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as i64;

        // Send completion message
        let complete_msg = SyncMessage::HistoricalSyncComplete {
            session_id: session_id.to_string(),
            total_entities: total_entities_sent,
            total_batches: batch_number,
            duration_ms,
            bytes_transferred: total_bytes_transferred,
        };

        let json = serde_json::to_string(&complete_msg)
            .map_err(|e| format!("Serialization error: {}", e))?;

        let peer_msg = PeerMessage::SyncData {
            from_id: self.node_id.clone(),
            data: json,
        };

        let sent_count = self.peer_mesh.send_to_peers(peer_msg, &[target_node.to_string()]).await;
        if sent_count == 0 {
            warn!(
                "Failed to send completion message to {}, but all batches were sent successfully",
                target_node
            );
        }

        // Update session as completed
        if let Some(mut session) = self.active_sessions.get_mut(session_id) {
            session.status = HistoricalSyncStatus::Completed;
            session.completed_at = Some(chrono::Utc::now().timestamp_millis());
            let _ = self.sqlite_store.update_historical_sync_session(&session);
        }

        info!(
            "Historical sync complete for session {}: {} entities in {} batches ({} ms)",
            session_id, total_entities_sent, batch_number, duration_ms
        );

        Ok(())
    }

    fn compress_gzip(data: &[u8]) -> Result<Vec<u8>, String> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).map_err(|e| format!("Compression failed: {}", e))?;
        encoder.finish().map_err(|e| format!("Compression finalization failed: {}", e))
    }

    fn calculate_checksum(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }
}
