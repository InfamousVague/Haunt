//! Sync types for distributed mesh data synchronization.

use serde::{Deserialize, Serialize};

/// Types of entities that can be synchronized across the mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Profile,
    Portfolio,
    Order,
    Position,
    Trade,
    OptionsPosition,
    Strategy,
    FundingPayment,
    Liquidation,
    MarginHistory,
    PortfolioSnapshot,
    InsuranceFund,
    PredictionHistory,
}

impl EntityType {
    /// Get the table name for this entity type.
    pub fn table_name(&self) -> &'static str {
        match self {
            EntityType::Profile => "profiles",
            EntityType::Portfolio => "portfolios",
            EntityType::Order => "orders",
            EntityType::Position => "positions",
            EntityType::Trade => "trades",
            EntityType::OptionsPosition => "options_positions",
            EntityType::Strategy => "strategies",
            EntityType::FundingPayment => "funding_payments",
            EntityType::Liquidation => "liquidations",
            EntityType::MarginHistory => "margin_history",
            EntityType::PortfolioSnapshot => "portfolio_snapshots",
            EntityType::InsuranceFund => "insurance_fund",
            EntityType::PredictionHistory => "prediction_history",
        }
    }

    /// Get the sync priority for this entity type (0 = highest).
    pub fn priority(&self) -> u8 {
        match self {
            EntityType::Order => 0,          // Critical: order fills, liquidations
            EntityType::Position => 1,       // Critical: position changes
            EntityType::Portfolio => 2,      // High: balance updates
            EntityType::Trade => 3,          // High: trade history
            EntityType::Liquidation => 3,    // High: liquidation events
            EntityType::InsuranceFund => 4,  // High: fund state
            EntityType::OptionsPosition => 5, // Medium: options
            EntityType::FundingPayment => 6, // Medium: funding
            EntityType::MarginHistory => 7,  // Medium: margin
            EntityType::Profile => 8,        // Low: profile updates
            EntityType::Strategy => 8,       // Low: strategy config
            EntityType::PortfolioSnapshot => 9, // Low: snapshots
            EntityType::PredictionHistory => 10, // Lowest: analytics
        }
    }

    /// Whether this entity type is append-only (never updated after creation).
    pub fn is_append_only(&self) -> bool {
        matches!(
            self,
            EntityType::Trade
                | EntityType::FundingPayment
                | EntityType::Liquidation
                | EntityType::MarginHistory
                | EntityType::PortfolioSnapshot
                | EntityType::PredictionHistory
        )
    }

    /// Get the consistency model for this entity type.
    pub fn consistency_model(&self) -> ConsistencyModel {
        match self {
            // Strong consistency for critical trading entities
            EntityType::Order => ConsistencyModel::Strong,
            EntityType::Position => ConsistencyModel::Strong,
            EntityType::Portfolio => ConsistencyModel::Strong,
            EntityType::OptionsPosition => ConsistencyModel::Strong,
            EntityType::InsuranceFund => ConsistencyModel::Strong,
            
            // Eventual consistency for everything else
            EntityType::Trade => ConsistencyModel::Eventual,
            EntityType::Strategy => ConsistencyModel::Eventual,
            EntityType::FundingPayment => ConsistencyModel::Eventual,
            EntityType::Liquidation => ConsistencyModel::Eventual,
            EntityType::MarginHistory => ConsistencyModel::Eventual,
            EntityType::PortfolioSnapshot => ConsistencyModel::Eventual,
            EntityType::Profile => ConsistencyModel::Eventual,
            EntityType::PredictionHistory => ConsistencyModel::Eventual,
        }
    }

    /// Get the conflict resolution strategy for this entity type.
    pub fn conflict_strategy(&self) -> ConflictStrategy {
        match self {
            // Append-only entities use merge strategy
            EntityType::Trade
            | EntityType::FundingPayment
            | EntityType::Liquidation
            | EntityType::MarginHistory
            | EntityType::PortfolioSnapshot
            | EntityType::PredictionHistory => ConflictStrategy::Merge,
            
            // Critical entities use primary node for conflict resolution
            EntityType::Order
            | EntityType::Position
            | EntityType::InsuranceFund => ConflictStrategy::PrimaryWins,
            
            // Portfolio and OptionsPosition use last-write-wins
            EntityType::Portfolio
            | EntityType::OptionsPosition => ConflictStrategy::LastWriteWins,
            
            // Lower priority entities use last-write-wins
            EntityType::Strategy
            | EntityType::Profile => ConflictStrategy::LastWriteWins,
        }
    }
}

/// Sync operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncOperation {
    Insert,
    Update,
    Delete,
}

/// Consistency model for entity synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsistencyModel {
    /// Strong consistency - updates must be applied in order with version checking
    Strong,
    /// Eventual consistency - updates can be applied out of order, conflicts resolved later
    Eventual,
}

/// Conflict resolution strategy for entity type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictStrategy {
    /// Primary node wins (Osaka decides)
    PrimaryWins,
    /// Last write wins based on timestamp
    LastWriteWins,
    /// Reject conflicting writes, keep existing
    Reject,
    /// Merge fields (for append-only entities)
    Merge,
}

