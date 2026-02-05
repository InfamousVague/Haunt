//! Random Auto Trader (RAT) Service
//!
//! Provides automated random trading for testing and simulation purposes.
//! Generates random trades at configurable intervals to populate portfolios
//! with realistic trading activity.

use crate::services::{PriceCache, SqliteStore, TradingService};
use crate::types::{
    AssetClass, OrderSide, OrderType, PlaceOrderRequest, Position, RatAction, RatConfig,
    RatConfigUpdate, RatState, RatStats, RatStatus, ServerMessage,
};
use crate::websocket::RoomManager;
use dashmap::DashMap;
use rand::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// RAT service errors.
#[derive(Debug, Error)]
pub enum RatError {
    #[error("Portfolio not found: {0}")]
    PortfolioNotFound(String),

    #[error("RAT already running for portfolio: {0}")]
    AlreadyRunning(String),

    #[error("RAT not running for portfolio: {0}")]
    NotRunning(String),

    #[error("Trading error: {0}")]
    TradingError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("No price data available")]
    NoPriceData,
}

/// Active RAT instance tracking.
struct ActiveRat {
    handle: JoinHandle<()>,
    cancel: Arc<AtomicBool>,
}

/// Random Auto Trader service.
pub struct RatService {
    /// SQLite store for persistence.
    sqlite: Arc<SqliteStore>,
    /// Trading service for executing orders.
    trading: Arc<TradingService>,
    /// Price cache for current prices.
    price_cache: Arc<PriceCache>,
    /// Room manager for WebSocket broadcasts.
    room_manager: Option<Arc<RoomManager>>,
    /// Active RAT instances (portfolio_id -> active instance).
    active_rats: DashMap<String, ActiveRat>,
    /// Runtime stats (portfolio_id -> stats).
    stats: DashMap<String, RatStats>,
    /// Configs (portfolio_id -> config).
    configs: DashMap<String, RatConfig>,
    /// Current status (portfolio_id -> status).
    statuses: DashMap<String, RatStatus>,
    /// Default symbols if none configured.
    default_symbols: Vec<String>,
}

impl RatService {
    /// Create a new RAT service.
    pub fn new(
        sqlite: Arc<SqliteStore>,
        trading: Arc<TradingService>,
        price_cache: Arc<PriceCache>,
    ) -> Arc<Self> {
        Arc::new(Self {
            sqlite,
            trading,
            price_cache,
            room_manager: None,
            active_rats: DashMap::new(),
            stats: DashMap::new(),
            configs: DashMap::new(),
            statuses: DashMap::new(),
            default_symbols: vec![
                "BTC".to_string(),
                "ETH".to_string(),
                "SOL".to_string(),
                "XRP".to_string(),
                "ADA".to_string(),
                "DOGE".to_string(),
                "AVAX".to_string(),
                "DOT".to_string(),
                "LINK".to_string(),
                "MATIC".to_string(),
            ],
        })
    }

    /// Set room manager for WebSocket broadcasts.
    pub fn set_room_manager(&mut self, room_manager: Arc<RoomManager>) {
        self.room_manager = Some(room_manager);
    }

    /// Load persisted configs and stats from database.
    pub fn load_from_database(self: &Arc<Self>) {
        info!("Loading RAT configs from database");

        // Load all configs (enabled or not)
        let enabled_configs = self.sqlite.get_enabled_rat_configs();
        for config in enabled_configs {
            let portfolio_id = config.portfolio_id.clone();

            // Load associated stats
            let stats = self
                .sqlite
                .get_rat_stats(&portfolio_id)
                .unwrap_or_else(|| RatStats::new(portfolio_id.clone()));

            self.configs.insert(portfolio_id.clone(), config);
            self.stats.insert(portfolio_id.clone(), stats);
            self.statuses.insert(portfolio_id, RatStatus::Idle);
        }

        info!(
            "Loaded {} RAT configs from database",
            self.configs.len()
        );
    }

    /// Auto-start previously enabled RATs.
    pub fn auto_start_enabled(self: Arc<Self>) {
        let enabled: Vec<String> = self
            .configs
            .iter()
            .filter(|e| e.value().enabled)
            .map(|e| e.key().clone())
            .collect();

        for portfolio_id in enabled {
            info!("Auto-starting RAT for portfolio {}", portfolio_id);
            if let Err(e) = self.clone().start_internal(&portfolio_id) {
                error!("Failed to auto-start RAT for {}: {}", portfolio_id, e);
            }
        }
    }

