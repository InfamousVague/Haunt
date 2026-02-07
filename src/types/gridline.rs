//! Gridline Trading Types
//!
//! Types for the Gridline real-time prediction trading system.
//! Users place positions on a price/time grid and win if the price line
//! crosses through their cell during the time window.

use serde::{Deserialize, Serialize};

// =============================================================================
// Enums
// =============================================================================

/// Status of a gridline position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GridlineStatus {
    /// Waiting for resolution (time column hasn't expired yet)
    Active,
    /// Price crossed through the cell — user wins
    Won,
    /// Column expired without price reaching the cell — user loses
    Lost,
    /// User cancelled the position before resolution
    Cancelled,
}

impl std::fmt::Display for GridlineStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GridlineStatus::Active => write!(f, "active"),
            GridlineStatus::Won => write!(f, "won"),
            GridlineStatus::Lost => write!(f, "lost"),
            GridlineStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl GridlineStatus {
    pub fn from_str(s: &str) -> Self {
        match s {
            "active" => GridlineStatus::Active,
            "won" => GridlineStatus::Won,
            "lost" => GridlineStatus::Lost,
            "cancelled" => GridlineStatus::Cancelled,
            _ => GridlineStatus::Active,
        }
    }
}

// =============================================================================
// Core Structs
// =============================================================================

/// A single gridline position placed by a user on a specific price/time cell.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridlinePosition {
    /// Unique position identifier
    pub id: String,
    /// Portfolio that placed the position
    pub portfolio_id: String,
    /// Trading symbol (e.g., "BTC", "ETH")
    pub symbol: String,
    /// Base position amount in USD (margin committed)
    pub amount: f64,
    /// Leverage multiplier (1.0 to 10.0)
    pub leverage: f64,
    /// Effective position amount = amount * leverage
    pub effective_amount: f64,
    /// Payout multiplier at time of placement (based on volatility/distance)
    pub multiplier: f64,
    /// Potential payout if won = effective_amount * multiplier
    pub potential_payout: f64,
    /// Bottom of the cell's price range
    pub price_low: f64,
    /// Top of the cell's price range
    pub price_high: f64,
    /// Column start timestamp (ms since epoch)
    pub time_start: i64,
    /// Column end timestamp (ms since epoch)
    pub time_end: i64,
    /// Grid row index (0 = bottom row)
    pub row_index: i32,
    /// Grid column index (0 = current/leftmost visible column)
    pub col_index: i32,
    /// Current status of the position
    pub status: GridlineStatus,
    /// Actual P&L after resolution (positive for wins, None for active)
    pub result_pnl: Option<f64>,
    /// Timestamp when the position was resolved
    pub resolved_at: Option<i64>,
    /// Timestamp when the position was placed
    pub created_at: i64,
}

/// Configuration for a grid layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridConfig {
    /// Trading symbol
    pub symbol: String,
    /// Top price of the visible grid
    pub price_high: f64,
    /// Bottom price of the visible grid
    pub price_low: f64,
    /// Number of price rows in the grid
    pub row_count: u32,
    /// Number of time columns visible
    pub col_count: u32,
    /// Duration of each time column in milliseconds
    pub interval_ms: u64,
    /// Price range per row = (price_high - price_low) / row_count
    pub row_height: f64,
    /// Maximum allowed leverage
    pub max_leverage: f64,
}

/// Multiplier information for a single grid cell.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridMultiplier {
    /// Row index
    pub row: i32,
    /// Column index
    pub col: i32,
    /// Payout multiplier for this cell
    pub multiplier: f64,
    /// Estimated probability of price touching this cell
    pub probability: f64,
}

/// A price/time data point for sparkline history.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PricePoint {
    pub time: i64,
    pub price: f64,
}

/// Complete grid state for a symbol (sent to frontend).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridState {
    /// Current grid configuration
    pub config: GridConfig,
    /// Multiplier matrix [row][col]
    pub multipliers: Vec<Vec<f64>>,
    /// All active positions for the requesting portfolio
    pub active_positions: Vec<GridlinePosition>,
    /// Recently resolved positions (last 20)
    pub recent_results: Vec<GridlinePosition>,
    /// Recent price history for sparkline (timestamped)
    #[serde(default)]
    pub price_history: Vec<PricePoint>,
}

/// Session statistics for a portfolio's gridline trades on a symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridStats {
    pub portfolio_id: String,
    pub symbol: String,
    pub total_trades: i64,
    pub total_won: i64,
    pub total_lost: i64,
    pub total_wagered: f64,
    pub total_payout: f64,
    pub net_pnl: f64,
    pub best_multiplier_hit: f64,
    pub max_leverage_used: f64,
    pub updated_at: i64,
}

