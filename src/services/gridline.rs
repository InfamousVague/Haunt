//! Gridline Trading Service
//!
//! Core engine for the Gridline Trading real-time prediction trading system.
//! Handles trade placement, resolution, multiplier calculation, and broadcasting.
//!
//! Users place trades on a price/time grid. If the real-time price line crosses
//! through a cell's price range during its time window, the trade wins.
//! Multipliers are derived from a log-normal price model using recent volatility.

use crate::services::SqliteStore;
use crate::types::{
    GridlinePosition, GridlineError, GridlineResolution, GridlineStatus, GridConfig, GridMultiplier,
    GridState, GridStats, PlaceGridlineRequest, PricePoint,
};
use crate::types::{
    GridlineTradePlacedData, GridlineTradeResolvedData, GridColumnExpiredData, GridMultiplierUpdateData,
    ServerMessage,
};
use crate::websocket::RoomManager;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tracing::{debug, info, warn};
use uuid::Uuid;

// =============================================================================
// Constants
// =============================================================================

/// Maximum number of active trades per portfolio
const MAX_ACTIVE_TRADES: usize = 10;

/// House edge (5%)
const HOUSE_EDGE: f64 = 0.05;

/// Minimum multiplier (ensures all cells pay at least 1.1x)
const MIN_MULTIPLIER: f64 = 1.1;

/// Maximum multiplier cap
const MAX_MULTIPLIER: f64 = 100.0;

/// Minimum probability floor (prevents infinite multipliers)
const MIN_PROBABILITY: f64 = 0.001;

/// Maximum probability ceiling
const MAX_PROBABILITY: f64 = 0.999;

/// Default annualized volatility fallback (100% — typical crypto)
const DEFAULT_VOLATILITY: f64 = 1.0;

/// Number of price ticks to keep for volatility calculation
const VOLATILITY_WINDOW: usize = 500;

/// Minimum trade amount in USD
const MIN_TRADE_AMOUNT: f64 = 0.10;

/// Maximum leverage multiplier
const MAX_LEVERAGE: f64 = 10.0;

/// Minimum leverage multiplier
const MIN_LEVERAGE: f64 = 1.0;

// =============================================================================
// Price Tick Buffer (for volatility)
// =============================================================================

/// A single price observation.
#[derive(Debug, Clone, Copy)]
struct PriceTick {
    price: f64,
    timestamp: i64,
}

/// Per-symbol rolling price buffer for volatility estimation.
#[derive(Debug)]
struct PriceBuffer {
    ticks: VecDeque<PriceTick>,
    max_size: usize,
}