    /// Get current RAT state for a portfolio.
    /// Creates a default config if none exists (allows settings to be changed before first start).
    pub fn get_state(&self, portfolio_id: &str) -> Option<RatState> {
        // Get or create config for this portfolio
        let config = self.get_or_create_config(portfolio_id);

        let stats = self
            .stats
            .get(portfolio_id)
            .map(|s| s.clone())
            .unwrap_or_else(|| RatStats::new(portfolio_id.to_string()));
        let status = self
            .statuses
            .get(portfolio_id)
            .map(|s| s.clone())
            .unwrap_or(RatStatus::Idle);

        // Get current open positions count from trading service
        let open_positions = self.trading.get_positions(portfolio_id).len() as u32;

        Some(RatState {
            config,
            stats,
            status,
            open_positions,
        })
    }

    /// Get existing config or create a default one for the portfolio.
    fn get_or_create_config(&self, portfolio_id: &str) -> RatConfig {
        // Check in-memory cache first
        if let Some(config) = self.configs.get(portfolio_id) {
            return config.clone();
        }

        // Check database
        if let Some(config) = self.sqlite.get_rat_config(portfolio_id) {
            self.configs.insert(portfolio_id.to_string(), config.clone());
            return config;
        }

        // Create new default config
        let config = RatConfig::new(portfolio_id.to_string());
        self.configs.insert(portfolio_id.to_string(), config.clone());

        // Save to database
        if let Err(e) = self.sqlite.save_rat_config(&config) {
            warn!("Failed to save new RAT config: {}", e);
        }

        config
    }

    /// Start RAT for a portfolio.
    pub fn start(
        self: Arc<Self>,
        portfolio_id: &str,
        config_update: Option<RatConfigUpdate>,
    ) -> Result<RatState, RatError> {
        // Check if already running
        if self.active_rats.contains_key(portfolio_id) {
            return Err(RatError::AlreadyRunning(portfolio_id.to_string()));
        }

        // Verify portfolio exists
        if self.trading.get_portfolio(portfolio_id).is_none() {
            return Err(RatError::PortfolioNotFound(portfolio_id.to_string()));
        }

        // Get or create config
        let mut config = self
            .configs
            .get(portfolio_id)
            .map(|c| c.clone())
            .unwrap_or_else(|| RatConfig::new(portfolio_id.to_string()));

        // Apply any config updates
        if let Some(update) = config_update {
            update.apply_to(&mut config);
        }

        // Mark as enabled
        config.enabled = true;
        config.updated_at = chrono::Utc::now().timestamp_millis();

        // Save config
        self.sqlite
            .save_rat_config(&config)
            .map_err(|e| RatError::DatabaseError(e.to_string()))?;
        self.configs.insert(portfolio_id.to_string(), config.clone());

        // Get or create stats
        let mut stats = self
            .stats
            .get(portfolio_id)
            .map(|s| s.clone())
            .unwrap_or_else(|| RatStats::new(portfolio_id.to_string()));
        stats.started_at = Some(chrono::Utc::now().timestamp_millis());
        self.stats.insert(portfolio_id.to_string(), stats.clone());

        // Start the trading loop
        self.clone().start_internal(portfolio_id)?;

        // Get current open positions count
        let open_positions = self.trading.get_positions(portfolio_id).len() as u32;

        Ok(RatState {
            config,
            stats,
            status: RatStatus::Active,
            open_positions,
        })
    }

    /// Internal start method (spawns the trading loop).
    fn start_internal(self: Arc<Self>, portfolio_id: &str) -> Result<(), RatError> {
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = cancel.clone();
        let portfolio_id_owned = portfolio_id.to_string();
        let service = self.clone();

        let handle = tokio::spawn(async move {
            service.trading_loop(portfolio_id_owned, cancel_clone).await;
        });

        self.active_rats.insert(
            portfolio_id.to_string(),
            ActiveRat { handle, cancel },
        );
        self.statuses
            .insert(portfolio_id.to_string(), RatStatus::Active);

        // Broadcast status update
        self.broadcast_status(portfolio_id);

        info!("Started RAT for portfolio {}", portfolio_id);
        Ok(())
    }

