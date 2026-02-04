//! Bot Runner Service
//!
//! Manages the lifecycle of trading bots, including starting, stopping,
//! and coordinating their trading activities.

use std::sync::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

use crate::error::AppError;
use crate::services::{PriceCache, SignalStore, SqliteStore, TradingService};
use crate::types::{AssetClass, TradingTimeframe};

use super::{BotPersonality, DecisionContext, TradingBot, TradeDecision};

/// Status of a running bot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotStatus {
    /// Bot's unique ID
    pub id: String,
    /// Bot's display name
    pub name: String,
    /// Bot personality type
    pub personality: BotPersonality,
    /// Whether the bot is currently running
    pub running: bool,
    /// Bot's portfolio ID
    pub portfolio_id: Option<String>,
    /// Total trades executed
    pub total_trades: u64,
    /// Winning trades
    pub winning_trades: u64,
    /// Total realized PnL
    pub total_pnl: f64,
    /// Current portfolio value
    pub portfolio_value: f64,
    /// Last decision timestamp
    pub last_decision_at: Option<i64>,
    /// Last error message (if any)
    pub last_error: Option<String>,
    /// Asset classes being traded
    pub asset_classes: Vec<AssetClass>,
}

/// Bot runner manages all trading bots
pub struct BotRunner {
    /// Registered bots
    bots: RwLock<HashMap<String, Arc<dyn TradingBot>>>,
    /// Bot statuses
    statuses: RwLock<HashMap<String, BotStatus>>,
    /// Price cache for market data
    price_cache: Arc<PriceCache>,
    /// Signal store for indicators
    signal_store: Arc<SignalStore>,
    /// Trading service for order execution
    trading_service: Arc<TradingService>,
    /// SQLite store for persistence
    sqlite_store: Arc<SqliteStore>,
    /// Shutdown signal sender
    shutdown_tx: broadcast::Sender<()>,
    /// Whether the runner is active
    running: RwLock<bool>,
}

