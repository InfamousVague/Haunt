//! Random Auto Trader (RAT) types.
//!
//! Types for the automated random trading system used for testing and
//! simulating realistic trading activity.

use serde::{Deserialize, Serialize};

/// RAT configuration for a portfolio.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RatConfig {
    /// Unique RAT config ID.
    pub id: String,
    /// Portfolio this RAT operates on.
    pub portfolio_id: String,
    /// Whether RAT is currently enabled.
    pub enabled: bool,
    /// Trade interval in seconds (how often to attempt trades).
    pub trade_interval_secs: u64,
    /// Maximum concurrent open positions.
    pub max_open_positions: u32,
    /// Symbols available for trading (empty = all available).
    pub symbols: Vec<String>,
    /// Minimum position hold time in seconds before closing.
    pub min_hold_time_secs: u64,
    /// Position size range as percentage of available margin (min, max).
    pub size_range_pct: (f64, f64),
    /// Probability of setting stop loss (0.0 - 1.0).
    pub stop_loss_probability: f64,
    /// Probability of setting take profit (0.0 - 1.0).
    pub take_profit_probability: f64,
    /// Stop loss distance range (percentage from entry).
    pub stop_loss_range_pct: (f64, f64),
    /// Take profit distance range (percentage from entry).
    pub take_profit_range_pct: (f64, f64),
    /// Created timestamp.
    pub created_at: i64,
    /// Last updated timestamp.
    pub updated_at: i64,
}

impl Default for RatConfig {
    fn default() -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            portfolio_id: String::new(),
            enabled: false,
            trade_interval_secs: 60,
            max_open_positions: 5,
            symbols: vec![],
            min_hold_time_secs: 30,
            size_range_pct: (0.05, 0.15),
            stop_loss_probability: 0.7,
            take_profit_probability: 0.6,
            stop_loss_range_pct: (0.02, 0.05),
            take_profit_range_pct: (0.03, 0.08),
            created_at: now,
            updated_at: now,
        }
    }
}

impl RatConfig {
    /// Create a new RAT config for a portfolio with default settings.
    pub fn new(portfolio_id: String) -> Self {
        Self {
            portfolio_id,
            ..Default::default()
        }
    }
}

/// RAT runtime statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RatStats {
    /// Unique stats ID.
    pub id: String,
    /// Portfolio ID.
    pub portfolio_id: String,
    /// Total number of trades executed.
    pub total_trades: u64,
    /// Number of winning trades.
    pub winning_trades: u64,
    /// Number of losing trades.
    pub losing_trades: u64,
    /// Total P&L across all trades.
    pub total_pnl: f64,
    /// Number of errors encountered.
    pub errors: u32,
    /// Timestamp of last trade (if any).
    pub last_trade_at: Option<i64>,
    /// Timestamp when RAT was started (if running).
    pub started_at: Option<i64>,
    /// Last updated timestamp.
    pub updated_at: i64,
}

impl RatStats {
    /// Create new stats for a portfolio.
    pub fn new(portfolio_id: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            portfolio_id,
            updated_at: chrono::Utc::now().timestamp_millis(),
            ..Default::default()
        }
    }

    /// Calculate win rate as a percentage.
    pub fn win_rate(&self) -> f64 {
        if self.total_trades == 0 {
            return 0.0;
        }
        (self.winning_trades as f64 / self.total_trades as f64) * 100.0
    }

    /// Record a winning trade.
    pub fn record_win(&mut self, pnl: f64) {
        self.total_trades += 1;
        self.winning_trades += 1;
        self.total_pnl += pnl;
        self.last_trade_at = Some(chrono::Utc::now().timestamp_millis());
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Record a losing trade.
    pub fn record_loss(&mut self, pnl: f64) {
        self.total_trades += 1;
        self.losing_trades += 1;
        self.total_pnl += pnl;
        self.last_trade_at = Some(chrono::Utc::now().timestamp_millis());
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Record an error.
    pub fn record_error(&mut self) {
        self.errors += 1;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }
}

/// RAT operational status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RatStatus {
    /// RAT is not running.
    Idle,
    /// RAT is actively trading.
    Active,
    /// RAT encountered an error.
    Error { message: String },
    /// RAT is in the process of stopping.
    Stopping,
}

impl Default for RatStatus {
    fn default() -> Self {
        Self::Idle
    }
}

/// Combined RAT state for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RatState {
    /// Current configuration.
    pub config: RatConfig,
    /// Runtime statistics.
    pub stats: RatStats,
    /// Current operational status.
    pub status: RatStatus,
    /// Current number of open positions.
    pub open_positions: u32,
}

/// Request to start RAT.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRatRequest {
    /// Portfolio ID to start RAT for.
    pub portfolio_id: String,
    /// Optional configuration overrides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<RatConfigUpdate>,
}

/// Request to stop RAT.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StopRatRequest {
    /// Portfolio ID to stop RAT for.
    pub portfolio_id: String,
}

/// Request to update RAT configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RatConfigUpdate {
    /// Trade interval in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trade_interval_secs: Option<u64>,
    /// Maximum concurrent open positions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_open_positions: Option<u32>,
    /// Symbols available for trading.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbols: Option<Vec<String>>,
    /// Minimum position hold time in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_hold_time_secs: Option<u64>,
    /// Position size range as percentage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_range_pct: Option<(f64, f64)>,
    /// Probability of setting stop loss.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_loss_probability: Option<f64>,
    /// Probability of setting take profit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub take_profit_probability: Option<f64>,
    /// Stop loss distance range.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_loss_range_pct: Option<(f64, f64)>,
    /// Take profit distance range.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub take_profit_range_pct: Option<(f64, f64)>,
}

impl RatConfigUpdate {
    /// Apply updates to a config.
    pub fn apply_to(&self, config: &mut RatConfig) {
        if let Some(v) = self.trade_interval_secs {
            config.trade_interval_secs = v;
        }
        if let Some(v) = self.max_open_positions {
            config.max_open_positions = v;
        }
        if let Some(v) = &self.symbols {
            config.symbols = v.clone();
        }
        if let Some(v) = self.min_hold_time_secs {
            config.min_hold_time_secs = v;
        }
        if let Some(v) = self.size_range_pct {
            config.size_range_pct = v;
        }
        if let Some(v) = self.stop_loss_probability {
            config.stop_loss_probability = v;
        }
        if let Some(v) = self.take_profit_probability {
            config.take_profit_probability = v;
        }
        if let Some(v) = self.stop_loss_range_pct {
            config.stop_loss_range_pct = v;
        }
        if let Some(v) = self.take_profit_range_pct {
            config.take_profit_range_pct = v;
        }
        config.updated_at = chrono::Utc::now().timestamp_millis();
    }
}

/// Action decided by RAT trading logic.
#[derive(Debug, Clone)]
pub enum RatAction {
    /// Open a new position.
    OpenPosition {
        symbol: String,
        side: super::OrderSide,
        size: f64,
        stop_loss: Option<f64>,
        take_profit: Option<f64>,
    },
    /// Close an existing position.
    ClosePosition {
        position_id: String,
    },
    /// Modify an existing position's SL/TP.
    ModifyPosition {
        position_id: String,
        new_stop_loss: Option<f64>,
        new_take_profit: Option<f64>,
    },
    /// Skip this iteration (do nothing).
    Skip,
}