    /// Stop RAT for a portfolio.
    pub fn stop(&self, portfolio_id: &str) -> Result<RatState, RatError> {
        // Check if running
        let active = self
            .active_rats
            .remove(portfolio_id)
            .ok_or_else(|| RatError::NotRunning(portfolio_id.to_string()))?;

        // Update status to stopping
        self.statuses
            .insert(portfolio_id.to_string(), RatStatus::Stopping);
        self.broadcast_status(portfolio_id);

        // Cancel the task
        active.1.cancel.store(true, Ordering::SeqCst);

        // Update config to disabled
        if let Some(mut config) = self.configs.get_mut(portfolio_id) {
            config.enabled = false;
            config.updated_at = chrono::Utc::now().timestamp_millis();
            let _ = self.sqlite.save_rat_config(&config);
        }

        // Update stats
        if let Some(mut stats) = self.stats.get_mut(portfolio_id) {
            stats.started_at = None;
            stats.updated_at = chrono::Utc::now().timestamp_millis();
            let _ = self.sqlite.save_rat_stats(&stats);
        }

        // Update status to idle
        self.statuses
            .insert(portfolio_id.to_string(), RatStatus::Idle);
        self.broadcast_status(portfolio_id);

        info!("Stopped RAT for portfolio {}", portfolio_id);

        self.get_state(portfolio_id)
            .ok_or_else(|| RatError::NotRunning(portfolio_id.to_string()))
    }

    /// Update RAT configuration.
    /// Creates a default config if none exists, then applies the update.
    pub fn update_config(
        &self,
        portfolio_id: &str,
        update: RatConfigUpdate,
    ) -> Result<RatConfig, RatError> {
        // Ensure config exists (creates default if needed)
        self.get_or_create_config(portfolio_id);

        // Now get mutable reference and apply update
        let mut config = self
            .configs
            .get_mut(portfolio_id)
            .ok_or_else(|| RatError::PortfolioNotFound(portfolio_id.to_string()))?;

        update.apply_to(&mut config);

        self.sqlite
            .save_rat_config(&config)
            .map_err(|e| RatError::DatabaseError(e.to_string()))?;

        // Broadcast update
        self.broadcast_status(portfolio_id);

        Ok(config.clone())
    }

    /// The main trading loop.
    async fn trading_loop(self: Arc<Self>, portfolio_id: String, cancel: Arc<AtomicBool>) {
        info!("RAT trading loop started for portfolio {}", portfolio_id);

        // Execute first iteration immediately
        let mut first_run = true;

        loop {
            // Check if cancelled
            if cancel.load(Ordering::SeqCst) {
                info!("RAT cancelled for portfolio {}", portfolio_id);
                break;
            }

            // Get current config
            let config = match self.configs.get(&portfolio_id) {
                Some(c) => c.clone(),
                None => {
                    warn!("Config not found for RAT {}, stopping", portfolio_id);
                    break;
                }
            };

            // Skip sleep on first run to execute immediately
            if first_run {
                first_run = false;
            } else {
                let interval = Duration::from_secs(config.trade_interval_secs);
                tokio::time::sleep(interval).await;

                // Check cancel again after sleep
                if cancel.load(Ordering::SeqCst) {
                    info!("RAT cancelled for portfolio {}", portfolio_id);
                    break;
                }
            }

            // Execute a trading iteration
            if let Err(e) = self.execute_iteration(&portfolio_id, &config).await {
                warn!("RAT iteration error for {}: {}", portfolio_id, e);
                if let Some(mut stats) = self.stats.get_mut(&portfolio_id) {
                    stats.record_error();
                    let _ = self.sqlite.save_rat_stats(&stats);
                }
            }
        }

        info!("RAT trading loop ended for portfolio {}", portfolio_id);
    }