impl BotRunner {
    /// Create a new bot runner
    pub fn new(
        price_cache: Arc<PriceCache>,
        signal_store: Arc<SignalStore>,
        trading_service: Arc<TradingService>,
        sqlite_store: Arc<SqliteStore>,
    ) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);

        Self {
            bots: RwLock::new(HashMap::new()),
            statuses: RwLock::new(HashMap::new()),
            price_cache,
            signal_store,
            trading_service,
            sqlite_store,
            shutdown_tx,
            running: RwLock::new(false),
        }
    }

    /// Register a bot synchronously (portfolio created on first tick)
    pub fn register_bot<T: TradingBot + 'static>(&self, bot: T) {
        let bot = Arc::new(bot);
        let config = bot.config().clone();
        let bot_id = config.id.clone();

        // Initialize status (portfolio will be created on first tick)
        let status = BotStatus {
            id: bot_id.clone(),
            name: config.name.clone(),
            personality: config.personality,
            running: false,
            portfolio_id: None, // Will be set when start() is called
            total_trades: 0,
            winning_trades: 0,
            total_pnl: 0.0,
            portfolio_value: config.initial_capital,
            last_decision_at: None,
            last_error: None,
            asset_classes: config.asset_classes.clone(),
        };

        self.bots.write().unwrap().insert(bot_id.clone(), bot);
        self.statuses.write().unwrap().insert(bot_id, status);
    }

    /// Get the number of registered bots
    pub fn bot_count(&self) -> usize {
        self.bots.read().unwrap().len()
    }

    /// Register a bot with the runner (async version that creates portfolio)
    pub async fn register_bot_async(&self, bot: Arc<dyn TradingBot>) -> Result<String, AppError> {
        let config = bot.config().clone();
        let bot_id = config.id.clone();

        // Create a portfolio for the bot if it doesn't exist
        let portfolio_id = format!("bot_{}", bot_id);

        // Check if portfolio exists, create if not
        let portfolio = self.trading_service.get_portfolio(&portfolio_id);

        let portfolio_id = match portfolio {
            Some(p) => p.id,
            None => {
                // Create new portfolio for bot
                let new_portfolio = self
                    .trading_service
                    .create_portfolio(
                        &portfolio_id,
                        &format!("{} Portfolio", config.name),
                        Some(format!("Automated trading bot: {}", config.name)),
                        None, // Default risk settings
                    )
                    .map_err(|e| AppError::Internal(e.to_string()))?;
                new_portfolio.id
            }
        };

        // Initialize status
        let status = BotStatus {
            id: bot_id.clone(),
            name: config.name.clone(),
            personality: config.personality,
            running: false,
            portfolio_id: Some(portfolio_id),
            total_trades: 0,
            winning_trades: 0,
            total_pnl: 0.0,
            portfolio_value: config.initial_capital,
            last_decision_at: None,
            last_error: None,
            asset_classes: config.asset_classes.clone(),
        };

        // Register bot
        self.bots.write().unwrap().insert(bot_id.clone(), bot);
        self.statuses.write().unwrap().insert(bot_id.clone(), status);

        info!("Registered bot: {} ({})", config.name, bot_id);

        Ok(bot_id)
    }

    /// Initialize bot portfolios without starting the main loop.
    /// This is useful for testing and for ensuring portfolios exist before starting.
    pub fn initialize(&self) {
        // Create portfolios for bots that don't have one yet
        let bots: Vec<_> = self.bots.read().unwrap().values().cloned().collect();
        for bot in bots {
            let config = bot.config();
            let bot_id = config.id.clone();
            let bot_user_id = format!("bot_{}", bot_id);

            // Check if portfolio exists for this bot (by user_id)
            let existing_portfolios = self.trading_service.get_user_portfolios(&bot_user_id);

            if existing_portfolios.is_empty() {
                // Create new portfolio for bot
                match self.trading_service.create_portfolio(
                    &bot_user_id,
                    &format!("{} Portfolio", config.name),
                    Some(format!("Automated trading bot: {}", config.name)),
                    None,
                ) {
                    Ok(portfolio) => {
                        // Update status with portfolio ID
                        let mut statuses = self.statuses.write().unwrap();
                        if let Some(status) = statuses.get_mut(&bot_id) {
                            status.portfolio_id = Some(portfolio.id);
                        }
                        info!("Created portfolio for bot {}", config.name);
                    }
                    Err(e) => {
                        warn!("Failed to create portfolio for bot {}: {}", config.name, e);
                    }
                }
            } else {
                // Portfolio exists, update status with the first one
                let portfolio_id = existing_portfolios[0].id.clone();
                let mut statuses = self.statuses.write().unwrap();
                if let Some(status) = statuses.get_mut(&bot_id) {
                    status.portfolio_id = Some(portfolio_id);
                }
            }
        }
    }

    /// Start the bot runner
    pub async fn start(&self) {
        if *self.running.read().unwrap() {
            return;
        }

        *self.running.write().unwrap() = true;
        info!("Bot runner started");

        // Initialize portfolios
        self.initialize();

        // Mark all bots as running
        for status in self.statuses.write().unwrap().values_mut() {
            status.running = true;
        }

        // Start the main loop
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        // Find minimum tick interval from all bots
        let min_interval = {
            let bots = self.bots.read().unwrap();
            bots.values()
                .map(|b| b.config().decision_interval_secs)
                .min()
                .unwrap_or(60)
        };

        info!("Bot runner tick interval: {}s", min_interval);
        let mut ticker = interval(Duration::from_secs(min_interval));

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if let Err(e) = self.tick().await {
                        error!("Bot runner tick error: {}", e);
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Bot runner received shutdown signal");
                    break;
                }
            }
        }
    }

    /// Stop the bot runner
    pub async fn stop(&self) -> Result<(), AppError> {
        if !*self.running.read().unwrap() {
            return Ok(());
        }

        *self.running.write().unwrap() = false;

        // Send shutdown signal
        let _ = self.shutdown_tx.send(());

        // Mark all bots as stopped
        for status in self.statuses.write().unwrap().values_mut() {
            status.running = false;
        }

        info!("Bot runner stopped");
        Ok(())
    }

    /// Run a single tick for all bots
    pub async fn tick(&self) -> Result<(), AppError> {
        if !*self.running.read().unwrap() {
            return Ok(());
        }

        let bots: Vec<_> = self.bots.read().unwrap().values().cloned().collect();

        for bot in bots {
            if let Err(e) = self.tick_bot(&bot).await {
                let mut statuses = self.statuses.write().unwrap();
                if let Some(status) = statuses.get_mut(&bot.config().id) {
                    status.last_error = Some(e.to_string());
                }
                error!("Bot {} tick error: {}", bot.name(), e);
            }
        }

        Ok(())
    }

    /// Run a single tick for one bot
    async fn tick_bot(&self, bot: &Arc<dyn TradingBot>) -> Result<(), AppError> {
        let config = bot.config();

        // Skip if disabled
        if !config.enabled {
            return Ok(());
        }

        // Get the bot's portfolio
        let portfolio_id = format!("bot_{}", config.id);
        let portfolio = self
            .trading_service
            .get_portfolio(&portfolio_id)
            .ok_or_else(|| AppError::NotFound(format!("Portfolio not found: {}", portfolio_id)))?;

        // Get symbols to analyze
        let symbols = self.get_symbols_for_bot(bot).await?;

        for symbol in symbols {
            // Build decision context
            let ctx = match self.build_context(&symbol, &portfolio_id, &portfolio).await {
                Ok(ctx) => ctx,
                Err(e) => {
                    debug!("Could not build context for {}: {}", symbol, e);
                    continue;
                }
            };

            // Skip if asset class not supported
            if !bot.supported_asset_classes().contains(&ctx.asset_class) {
                continue;
            }

            // Get decision from bot
            let decision = bot.analyze(&ctx).await?;

            // Execute decision if not hold
            if !decision.is_hold() {
                self.execute_decision(bot, &decision, &portfolio_id, ctx.current_price)
                    .await?;
            }
        }

        // Update last decision time
        {
            let mut statuses = self.statuses.write().unwrap();
            if let Some(status) = statuses.get_mut(&config.id) {
                status.last_decision_at = Some(chrono::Utc::now().timestamp());
                status.last_error = None;
            }
        } // Drop the lock guard before awaiting

        // Run bot's internal tick
        bot.tick().await?;

        Ok(())
    }

    /// Get symbols for a bot to analyze
    async fn get_symbols_for_bot(&self, bot: &Arc<dyn TradingBot>) -> Result<Vec<String>, AppError> {
        let config = bot.config();

        // If bot has specific symbols, use those
        if !config.symbols.is_empty() {
            return Ok(config.symbols.clone());
        }

        // Otherwise, get popular symbols for each asset class
        let mut symbols = Vec::new();

        for asset_class in &config.asset_classes {
            match asset_class {
                AssetClass::CryptoSpot | AssetClass::Perp => {
                    // Top crypto pairs
                    symbols.extend(vec![
                        "BTC".to_string(),
                        "ETH".to_string(),
                        "SOL".to_string(),
                        "DOGE".to_string(),
                        "XRP".to_string(),
                    ]);
                }
                AssetClass::Stock | AssetClass::Etf => {
                    // Top stocks
                    symbols.extend(vec![
                        "AAPL".to_string(),
                        "MSFT".to_string(),
                        "GOOGL".to_string(),
                        "AMZN".to_string(),
                        "NVDA".to_string(),
                    ]);
                }
                AssetClass::Forex => {
                    // Major forex pairs
                    symbols.extend(vec![
                        "EUR/USD".to_string(),
                        "GBP/USD".to_string(),
                        "USD/JPY".to_string(),
                    ]);
                }
                AssetClass::Option => {
                    // Options trading handled separately
                }
            }
        }

        Ok(symbols)
    }

    /// Build decision context for a symbol
    async fn build_context(
        &self,
        symbol: &str,
        portfolio_id: &str,
        portfolio: &crate::types::Portfolio,
    ) -> Result<DecisionContext, AppError> {
        // Get current price (just an f64)
        let current_price = self
            .price_cache
            .get_price(symbol)
            .ok_or_else(|| AppError::NotFound(format!("No price for {}", symbol)))?;

        // Get signals/indicators - use SwingTrading timeframe for Grandma (longer-term)
        let signals = self
            .signal_store
            .get_signals(symbol, TradingTimeframe::SwingTrading)
            .await;

        // Get position info
        let positions = self.trading_service.get_positions(portfolio_id);
        let position = positions.iter().find(|p| p.symbol == symbol);

        // Get today's trades
        let trades = self.trading_service.get_trades(portfolio_id, 100);

        // Convert today start to milliseconds for comparison
        let today_start_ms = chrono::Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis();

        let trades_today = trades
            .iter()
            .filter(|t| t.symbol == symbol && t.executed_at >= today_start_ms)
            .count() as u32;

        let last_trade = trades
            .iter()
            .filter(|t| t.symbol == symbol)
            .max_by_key(|t| t.executed_at)
            .map(|t| t.executed_at / 1000); // Convert to seconds for consistency

        // Determine asset class
        let asset_class = self.determine_asset_class(symbol);

        // Extract indicator values from signals
        let (rsi, macd_histogram, sma_short, sma_long, bb_upper, bb_lower, bb_middle, atr, adx) =
            if let Some(ref sigs) = signals {
                // Extract values from the signals vector by indicator name
                let get_value = |name: &str| -> Option<f64> {
                    sigs.signals.iter().find(|s| s.name == name).map(|s| s.value)
                };

                (
                    get_value("RSI"),
                    get_value("MACD"), // MACD value is the histogram
                    get_value("SMA"), // Short-term SMA (typically SMA 50)
                    None, // Long-term SMA (200) - we'll calculate separately if needed
                    get_value("Bollinger Upper"),
                    get_value("Bollinger Lower"),
                    get_value("Bollinger Middle"),
                    get_value("ATR"),
                    get_value("ADX"),
                )
            } else {
                (None, None, None, None, None, None, None, None, None)
            };

        Ok(DecisionContext {
            symbol: symbol.to_string(),
            asset_class,
            current_price,
            high_24h: None,    // Price cache doesn't have 24h data
            low_24h: None,
            volume_24h: None,
            price_change_24h_pct: None,
            rsi,
            macd_histogram,
            macd_crossover: None, // TODO: Track crossovers
            sma_short,
            sma_long,
            ema_short: None, // TODO: Add EMA to signals
            ema_long: None,
            bb_upper,
            bb_lower,
            bb_middle,
            atr,
            adx,
            volume_ratio: None, // TODO: Calculate volume ratio
            orderbook: None,    // TODO: Add orderbook data
            current_position: position.map(|p| p.quantity),
            position_entry_price: position.map(|p| p.entry_price),
            unrealized_pnl: position.map(|p| p.unrealized_pnl),
            trades_today,
            last_trade_timestamp: last_trade,
            available_cash: portfolio.cash_balance,
            portfolio_value: portfolio.total_value,
            timestamp: chrono::Utc::now().timestamp(),
        })
    }

    /// Determine asset class from symbol
    fn determine_asset_class(&self, symbol: &str) -> AssetClass {
        // Simple heuristics - in production, this would use a proper lookup
        let crypto_symbols = [
            "BTC", "ETH", "SOL", "DOGE", "XRP", "ADA", "AVAX", "DOT", "MATIC", "LINK",
        ];

        if crypto_symbols.contains(&symbol) || symbol.ends_with("USDT") || symbol.ends_with("USD") {
            AssetClass::CryptoSpot
        } else if symbol.contains('/') {
            AssetClass::Forex
        } else {
            AssetClass::Stock
        }
    }

    /// Execute a trade decision
    async fn execute_decision(
        &self,
        bot: &Arc<dyn TradingBot>,
        decision: &TradeDecision,
        portfolio_id: &str,
        current_price: f64,
    ) -> Result<(), AppError> {
        use crate::types::{OrderSide, OrderType, PlaceOrderRequest};

        let config = bot.config();

        match decision {
            TradeDecision::Buy {
                symbol,
                quantity,
                confidence,
                stop_loss,
                take_profit,
                ..
            } => {
                info!(
                    "Bot {} executing BUY: {} {} @ ~{:.2} (confidence: {:.1}%)",
                    config.name,
                    quantity,
                    symbol,
                    current_price,
                    confidence * 100.0
                );

                let asset_class = self.determine_asset_class(symbol);

                // Place market order
                let request = PlaceOrderRequest {
                    portfolio_id: portfolio_id.to_string(),
                    symbol: symbol.clone(),
                    asset_class,
                    side: OrderSide::Buy,
                    order_type: OrderType::Market,
                    quantity: *quantity,
                    price: None,
                    stop_price: None,
                    trail_amount: None,
                    trail_percent: None,
                    time_in_force: None,
                    leverage: None,
                    stop_loss: *stop_loss,
                    take_profit: *take_profit,
                    client_order_id: None,
                };

                let order = self
                    .trading_service
                    .place_order(request)
                    .map_err(|e| AppError::Internal(e.to_string()))?;

                // Update stats
                self.update_bot_stats(&config.id, true, 0.0);

                // Notify bot of execution
                bot.on_trade_executed(symbol, decision, order.avg_fill_price.unwrap_or(current_price))
                    .await?;
            }
            TradeDecision::Sell {
                symbol,
                quantity,
                confidence,
                ..
            } => {
                info!(
                    "Bot {} executing SELL: {} {} @ ~{:.2} (confidence: {:.1}%)",
                    config.name,
                    quantity,
                    symbol,
                    current_price,
                    confidence * 100.0
                );

                let asset_class = self.determine_asset_class(symbol);

                // Place market order
                let request = PlaceOrderRequest {
                    portfolio_id: portfolio_id.to_string(),
                    symbol: symbol.clone(),
                    asset_class,
                    side: OrderSide::Sell,
                    order_type: OrderType::Market,
                    quantity: *quantity,
                    price: None,
                    stop_price: None,
                    trail_amount: None,
                    trail_percent: None,
                    time_in_force: None,
                    leverage: None,
                    stop_loss: None,
                    take_profit: None,
                    client_order_id: None,
                };

                let order = self
                    .trading_service
                    .place_order(request)
                    .map_err(|e| AppError::Internal(e.to_string()))?;

                // Calculate PnL for stats
                // Note: This is simplified; real PnL should come from position close
                let realized_pnl = order.avg_fill_price.map(|p| {
                    let position = quantity * p;
                    position * 0.01 // Placeholder - actual PnL tracking needed
                }).unwrap_or(0.0);

                // Update stats
                self.update_bot_stats(&config.id, realized_pnl > 0.0, realized_pnl);

                // Notify bot of execution
                bot.on_trade_executed(symbol, decision, order.avg_fill_price.unwrap_or(current_price))
                    .await?;
            }
            TradeDecision::Hold { .. } => {
                // No action needed
            }
        }

        Ok(())
    }

    /// Update bot statistics after a trade
    fn update_bot_stats(&self, bot_id: &str, is_winner: bool, pnl: f64) {
        let mut statuses = self.statuses.write().unwrap();
        if let Some(status) = statuses.get_mut(bot_id) {
            status.total_trades += 1;
            if is_winner {
                status.winning_trades += 1;
            }
            status.total_pnl += pnl;
        }
    }

    /// Get status of all bots
    pub fn get_all_statuses(&self) -> Vec<BotStatus> {
        self.statuses.read().unwrap().values().cloned().collect()
    }

    /// Get status of a specific bot
    pub fn get_status(&self, bot_id: &str) -> Option<BotStatus> {
        self.statuses.read().unwrap().get(bot_id).cloned()
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{ChartStore, PriceCache, SqliteStore, TradingService};
    use crate::services::signals::{AccuracyStore, PredictionStore, SignalStore};
    use crate::services::paperbot::GrandmaBot;
    use crate::types::AggregationConfig;

    /// Create test dependencies for BotRunner
    fn create_test_runner() -> (BotRunner, Arc<TradingService>) {
        let sqlite = Arc::new(SqliteStore::new(":memory:").expect("Failed to create SQLite"));
        let trading_service = Arc::new(TradingService::new(sqlite.clone()));
        let (price_cache, _rx) = PriceCache::new(AggregationConfig::default());
        let signal_store = SignalStore::new(
            ChartStore::new(),
            PredictionStore::new(),
            AccuracyStore::new(),
        );

        let runner = BotRunner::new(
            price_cache,
            signal_store,
            trading_service.clone(),
            sqlite,
        );

        (runner, trading_service)
    }

    #[test]
    fn test_bot_registration() {
        let (runner, _) = create_test_runner();

        assert_eq!(runner.bot_count(), 0);

        let grandma = GrandmaBot::new();
        runner.register_bot(grandma);

        assert_eq!(runner.bot_count(), 1);
    }

    #[test]
    fn test_get_bot_status() {
        let (runner, _) = create_test_runner();

        runner.register_bot(GrandmaBot::new());

        let status = runner.get_status("grandma");
        assert!(status.is_some());

        let status = status.unwrap();
        assert_eq!(status.id, "grandma");
        assert_eq!(status.name, "Grandma");
        assert!(!status.running);
        assert_eq!(status.total_trades, 0);
        assert_eq!(status.winning_trades, 0);
    }

    #[test]
    fn test_get_all_statuses() {
        let (runner, _) = create_test_runner();

        runner.register_bot(GrandmaBot::new());

        let statuses = runner.get_all_statuses();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].name, "Grandma");
    }

    #[test]
    fn test_bot_not_running_before_start() {
        let (runner, _) = create_test_runner();

        runner.register_bot(GrandmaBot::new());

        let status = runner.get_status("grandma").unwrap();
        assert!(!status.running);
    }

    #[test]
    fn test_portfolio_created_on_initialize() {
        let (runner, trading_service) = create_test_runner();

        runner.register_bot(GrandmaBot::new());

        // Before initialize, no portfolio for this bot
        let portfolios = trading_service.get_user_portfolios("bot_grandma");
        assert!(portfolios.is_empty(), "No portfolio should exist before initialize");

        // Initialize (this creates portfolios without starting the loop)
        runner.initialize();

        // After initialize, portfolio should exist
        let portfolios = trading_service.get_user_portfolios("bot_grandma");
        assert!(!portfolios.is_empty(), "Bot portfolio should be created on initialize");

        let portfolio = &portfolios[0];
        assert_eq!(portfolio.name, "Grandma Portfolio");
        assert_eq!(portfolio.user_id, "bot_grandma");
    }

    #[test]
    fn test_update_bot_stats() {
        let (runner, _) = create_test_runner();

        runner.register_bot(GrandmaBot::new());

        // Simulate a winning trade
        runner.update_bot_stats("grandma", true, 100.0);

        let status = runner.get_status("grandma").unwrap();
        assert_eq!(status.total_trades, 1);
        assert_eq!(status.winning_trades, 1);
        assert_eq!(status.total_pnl, 100.0);

        // Simulate a losing trade
        runner.update_bot_stats("grandma", false, -50.0);

        let status = runner.get_status("grandma").unwrap();
        assert_eq!(status.total_trades, 2);
        assert_eq!(status.winning_trades, 1);
        assert_eq!(status.total_pnl, 50.0);
    }

    #[test]
    fn test_bot_status_personality() {
        let (runner, _) = create_test_runner();

        runner.register_bot(GrandmaBot::new());

        let status = runner.get_status("grandma").unwrap();
        assert_eq!(status.personality, BotPersonality::Grandma);
    }

    #[test]
    fn test_bot_status_asset_classes() {
        let (runner, _) = create_test_runner();

        runner.register_bot(GrandmaBot::new());

        let status = runner.get_status("grandma").unwrap();
        assert!(status.asset_classes.contains(&AssetClass::CryptoSpot));
        assert!(status.asset_classes.contains(&AssetClass::Stock));
        assert!(status.asset_classes.contains(&AssetClass::Forex));
    }
}