/// Sync messages exchanged between nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SyncMessage {
    /// Update or insert entity data.
    DataUpdate {
        entity_type: EntityType,
        entity_id: String,
        version: u64,
        timestamp: i64,
        node_id: String,
        checksum: String,
        data: Vec<u8>,
    },

    /// Request entity data from peer.
    DataRequest {
        entity_type: EntityType,
        entity_id: String,
        since_version: Option<u64>,
    },

    /// Response to data request.
    DataResponse {
        entity_type: EntityType,
        entity_id: String,
        data: Vec<u8>,
        version: u64,
        timestamp: i64,
        checksum: String,
    },

    /// Bulk sync of multiple entities (for initial sync or recovery).
    BulkSync {
        entity_type: EntityType,
        entities: Vec<SyncEntity>,
        page: u32,
        total_pages: u32,
    },

    /// Conflict detected, resolution required.
    ConflictDetected {
        entity_type: EntityType,
        entity_id: String,
        node_a: String,
        version_a: u64,
        timestamp_a: i64,
        node_b: String,
        version_b: u64,
        timestamp_b: i64,
    },

    /// Conflict resolution result.
    ConflictResolution {
        entity_type: EntityType,
        entity_id: String,
        winner_node: String,
        winner_version: u64,
        winner_data: Vec<u8>,
    },

    /// Health check for sync status.
    SyncHealthCheck {
        node_id: String,
        sync_lag_ms: i64,
        pending_syncs: u32,
        error_count: u32,
    },

    /// Request full reconciliation for an entity type.
    ReconcileRequest {
        entity_type: EntityType,
        entity_ids: Vec<String>,
    },

    /// Checksum verification request.
    ChecksumRequest {
        entity_type: EntityType,
        entity_id: String,
    },

    /// Checksum verification response.
    ChecksumResponse {
        entity_type: EntityType,
        entity_id: String,
        checksum: String,
        version: u64,
    },

    /// Batch update - multiple entities in one message (Phase 3 optimization).
    BatchUpdate {
        updates: Vec<BatchUpdateItem>,
        compression: Option<CompressionType>,
    },

    /// Delta update - only changed fields (Phase 3 optimization).
    DeltaUpdate {
        entity_type: EntityType,
        entity_id: String,
        version: u64,
        timestamp: i64,
        node_id: String,
        changes: Vec<FieldChange>,
    },
}

/// Single item in a batch update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchUpdateItem {
    pub entity_type: EntityType,
    pub entity_id: String,
    pub version: u64,
    pub timestamp: i64,
    pub node_id: String,
    pub checksum: String,
    pub data: Vec<u8>,
}

/// Field change for delta sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldChange {
    pub field: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: serde_json::Value,
}

/// Compression type for large payloads.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressionType {
    None,
    Gzip,
    Brotli,
}

/// Entity data for bulk sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncEntity {
    pub entity_id: String,
    pub version: u64,
    pub timestamp: i64,
    pub checksum: String,
    pub data: Vec<u8>,
}

/// Sync queue item stored in database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncQueueItem {
    pub id: String,
    pub entity_type: EntityType,
    pub entity_id: String,
    pub operation: SyncOperation,
    pub priority: u8,
    pub target_nodes: Option<Vec<String>>, // None = all nodes
    pub retry_count: u32,
    pub created_at: i64,
    pub scheduled_at: i64,
    pub attempted_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub error: Option<String>,
}

/// Sync conflict record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConflict {
    pub id: String,
    pub entity_type: EntityType,
    pub entity_id: String,
    pub node_a: String,
    pub version_a: u64,
    pub data_a: Vec<u8>,
    pub timestamp_a: i64,
    pub node_b: String,
    pub version_b: u64,
    pub data_b: Vec<u8>,
    pub timestamp_b: i64,
    pub detected_at: i64,
    pub resolved_at: Option<i64>,
    pub resolution_strategy: Option<String>,
    pub winner_node: Option<String>,
    pub resolution_reason: Option<String>,
}

/// Result of applying a sync update (may detect conflict).
#[derive(Debug, Clone)]
pub enum SyncUpdateResult {
    /// Update applied successfully
    Applied,
    /// Conflict detected - needs resolution
    Conflict {
        existing_version: u64,
        existing_timestamp: i64,
        existing_node: String,
        incoming_version: u64,
        incoming_timestamp: i64,
        incoming_node: String,
    },
}

/// Node metrics for analytics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeMetrics {
    pub id: String,
    pub node_id: String,
    pub timestamp: i64,
    
    // Sync metrics
    pub sync_lag_ms: i64,
    pub pending_sync_count: u32,
    pub synced_entities_1m: u32,
    pub sync_errors_1m: u32,
    pub sync_throughput_mbps: f64,
    
    // Database metrics
    pub db_size_mb: f64,
    pub db_row_count: u32,
    pub db_write_rate: f64,
    pub db_read_rate: f64,
    
    // System metrics (optional, requires system monitoring)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_usage_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_usage_mb: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_usage_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_rx_mbps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_tx_mbps: Option<f64>,
    
    // User metrics
    pub active_users: u32,
    pub active_portfolios: u32,
    pub open_orders: u32,
    pub open_positions: u32,
}

/// Sync state (single row, persisted state).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncState {
    pub last_full_sync_at: i64,
    pub last_incremental_sync_at: i64,
    pub sync_cursor_position: u64,
    pub pending_sync_count: u32,
    pub failed_sync_count: u32,
    pub total_synced_entities: u64,
    pub sync_enabled: bool,
}