// =============================================================================
// Request / Response Types
// =============================================================================

/// Request to place a new gridline position.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceGridlineRequest {
    /// Portfolio placing the position
    pub portfolio_id: String,
    /// Trading symbol
    pub symbol: String,
    /// Position amount in USD (margin)
    pub amount: f64,
    /// Leverage (1.0 to 10.0)
    pub leverage: f64,
    /// Grid row index
    pub row_index: i32,
    /// Grid column index
    pub col_index: i32,
    /// Bottom of cell price range
    pub price_low: f64,
    /// Top of cell price range
    pub price_high: f64,
    /// Column start timestamp
    pub time_start: i64,
    /// Column end timestamp
    pub time_end: i64,
    /// Multiplier at time of placement
    pub multiplier: f64,
}

/// Result of a position resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridlineResolution {
    /// The resolved position
    pub position: GridlinePosition,
    /// Whether the position was won
    pub won: bool,
    /// Payout amount (if won)
    pub payout: Option<f64>,
    /// Net P&L (payout - margin, or -margin if lost)
    pub pnl: f64,
}

/// Request for grid config (optional overrides).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridConfigRequest {
    /// Override row count (default auto-calculated)
    pub row_count: Option<u32>,
    /// Override column count
    pub col_count: Option<u32>,
    /// Portfolio ID for fetching active positions
    pub portfolio_id: Option<String>,
}

// =============================================================================
// Error Type
// =============================================================================

/// Errors specific to gridline trading operations.
#[derive(Debug, thiserror::Error)]
pub enum GridlineError {
    #[error("Insufficient balance: need ${needed:.2}, available ${available:.2}")]
    InsufficientBalance { needed: f64, available: f64 },

    #[error("Maximum active trades reached ({max})")]
    MaxTradesReached { max: usize },

    #[error("Gridline trade not found: {0}")]
    TradeNotFound(String),

    #[error("Trade is not active (status: {0})")]
    TradeNotActive(String),

    #[error("Invalid leverage: {0} (must be 1.0-10.0)")]
    InvalidLeverage(f64),

    #[error("Invalid trade amount: {0} (must be > 0)")]
    InvalidAmount(f64),

    #[error("Invalid cell: row {row}, col {col}")]
    InvalidCell { row: i32, col: i32 },

    #[error("Column already expired")]
    ColumnExpired,

    #[error("Portfolio not found: {0}")]
    PortfolioNotFound(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Trading error: {0}")]
    TradingError(String),

    #[error("Rate limited: too many trades per second")]
    RateLimited,
}

impl From<rusqlite::Error> for GridlineError {
    fn from(e: rusqlite::Error) -> Self {
        GridlineError::DatabaseError(e.to_string())
    }
}


impl axum::response::IntoResponse for GridlineError {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;
        use axum::Json;

        let (status, code) = match &self {
            GridlineError::InsufficientBalance { .. } => (StatusCode::BAD_REQUEST, "INSUFFICIENT_BALANCE"),
            GridlineError::MaxTradesReached { .. } => (StatusCode::BAD_REQUEST, "MAX_TRADES_REACHED"),
            GridlineError::TradeNotFound(_) => (StatusCode::NOT_FOUND, "TRADE_NOT_FOUND"),
            GridlineError::TradeNotActive(_) => (StatusCode::BAD_REQUEST, "TRADE_NOT_ACTIVE"),
            GridlineError::InvalidLeverage(_) => (StatusCode::BAD_REQUEST, "INVALID_LEVERAGE"),
            GridlineError::InvalidAmount(_) => (StatusCode::BAD_REQUEST, "INVALID_AMOUNT"),
            GridlineError::InvalidCell { .. } => (StatusCode::BAD_REQUEST, "INVALID_CELL"),
            GridlineError::ColumnExpired => (StatusCode::BAD_REQUEST, "COLUMN_EXPIRED"),
            GridlineError::PortfolioNotFound(_) => (StatusCode::NOT_FOUND, "PORTFOLIO_NOT_FOUND"),
            GridlineError::DatabaseError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR"),
            GridlineError::TradingError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "TRADING_ERROR"),
            GridlineError::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED"),
        };

        let body = Json(serde_json::json!({
            "error": self.to_string(),
            "code": code,
        }));

        (status, body).into_response()
    }
}