impl PriceBuffer {
    fn new(max_size: usize) -> Self {
        Self {
            ticks: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    fn push(&mut self, price: f64, timestamp: i64) {
        if self.ticks.len() >= self.max_size {
            self.ticks.pop_front();
        }
        self.ticks.push_back(PriceTick { price, timestamp });
    }

    fn len(&self) -> usize {
        self.ticks.len()
    }

    /// Calculate annualized volatility from log returns.
    fn annualized_volatility(&self) -> f64 {
        if self.ticks.len() < 10 {
            return DEFAULT_VOLATILITY;
        }

        // Calculate log returns
        let mut log_returns = Vec::with_capacity(self.ticks.len() - 1);
        for i in 1..self.ticks.len() {
            let prev = self.ticks[i - 1].price;
            let curr = self.ticks[i].price;
            if prev > 0.0 && curr > 0.0 {
                log_returns.push((curr / prev).ln());
            }
        }

        if log_returns.is_empty() {
            return DEFAULT_VOLATILITY;
        }

        // Standard deviation of log returns
        let n = log_returns.len() as f64;
        let mean = log_returns.iter().sum::<f64>() / n;
        let variance = log_returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / n;
        let std_dev = variance.sqrt();

        // Estimate average time between ticks in seconds
        let first_ts = self.ticks.front().unwrap().timestamp;
        let last_ts = self.ticks.back().unwrap().timestamp;
        let total_span_ms = (last_ts - first_ts).max(1) as f64;
        let avg_interval_ms = total_span_ms / (self.ticks.len() - 1) as f64;

        // Annualize: vol_annual = std_dev * sqrt(ticks_per_year)
        let ms_per_year = 365.25 * 24.0 * 3600.0 * 1000.0;
        let ticks_per_year = ms_per_year / avg_interval_ms;
        let annualized = std_dev * ticks_per_year.sqrt();

        // Clamp to reasonable range
        annualized.clamp(0.01, 10.0)
    }

    /// Get the latest price, if any.
    fn latest_price(&self) -> Option<f64> {
        self.ticks.back().map(|t| t.price)
    }

    /// Get price history as (timestamp, price) pairs for a time range.
    fn prices_in_range(&self, start: i64, end: i64) -> Vec<(i64, f64)> {
        self.ticks
            .iter()
            .filter(|t| t.timestamp >= start && t.timestamp <= end)
            .map(|t| (t.timestamp, t.price))
            .collect()
    }

    /// Get the most recent N ticks as (timestamp, price) pairs.
    fn recent_ticks(&self, count: usize) -> Vec<(i64, f64)> {
        let start = if self.ticks.len() > count {
            self.ticks.len() - count
        } else {
            0
        };
        self.ticks
            .iter()
            .skip(start)
            .map(|t| (t.timestamp, t.price))
            .collect()
    }
}

// =============================================================================
// Gridline Trading Service
// =============================================================================

/// Core service for the Gridline Trading system.
pub struct GridlineService {
    /// SQLite store for position persistence
    sqlite: Arc<SqliteStore>,
    /// Room manager for WebSocket broadcasts
    room_manager: Option<Arc<RoomManager>>,
    /// Per-symbol rolling price buffers for volatility calculation
    price_buffers: Mutex<std::collections::HashMap<String, PriceBuffer>>,
}

impl GridlineService {
    /// Create a new GridlineService.
    pub fn new(sqlite: Arc<SqliteStore>, room_manager: Option<Arc<RoomManager>>) -> Self {
        info!("GridlineService initialized");
        Self {
            sqlite,
            room_manager,
            price_buffers: Mutex::new(std::collections::HashMap::new()),
        }
    }

    // =========================================================================
    // Core Operations
    // =========================================================================

    /// Place a new gridline trade.
    ///
    /// Validates: balance check done by caller (API layer with TradingService),
    /// max active trades, valid leverage, valid amount, valid cell, column not expired.
    pub fn place_trade(
        &self,
        mut req: PlaceGridlineRequest,
        available_balance: f64,
    ) -> Result<GridlinePosition, GridlineError> {
        let now = chrono::Utc::now().timestamp_millis();

        // Normalize symbol to uppercase for consistent matching with resolution engine
        req.symbol = req.symbol.to_uppercase();

        // Validate amount
        if req.amount <= 0.0 || req.amount < MIN_TRADE_AMOUNT {
            return Err(GridlineError::InvalidAmount(req.amount));
        }

        // Validate leverage
        if req.leverage < MIN_LEVERAGE || req.leverage > MAX_LEVERAGE {
            return Err(GridlineError::InvalidLeverage(req.leverage));
        }

        // Calculate effective amount (margin x leverage)
        let effective_amount = req.amount * req.leverage;

        // Validate balance (only the margin/amount is debited, not effective)
        if available_balance < req.amount {
            return Err(GridlineError::InsufficientBalance {
                needed: req.amount,
                available: available_balance,
            });
        }

        // Validate column not expired
        if req.time_end <= now {
            return Err(GridlineError::ColumnExpired);
        }

        // Reject trades where the column expires within the minimum time buffer.
        // This prevents last-second "sure thing" bets that exploit sub-1.0x multipliers.
        let min_buffer_ms: i64 = 2000; // 2 seconds minimum before column expiry
        if req.time_end - now < min_buffer_ms {
            return Err(GridlineError::ColumnExpired);
        }

        // Validate cell indices
        if req.row_index < 0 || req.col_index < 0 {
            return Err(GridlineError::InvalidCell {
                row: req.row_index,
                col: req.col_index,
            });
        }

        // Check max active trades
        let active_count = self.sqlite.count_active_gridline_positions(&req.portfolio_id);
        if active_count >= MAX_ACTIVE_TRADES as i64 {
            return Err(GridlineError::MaxTradesReached {
                max: MAX_ACTIVE_TRADES,
            });
        }

        // Calculate potential payout
        let potential_payout = effective_amount * req.multiplier;

        // Create the position
        let position = GridlinePosition {
            id: Uuid::new_v4().to_string(),
            portfolio_id: req.portfolio_id.clone(),
            symbol: req.symbol.clone(),
            amount: req.amount,
            leverage: req.leverage,
            effective_amount,
            multiplier: req.multiplier,
            potential_payout,
            price_low: req.price_low,
            price_high: req.price_high,
            time_start: req.time_start,
            time_end: req.time_end,
            row_index: req.row_index,
            col_index: req.col_index,
            status: GridlineStatus::Active,
            result_pnl: None,
            resolved_at: None,
            created_at: now,
        };

        // Persist
        self.sqlite.create_gridline_position(&position)?;

        // Broadcast
        self.broadcast_trade_placed(&position);

        info!(
            "Gridline trade placed: {} on {} row={} col={} amount=${:.2} leverage={:.1}x eff=${:.2} mult={:.2}x payout=${:.2}",
            position.id, position.symbol, position.row_index, position.col_index,
            position.amount, position.leverage, position.effective_amount, position.multiplier, position.potential_payout
        );

        Ok(position)
    }

    /// Look up a gridline position by ID (for pre-cancel ownership verification).
    pub fn cancel_trade_lookup(&self, position_id: &str) -> Option<GridlinePosition> {
        self.sqlite.get_gridline_position(position_id)
    }

    /// Cancel an active trade. Returns the cancelled position for refund processing.
    pub fn cancel_trade(
        &self,
        position_id: &str,
        portfolio_id: &str,
    ) -> Result<GridlinePosition, GridlineError> {
        let position = self
            .sqlite
            .get_gridline_position(position_id)
            .ok_or_else(|| GridlineError::TradeNotFound(position_id.to_string()))?;

        // Verify ownership
        if position.portfolio_id != portfolio_id {
            return Err(GridlineError::TradeNotFound(position_id.to_string()));
        }

        // Must be active
        if position.status != GridlineStatus::Active {
            return Err(GridlineError::TradeNotActive(position.status.to_string()));
        }

        // Cancel in DB
        self.sqlite.cancel_gridline_position(position_id)?;

        // Return updated position
        let mut cancelled = position;
        cancelled.status = GridlineStatus::Cancelled;

        // Broadcast resolution
        self.broadcast_trade_resolved(&cancelled, false, None, 0.0);

        info!("Gridline trade cancelled: {} refund=${:.2}", position_id, cancelled.amount);

        Ok(cancelled)
    }

    /// Resolve a single trade as won or lost.
    fn resolve_trade_internal(
        &self,
        position: &GridlinePosition,
        won: bool,
    ) -> Result<GridlineResolution, GridlineError> {
        let (pnl, payout) = if won {
            // Win: payout = effective_amount * multiplier, pnl = payout - margin
            let payout = position.effective_amount * position.multiplier;
            let pnl = payout - position.amount;
            (pnl, Some(payout))
        } else {
            // Loss: lose the margin (amount)
            let pnl = -position.amount;
            (pnl, None)
        };

        let status = if won {
            GridlineStatus::Won
        } else {
            GridlineStatus::Lost
        };

        // Update in DB
        self.sqlite
            .resolve_gridline_position(&position.id, status, pnl)?;

        // Build resolved position
        let mut resolved_position = position.clone();
        resolved_position.status = status;
        resolved_position.result_pnl = Some(pnl);
        resolved_position.resolved_at = Some(chrono::Utc::now().timestamp_millis());

        let resolution = GridlineResolution {
            position: resolved_position.clone(),
            won,
            payout,
            pnl,
        };

        // Broadcast
        self.broadcast_trade_resolved(&resolved_position, won, payout, pnl);

        Ok(resolution)
    }

    // =========================================================================
    // Grid State
    // =========================================================================

    /// Get the current grid state for a symbol + portfolio.
    pub fn get_grid_state(
        &self,
        portfolio_id: Option<&str>,
        symbol: &str,
        config: &GridConfig,
    ) -> Result<GridState, GridlineError> {
        let current_price = self.get_latest_price(symbol).unwrap_or(0.0);
        let multipliers = self.calculate_multipliers(symbol, current_price, config);

        let active_positions = if let Some(pid) = portfolio_id {
            self.sqlite.get_active_gridline_positions_for_portfolio_symbol(pid, symbol)
        } else {
            Vec::new()
        };

        let recent_results = if let Some(pid) = portfolio_id {
            self.sqlite.get_gridline_history(pid, 20, 0)
        } else {
            Vec::new()
        };

        // Get recent price ticks for sparkline pre-population (up to 200 points)
        let price_history = self.get_price_history(symbol, 200);

        Ok(GridState {
            config: config.clone(),
            multipliers,
            active_positions,
            recent_results,
            price_history,
        })
    }

    /// Build a GridConfig for a symbol based on current price and volatility.
    ///
    /// The grid is designed for a fast-paced, dramatic trading experience:
    /// - Short column intervals (10s default) for rapid scrolling
    /// - Tight price range so small price movements cause visible dot movement
    /// - Configurable row/col count for different grid densities
    pub fn build_config(
        &self,
        symbol: &str,
        current_price: f64,
        row_count: Option<u32>,
        col_count: Option<u32>,
    ) -> GridConfig {
        let rows = row_count.unwrap_or(8);
        let cols = col_count.unwrap_or(6);
        let volatility = self.get_volatility(symbol);
        let interval_ms = self.get_optimal_interval(volatility);

        // Grid price range: use a tight span around current price for dramatic
        // dot movement. The span is based on expected movement over one column
        // interval, scaled so that a typical price tick moves the dot ~1-2 rows.
        //
        // With 36 rows visible and 12 rows on screen, we want ~0.05% of price
        // per visible row, so the full grid (36 rows) spans ~1.8% of price.
        // This makes individual ticks visually significant.
        let sensitivity = 8.0; // Tightening factor for dramatic grid movement
        let total_time_ms = interval_ms as f64 * cols as f64;
        let t_years = total_time_ms / (365.25 * 24.0 * 3600.0 * 1000.0);
        let expected_move = current_price * volatility * t_years.sqrt();

        // Compress the span for a tighter, more responsive grid
        let grid_half_span = ((expected_move * 3.0) / sensitivity).max(current_price * 0.0006);
        let price_high = current_price + grid_half_span;
        let price_low = (current_price - grid_half_span).max(0.01);
        let row_height = (price_high - price_low) / rows as f64;

        GridConfig {
            symbol: symbol.to_string(),
            price_high,
            price_low,
            row_count: rows,
            col_count: cols,
            interval_ms,
            row_height,
            max_leverage: MAX_LEVERAGE,
        }
    }

    // =========================================================================
    // Multiplier Engine
    // =========================================================================

    /// Calculate the multiplier matrix for a grid.
    ///
    /// Returns a `[row][col]` matrix of payout multipliers.
    ///
    /// Uses a difficulty-based model with time decay:
    /// - Price distance from current price determines base difficulty
    /// - Time factor: columns near the dot (about to expire) get sub-1.0x
    ///   multipliers via time decay, preventing last-minute "sure thing" bets
    /// - Difficulty maps to multiplier through a smooth curve:
    ///   easy (near price) = 1.1x, hard (far from price) = 50x+
    /// - House edge applied as a flat percentage reduction
    ///
    /// The time decay is the key anti-cheat mechanism: betting on a cell that
    /// the price is already sitting in but is about to close pays < 1.0x,
    /// meaning the user loses money on guaranteed-looking bets.
    pub fn calculate_multipliers(
        &self,
        symbol: &str,
        current_price: f64,
        config: &GridConfig,
    ) -> Vec<Vec<f64>> {
        let house_edge_factor = 1.0 - HOUSE_EDGE; // e.g., 0.95
        let half_span = (config.price_high - config.price_low) / 2.0;

        let mut matrix = Vec::with_capacity(config.row_count as usize);

        for row in 0..config.row_count as i32 {
            let mut row_multipliers = Vec::with_capacity(config.col_count as usize);

            // Price at center of this row
            let row_price = config.price_high - (row as f64 + 0.5) * config.row_height;
            // Normalized distance from current price (0 = at price, 1 = at grid edge)
            let price_dist = if half_span > 0.0 {
                (row_price - current_price).abs() / half_span
            } else {
                0.0
            };

            for col in 0..config.col_count as i32 {
                // Time fraction: column 0 is soonest (about to close), last is furthest
                let time_fraction = (col as f64 + 1.0) / config.col_count as f64;

                // Time decay: columns near the dot (low col) get a penalty < 1.0
                // This prevents last-minute "sure thing" bets from being profitable.
                // Column 0: ~0.3x, Column 1: ~0.5x, ramps to 1.0 by mid-grid
                let time_decay = 1.0_f64.min(time_fraction.powf(0.4));

                // Combined difficulty: higher distance + shorter time = harder
                let difficulty = price_dist / time_fraction.sqrt();

                // Map difficulty to multiplier via smooth piecewise curve
                let mut mult = if difficulty < 0.1 {
                    1.1 + difficulty * 8.0           // 1.1x to 1.9x
                } else if difficulty < 0.4 {
                    1.9 + (difficulty - 0.1) * 20.0  // 1.9x to 7.9x
                } else if difficulty < 0.8 {
                    7.9 + (difficulty - 0.4) * 30.0  // 7.9x to 19.9x
                } else {
                    19.9 + (difficulty - 0.8) * 75.0 // 19.9x to 50x+
                };

                // Apply house edge and time decay
                // Near-dot columns: time_decay < 1.0, so mult drops below 1.0x (user loses money)
                mult = mult * house_edge_factor * time_decay;
                mult = mult.clamp(0.1, MAX_MULTIPLIER);

                // Round to 2 decimal places for clean display
                mult = (mult * 100.0).round() / 100.0;

                row_multipliers.push(mult);
            }

            matrix.push(row_multipliers);
        }

        matrix
    }

    /// Calculate individual cell multiplier info (for tooltips).
    pub fn get_cell_multiplier(
        &self,
        symbol: &str,
        current_price: f64,
        cell_price_mid: f64,
        time_ahead_ms: u64,
    ) -> GridMultiplier {
        let volatility = self.get_volatility(symbol);
        let probability = Self::estimate_probability(
            current_price,
            cell_price_mid,
            time_ahead_ms,
            volatility,
        );
        let multiplier = Self::calculate_multiplier(probability);

        GridMultiplier {
            row: 0,
            col: 0,
            multiplier,
            probability,
        }
    }

    /// Estimate probability that price touches a level using log-normal model.
    ///
    /// Uses the reflection principle for barrier option pricing:
    /// P(touch) = 2 * N(-d) where d = |ln(target/current)| / (sigma * sqrt(t))
    fn estimate_probability(
        current_price: f64,
        cell_price_mid: f64,
        time_ahead_ms: u64,
        volatility: f64,
    ) -> f64 {
        // Convert time to years
        let t = time_ahead_ms as f64 / (365.25 * 24.0 * 3600.0 * 1000.0);
        let sigma_sqrt_t = volatility * t.sqrt();

        // Edge case: near-zero volatility or time
        if sigma_sqrt_t < 1e-10 {
            return if (current_price - cell_price_mid).abs() / current_price < 0.001 {
                MAX_PROBABILITY
            } else {
                MIN_PROBABILITY
            };
        }

        // Edge case: zero or negative prices
        if current_price <= 0.0 || cell_price_mid <= 0.0 {
            return MIN_PROBABILITY;
        }

        // Log distance
        let log_ratio = (cell_price_mid / current_price).ln();
        let d = log_ratio.abs() / sigma_sqrt_t;

        // P(touch) = 2 * N(-d) using reflection principle
        let p_touch = 2.0 * normal_cdf(-d);

        p_touch.clamp(MIN_PROBABILITY, MAX_PROBABILITY)
    }

    /// Convert probability to payout multiplier with house edge.
    ///
    /// multiplier = (1 / probability) * (1 - house_edge)
    fn calculate_multiplier(probability: f64) -> f64 {
        let raw = 1.0 / probability;
        let with_edge = raw * (1.0 - HOUSE_EDGE);
        with_edge.clamp(MIN_MULTIPLIER, MAX_MULTIPLIER)
    }

    // =========================================================================
    // Resolution Engine
    // =========================================================================

    /// Called on each price tick. Records price and checks for trade resolutions.
    ///
    /// Returns all resolutions (wins and losses) triggered by this price update.
    pub fn on_price_update(
        &self,
        symbol: &str,
        price: f64,
        timestamp: i64,
    ) -> Vec<GridlineResolution> {
        // Record the tick for volatility
        self.record_price_tick(symbol, price, timestamp);

        let mut resolutions = Vec::new();

        // Get all active positions for this symbol
        let active_positions = self.sqlite.get_active_gridline_positions_for_symbol(symbol);

        // Time tolerance: allow ±2 seconds to account for clock skew between
        // client (which computes time_start/time_end) and server price timestamps.
        let time_tolerance_ms: i64 = 2000;

        for position in &active_positions {
            let price_in_range = price >= position.price_low && price <= position.price_high;
            let time_in_window = timestamp >= position.time_start - time_tolerance_ms
                && timestamp <= position.time_end + time_tolerance_ms;

            // Check if price is within this position's price range AND time window
            if price_in_range && time_in_window {
                // WIN — price crossed through the cell
                match self.resolve_trade_internal(position, true) {
                    Ok(resolution) => {
                        info!(
                            "Gridline trade WON: {} on {} payout=${:.2} pnl=${:.2}",
                            position.id, position.symbol,
                            resolution.payout.unwrap_or(0.0),
                            resolution.pnl
                        );
                        resolutions.push(resolution);
                    }
                    Err(e) => {
                        warn!("Failed to resolve winning trade {}: {}", position.id, e);
                    }
                }
            } else {
                // Near-miss debug logging for diagnostics
                if price_in_range && !time_in_window {
                    debug!(
                        "NEAR-MISS time: pos={} price={:.2} ts={} window=[{}..{}] delta_start={}ms delta_end={}ms",
                        position.id, price, timestamp,
                        position.time_start, position.time_end,
                        timestamp - position.time_start,
                        position.time_end - timestamp
                    );
                }
                if time_in_window && !price_in_range {
                    let gap = if price < position.price_low {
                        position.price_low - price
                    } else {
                        price - position.price_high
                    };
                    debug!(
                        "NEAR-MISS price: pos={} price={:.2} range=[{:.2}..{:.2}] gap={:.6}",
                        position.id, price, position.price_low, position.price_high, gap
                    );
                }
            }
        }

        // Check for expired columns (time_end < current timestamp, still active)
        let expired_positions = self.sqlite.get_expired_unresolved_positions(symbol, timestamp);

        for position in &expired_positions {
            // LOSS — column expired without price reaching cell
            match self.resolve_trade_internal(position, false) {
                Ok(resolution) => {
                    debug!(
                        "Gridline trade LOST: {} on {} loss=${:.2}",
                        position.id, position.symbol, position.amount
                    );
                    resolutions.push(resolution);
                }
                Err(e) => {
                    warn!("Failed to resolve losing trade {}: {}", position.id, e);
                }
            }
        }

        resolutions
    }

    /// Resolve all positions in a specific time column. Used for batch resolution.
    pub fn resolve_column(
        &self,
        symbol: &str,
        col_time_start: i64,
        col_time_end: i64,
    ) -> Vec<GridlineResolution> {
        let mut resolutions = Vec::new();

        // Get price history within this column's time window
        let prices = {
            let buffers = self.price_buffers.lock().unwrap();
            buffers
                .get(symbol)
                .map(|b| b.prices_in_range(col_time_start, col_time_end))
                .unwrap_or_default()
        };

        // Get all active positions in this time window
        let active_positions = self.sqlite.get_active_gridline_positions_for_symbol(symbol);

        let column_positions: Vec<_> = active_positions
            .into_iter()
            .filter(|b| b.time_start >= col_time_start && b.time_end <= col_time_end)
            .collect();

        for position in &column_positions {
            // Check if any price tick crossed through this cell
            let won = prices
                .iter()
                .any(|(_, p)| *p >= position.price_low && *p <= position.price_high);

            match self.resolve_trade_internal(position, won) {
                Ok(resolution) => resolutions.push(resolution),
                Err(e) => warn!("Failed to resolve trade {}: {}", position.id, e),
            }
        }

        // Broadcast column expired event
        if !resolutions.is_empty() {
            self.broadcast_column_expired(symbol, col_time_end, &resolutions);
        }

        resolutions
    }

    // =========================================================================
    // Volatility & Interval Calculation
    // =========================================================================

    /// Record a price tick for volatility tracking.
    pub fn record_price_tick(&self, symbol: &str, price: f64, timestamp: i64) {
        let mut buffers = self.price_buffers.lock().unwrap();
        let buffer = buffers
            .entry(symbol.to_string())
            .or_insert_with(|| PriceBuffer::new(VOLATILITY_WINDOW));
        buffer.push(price, timestamp);
    }

    /// Get the latest recorded price for a symbol.
    pub fn get_latest_price(&self, symbol: &str) -> Option<f64> {
        let buffers = self.price_buffers.lock().unwrap();
        buffers.get(symbol).and_then(|b| b.latest_price())
    }

    /// Get recent price history for a symbol (for sparkline pre-population).
    pub fn get_price_history(&self, symbol: &str, max_points: usize) -> Vec<PricePoint> {
        let buffers = self.price_buffers.lock().unwrap();
        buffers
            .get(symbol)
            .map(|b| {
                b.recent_ticks(max_points)
                    .into_iter()
                    .map(|(time, price)| PricePoint { time, price })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get recent annualized volatility for a symbol.
    pub fn get_volatility(&self, symbol: &str) -> f64 {
        let buffers = self.price_buffers.lock().unwrap();
        buffers
            .get(symbol)
            .map(|b| b.annualized_volatility())
            .unwrap_or(DEFAULT_VOLATILITY)
    }

    /// Get the number of recorded ticks for a symbol.
    pub fn get_tick_count(&self, symbol: &str) -> usize {
        let buffers = self.price_buffers.lock().unwrap();
        buffers.get(symbol).map(|b| b.len()).unwrap_or(0)
    }

    /// Determine optimal time column interval based on volatility.
    ///
    /// Higher volatility -> shorter intervals (faster-paced trading).
    /// Target: fast-paced columns (10s default) for a dramatic, game-like feel.
    /// The tight price span from build_config() ensures enough dot movement
    /// even with short intervals.
    pub fn get_optimal_interval(&self, annualized_vol: f64) -> u64 {
        let vol_per_ms = annualized_vol / (365.25 * 24.0 * 3600.0 * 1000.0_f64).sqrt();
        let target = 0.0003; // 0.03% target movement per column (tighter for fast columns)

        if vol_per_ms < 1e-15 {
            return 10_000; // Default 10 seconds
        }

        let interval_ms = (target / vol_per_ms).powi(2);

        // Clamp between 5 seconds and 15 seconds for fast-paced gameplay
        (interval_ms as u64).clamp(5_000, 15_000)
    }

    // =========================================================================
    // Stats
    // =========================================================================

    /// Get gridline stats for a portfolio + symbol.
    pub fn get_stats(
        &self,
        portfolio_id: &str,
        symbol: &str,
    ) -> Option<GridStats> {
        self.sqlite.get_gridline_stats(portfolio_id, symbol)
    }

    /// Update stats after a trade is resolved.
    pub fn update_stats_after_resolution(
        &self,
        portfolio_id: &str,
        symbol: &str,
        resolution: &GridlineResolution,
    ) -> Result<(), GridlineError> {
        let now = chrono::Utc::now().timestamp_millis();

        // Get or create stats
        let mut stats = self
            .sqlite
            .get_gridline_stats(portfolio_id, symbol)
            .unwrap_or(GridStats {
                portfolio_id: portfolio_id.to_string(),
                symbol: symbol.to_string(),
                total_trades: 0,
                total_won: 0,
                total_lost: 0,
                total_wagered: 0.0,
                total_payout: 0.0,
                net_pnl: 0.0,
                best_multiplier_hit: 0.0,
                max_leverage_used: 0.0,
                updated_at: now,
            });

        stats.total_trades += 1;
        stats.total_wagered += resolution.position.amount;

        if resolution.won {
            stats.total_won += 1;
            if let Some(payout) = resolution.payout {
                stats.total_payout += payout;
            }
            if resolution.position.multiplier > stats.best_multiplier_hit {
                stats.best_multiplier_hit = resolution.position.multiplier;
            }
        } else {
            stats.total_lost += 1;
        }

        stats.net_pnl += resolution.pnl;

        if resolution.position.leverage > stats.max_leverage_used {
            stats.max_leverage_used = resolution.position.leverage;
        }

        stats.updated_at = now;

        self.sqlite.update_gridline_stats(&stats)?;
        Ok(())
    }

    // =========================================================================
    // WebSocket Broadcasting
    // =========================================================================

    /// Broadcast that a trade was placed.
    fn broadcast_trade_placed(&self, position: &GridlinePosition) {
        if let Some(ref room_manager) = self.room_manager {
            let data = GridlineTradePlacedData {
                position: position.clone(),
                timestamp: chrono::Utc::now().timestamp_millis(),
            };
            let msg = ServerMessage::GridlineTradePlaced { data };
            if let Ok(json) = serde_json::to_string(&msg) {
                room_manager.broadcast_trading(&position.portfolio_id, &json);
            }
        }
    }

    /// Broadcast that a trade was resolved (won, lost, or cancelled).
    fn broadcast_trade_resolved(
        &self,
        position: &GridlinePosition,
        won: bool,
        payout: Option<f64>,
        pnl: f64,
    ) {
        if let Some(ref room_manager) = self.room_manager {
            let data = GridlineTradeResolvedData {
                position: position.clone(),
                won,
                payout,
                pnl,
                timestamp: chrono::Utc::now().timestamp_millis(),
            };
            let msg = ServerMessage::GridlineTradeResolved { data };
            if let Ok(json) = serde_json::to_string(&msg) {
                room_manager.broadcast_trading(&position.portfolio_id, &json);
            }
        }
    }

    /// Broadcast multiplier matrix update to all gridline subscribers of a symbol.
    pub fn broadcast_multiplier_update(
        &self,
        symbol: &str,
        current_price: f64,
        multipliers: &[Vec<f64>],
        config: &GridConfig,
    ) {
        if let Some(ref room_manager) = self.room_manager {
            let data = GridMultiplierUpdateData {
                symbol: symbol.to_string(),
                multipliers: multipliers.to_vec(),
                config: config.clone(),
                current_price,
                timestamp: chrono::Utc::now().timestamp_millis(),
            };
            let msg = ServerMessage::GridMultiplierUpdate { data };
            if let Ok(json) = serde_json::to_string(&msg) {
                room_manager.broadcast_gridline(symbol, &json);
            }
        }
    }

    /// Broadcast column expired event with batch results.
    fn broadcast_column_expired(
        &self,
        symbol: &str,
        time_end: i64,
        resolutions: &[GridlineResolution],
    ) {
        if let Some(ref room_manager) = self.room_manager {
            let results: Vec<GridlineTradeResolvedData> = resolutions
                .iter()
                .map(|r| GridlineTradeResolvedData {
                    position: r.position.clone(),
                    won: r.won,
                    payout: r.payout,
                    pnl: r.pnl,
                    timestamp: chrono::Utc::now().timestamp_millis(),
                })
                .collect();

            let data = GridColumnExpiredData {
                symbol: symbol.to_string(),
                col_index: 0, // Column index relative to current view
                time_end,
                results,
                timestamp: chrono::Utc::now().timestamp_millis(),
            };
            let msg = ServerMessage::GridColumnExpired { data };
            if let Ok(json) = serde_json::to_string(&msg) {
                // Broadcast to each affected portfolio
                for resolution in resolutions {
                    room_manager.broadcast_trading(&resolution.position.portfolio_id, &json);
                }
            }
        }
    }
}

// =============================================================================
// Math Helpers
// =============================================================================

/// Standard normal CDF approximation using Abramowitz & Stegun formula.
/// Accurate to about 1.5e-7.
fn normal_cdf(x: f64) -> f64 {
    if x < -8.0 {
        return 0.0;
    }
    if x > 8.0 {
        return 1.0;
    }

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();

    // Abramowitz & Stegun approximation 26.2.17
    let b1 = 0.319381530;
    let b2 = -0.356563782;
    let b3 = 1.781477937;
    let b4 = -1.821255978;
    let b5 = 1.330274429;
    let p = 0.2316419;

    let t = 1.0 / (1.0 + p * x);
    let t2 = t * t;
    let t3 = t2 * t;
    let t4 = t3 * t;
    let t5 = t4 * t;

    let pdf = (-x * x / 2.0).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let cdf = 1.0 - pdf * (b1 * t + b2 * t2 + b3 * t3 + b4 * t4 + b5 * t5);

    0.5 * (1.0 + sign * (2.0 * cdf - 1.0))
}