    /// Execute a single trading iteration.
    async fn execute_iteration(
        &self,
        portfolio_id: &str,
        config: &RatConfig,
    ) -> Result<(), RatError> {
        // Get current positions
        let positions = self.trading.get_positions(portfolio_id);
        let _open_count = positions.len() as u32;

        // Decide what action to take
        let action = self.decide_action(portfolio_id, config, &positions);

        match action {
            RatAction::OpenPosition {
                symbol,
                side,
                size,
                stop_loss,
                take_profit,
            } => {
                debug!(
                    "RAT opening {} {} position for {} (size: {})",
                    side, symbol, portfolio_id, size
                );

                // Get current price
                let price = self
                    .price_cache
                    .get_price(&symbol)
                    .ok_or(RatError::NoPriceData)?;

                let request = PlaceOrderRequest {
                    portfolio_id: portfolio_id.to_string(),
                    symbol: symbol.clone(),
                    asset_class: AssetClass::CryptoSpot,
                    side,
                    order_type: OrderType::Market,
                    quantity: size,
                    price: None,
                    stop_price: None,
                    trail_amount: None,
                    trail_percent: None,
                    time_in_force: None,
                    leverage: Some(1.0),
                    stop_loss,
                    take_profit,
                    reduce_only: false,
                    bypass_drawdown: false,
                    post_only: false,
                    margin_mode: None,
                    client_order_id: None,
                };

                match self.trading.place_order(request) {
                    Ok(order) => {
                        // Execute immediately for market orders (no order book needed)
                        if let Err(e) = self.trading.execute_market_order(&order.id, price, None) {
                            warn!("Failed to execute RAT order: {}", e);
                        } else {
                            info!(
                                "RAT opened {} {} position for {} at ${:.2} (size: {:.6})",
                                side, symbol, portfolio_id, price, size
                            );
                        }
                    }
                    Err(e) => {
                        warn!("Failed to place RAT order: {}", e);
                    }
                }
            }
            RatAction::ClosePosition { position_id } => {
                debug!("RAT closing position {} for {}", position_id, portfolio_id);

                // Get position details
                if let Some(position) = self.trading.get_position(&position_id) {
                    let price = self
                        .price_cache
                        .get_price(&position.symbol)
                        .unwrap_or(position.current_price);

                    match self.trading.close_position(&position_id, price) {
                        Ok(trade) => {
                            // Record stats
                            if let Some(mut stats) = self.stats.get_mut(portfolio_id) {
                                if let Some(pnl) = trade.realized_pnl {
                                    if pnl >= 0.0 {
                                        stats.record_win(pnl);
                                        info!(
                                            "RAT closed position {} for {} with PROFIT: ${:.2}",
                                            position_id, portfolio_id, pnl
                                        );
                                    } else {
                                        stats.record_loss(pnl);
                                        info!(
                                            "RAT closed position {} for {} with LOSS: ${:.2}",
                                            position_id, portfolio_id, pnl
                                        );
                                    }
                                    let _ = self.sqlite.save_rat_stats(&stats);
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to close RAT position: {}", e);
                        }
                    }
                }
            }
            RatAction::ModifyPosition {
                position_id,
                new_stop_loss,
                new_take_profit,
            } => {
                debug!(
                    "RAT modifying position {} for {}",
                    position_id, portfolio_id
                );

                if let Err(e) =
                    self.trading
                        .modify_position(&position_id, new_stop_loss, new_take_profit)
                {
                    warn!("Failed to modify RAT position: {}", e);
                }
            }
            RatAction::Skip => {
                debug!("RAT skipping iteration for {}", portfolio_id);
            }
        }

        // Broadcast status update
        self.broadcast_status(portfolio_id);

        Ok(())
    }

    /// Decide what trading action to take.
    fn decide_action(
        &self,
        portfolio_id: &str,
        config: &RatConfig,
        positions: &[Position],
    ) -> RatAction {
        let mut rng = thread_rng();
        let now = chrono::Utc::now().timestamp_millis();
        let open_count = positions.len() as u32;

        // Find positions eligible for closing (past min hold time)
        let closeable: Vec<&Position> = positions
            .iter()
            .filter(|p| {
                let hold_time_ms = now - p.created_at;
                let min_hold_ms = (config.min_hold_time_secs * 1000) as i64;
                hold_time_ms >= min_hold_ms
            })
            .collect();

        // Decision probabilities
        let roll: f64 = rng.gen();

        // 20% chance to close a profitable position early
        if roll < 0.20 && !closeable.is_empty() {
            let profitable: Vec<&&Position> = closeable
                .iter()
                .filter(|p| p.unrealized_pnl > 0.0)
                .collect();
            if !profitable.is_empty() {
                let pos = profitable.choose(&mut rng).unwrap();
                return RatAction::ClosePosition {
                    position_id: pos.id.clone(),
                };
            }
        }

        // 10% chance to close a losing position
        if roll < 0.30 && !closeable.is_empty() {
            let losing: Vec<&&Position> = closeable
                .iter()
                .filter(|p| p.unrealized_pnl < 0.0)
                .collect();
            if !losing.is_empty() {
                let pos = losing.choose(&mut rng).unwrap();
                return RatAction::ClosePosition {
                    position_id: pos.id.clone(),
                };
            }
        }

        // 15% chance to modify SL/TP on existing position
        if roll < 0.45 && !positions.is_empty() {
            let pos = positions.choose(&mut rng).unwrap();

            // Generate new SL/TP
            let new_sl = if rng.gen::<f64>() < config.stop_loss_probability {
                let sl_pct = rng.gen_range(config.stop_loss_range_pct.0..config.stop_loss_range_pct.1);
                let sl = match pos.side {
                    crate::types::PositionSide::Long => pos.entry_price * (1.0 - sl_pct),
                    crate::types::PositionSide::Short => pos.entry_price * (1.0 + sl_pct),
                };
                Some(sl)
            } else {
                None
            };

            let new_tp = if rng.gen::<f64>() < config.take_profit_probability {
                let tp_pct = rng.gen_range(config.take_profit_range_pct.0..config.take_profit_range_pct.1);
                let tp = match pos.side {
                    crate::types::PositionSide::Long => pos.entry_price * (1.0 + tp_pct),
                    crate::types::PositionSide::Short => pos.entry_price * (1.0 - tp_pct),
                };
                Some(tp)
            } else {
                None
            };

            if new_sl.is_some() || new_tp.is_some() {
                return RatAction::ModifyPosition {
                    position_id: pos.id.clone(),
                    new_stop_loss: new_sl,
                    new_take_profit: new_tp,
                };
            }
        }

        // 55% chance to open a new position (if under limit)
        if open_count < config.max_open_positions {
            // Select symbol
            let symbols = if config.symbols.is_empty() {
                &self.default_symbols
            } else {
                &config.symbols
            };

            let symbol = symbols.choose(&mut rng).cloned().unwrap_or("BTC".to_string());

            // Get price to calculate size - check both price cache and all available prices
            let price = match self.price_cache.get_price(&symbol) {
                Some(p) => {
                    debug!("RAT got price for {}: ${:.2}", symbol, p);
                    p
                },
                None => {
                    // Log available symbols for debugging
                    let available = self.price_cache.get_all_prices();
                    if available.is_empty() {
                        warn!("RAT: No prices available in cache at all");
                    } else {
                        let symbols_list: Vec<String> = available.iter().map(|(s, _)| s.clone()).collect();
                        warn!("RAT: No price for {} (lowercase: {}). Available: {:?}",
                            symbol, symbol.to_lowercase(), &symbols_list[..symbols_list.len().min(10)]);
                    }
                    return RatAction::Skip;
                }
            };

            // Get portfolio for sizing
            let portfolio = match self.trading.get_portfolio(portfolio_id) {
                Some(p) => p,
                None => return RatAction::Skip,
            };

            // Calculate position size
            let size_pct = rng.gen_range(config.size_range_pct.0..config.size_range_pct.1);
            let notional = portfolio.margin_available * size_pct;
            let size = notional / price;

            if size < 0.0001 {
                return RatAction::Skip;
            }

            // Random side
            let side = if rng.gen::<bool>() {
                OrderSide::Buy
            } else {
                OrderSide::Sell
            };

            // Generate SL/TP
            let stop_loss = if rng.gen::<f64>() < config.stop_loss_probability {
                let sl_pct = rng.gen_range(config.stop_loss_range_pct.0..config.stop_loss_range_pct.1);
                let sl = match side {
                    OrderSide::Buy => price * (1.0 - sl_pct),
                    OrderSide::Sell => price * (1.0 + sl_pct),
                };
                Some(sl)
            } else {
                None
            };

            let take_profit = if rng.gen::<f64>() < config.take_profit_probability {
                let tp_pct = rng.gen_range(config.take_profit_range_pct.0..config.take_profit_range_pct.1);
                let tp = match side {
                    OrderSide::Buy => price * (1.0 + tp_pct),
                    OrderSide::Sell => price * (1.0 - tp_pct),
                };
                Some(tp)
            } else {
                None
            };

            return RatAction::OpenPosition {
                symbol,
                side,
                size,
                stop_loss,
                take_profit,
            };
        }

        RatAction::Skip
    }

    /// Broadcast RAT status update via WebSocket.
    fn broadcast_status(&self, portfolio_id: &str) {
        if let Some(ref room_manager) = self.room_manager {
            if let Some(state) = self.get_state(portfolio_id) {
                let msg = ServerMessage::RatStatusUpdate {
                    data: crate::types::RatStatusUpdateData {
                        portfolio_id: portfolio_id.to_string(),
                        status: state.status,
                        stats: state.stats,
                        config: state.config,
                        open_positions: state.open_positions,
                        timestamp: chrono::Utc::now().timestamp_millis(),
                    },
                };

                if let Ok(json) = serde_json::to_string(&msg) {
                    room_manager.broadcast_trading(portfolio_id, &json);
                }
            }
        }
    }

    /// Check if RAT is running for a portfolio.
    pub fn is_running(&self, portfolio_id: &str) -> bool {
        self.active_rats.contains_key(portfolio_id)
    }

    /// Get count of running RAT instances.
    pub fn running_count(&self) -> usize {
        self.active_rats.len()
    }
}
