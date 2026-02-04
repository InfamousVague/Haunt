//! Backtesting Engine
//!
//! Tests trading strategies against historical data.
//! Features:
//! - Historical price simulation
//! - Trade execution simulation with slippage and commission
//! - Equity curve generation
//! - Performance metrics calculation
//! - Buy-and-hold comparison
//! - Monte Carlo simulation for robustness testing

use crate::services::{SqliteStore, StrategyEngine, IndicatorSnapshot};
use crate::types::{
    AssetClass, BacktestConfig, BacktestMetrics, BacktestResult, BacktestStatus, BacktestTrade,
    BuyAndHoldComparison, EquityPoint, MonteCarloResults, OrderSide, TradingStrategy,
};
use dashmap::DashMap;
use rand::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, info, warn};

/// Backtesting errors.
#[derive(Debug, Error)]
pub enum BacktestError {
    #[error("Strategy not found: {0}")]
    StrategyNotFound(String),
    #[error("No historical data available for {symbol} from {start} to {end}")]
    NoHistoricalData { symbol: String, start: i64, end: i64 },
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Backtest cancelled")]
    Cancelled,
}

/// Simulated OHLCV candle for backtesting.
#[derive(Debug, Clone)]
pub struct BacktestCandle {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// State of a position during backtesting.
#[derive(Debug, Clone)]
struct SimulatedPosition {
    symbol: String,
    side: OrderSide,
    quantity: f64,
    entry_price: f64,
    entry_time: i64,
    entry_rule_id: Option<String>,
    commission_paid: f64,
    max_favorable_excursion: f64,
    max_adverse_excursion: f64,
}

impl SimulatedPosition {
    fn unrealized_pnl(&self, current_price: f64) -> f64 {
        match self.side {
            OrderSide::Buy => (current_price - self.entry_price) * self.quantity,
            OrderSide::Sell => (self.entry_price - current_price) * self.quantity,
        }
    }

    fn to_trade(&self, exit_price: f64, exit_time: i64, exit_rule_id: Option<String>, exit_commission: f64) -> BacktestTrade {
        let commission = self.commission_paid + exit_commission;
        let gross_pnl = match self.side {
            OrderSide::Buy => (exit_price - self.entry_price) * self.quantity,
            OrderSide::Sell => (self.entry_price - exit_price) * self.quantity,
        };
        let pnl = gross_pnl - commission;
        let pnl_pct = pnl / (self.entry_price * self.quantity) * 100.0;

        BacktestTrade {
            id: uuid::Uuid::new_v4().to_string(),
            symbol: self.symbol.clone(),
            side: self.side,
            entry_price: self.entry_price,
            exit_price: Some(exit_price),
            quantity: self.quantity,
            entry_time: self.entry_time,
            exit_time: Some(exit_time),
            pnl,
            pnl_pct,
            commission,
            entry_rule_id: self.entry_rule_id.clone(),
            exit_rule_id,
            is_winner: pnl > 0.0,
            max_favorable_excursion: self.max_favorable_excursion,
            max_adverse_excursion: self.max_adverse_excursion,
        }
    }
}

/// Portfolio state during backtesting.
#[derive(Debug, Clone)]
struct SimulatedPortfolio {
    cash: f64,
    positions: HashMap<String, SimulatedPosition>,
    realized_pnl: f64,
    total_commission: f64,
    total_slippage: f64,
    peak_equity: f64,
    time_with_positions: i64,
    total_time: i64,
}

impl SimulatedPortfolio {
    fn new(initial_balance: f64) -> Self {
        Self {
            cash: initial_balance,
            positions: HashMap::new(),
            realized_pnl: 0.0,
            total_commission: 0.0,
            total_slippage: 0.0,
            peak_equity: initial_balance,
            time_with_positions: 0,
            total_time: 0,
        }
    }

    fn equity(&self, prices: &HashMap<String, f64>) -> f64 {
        let positions_value: f64 = self.positions.values()
            .map(|pos| {
                let price = prices.get(&pos.symbol).copied().unwrap_or(pos.entry_price);
                pos.entry_price * pos.quantity + pos.unrealized_pnl(price)
            })
            .sum();
        self.cash + positions_value
    }

    fn unrealized_pnl(&self, prices: &HashMap<String, f64>) -> f64 {
        self.positions.values()
            .map(|pos| {
                let price = prices.get(&pos.symbol).copied().unwrap_or(pos.entry_price);
                pos.unrealized_pnl(price)
            })
            .sum()
    }

    fn positions_value(&self, prices: &HashMap<String, f64>) -> f64 {
        self.positions.values()
            .map(|pos| {
                let price = prices.get(&pos.symbol).copied().unwrap_or(pos.entry_price);
                price * pos.quantity
            })
            .sum()
    }

    fn drawdown_pct(&self, prices: &HashMap<String, f64>) -> f64 {
        let equity = self.equity(prices);
        if self.peak_equity > 0.0 {
            (self.peak_equity - equity) / self.peak_equity * 100.0
        } else {
            0.0
        }
    }

    fn open_position(
        &mut self,
        symbol: String,
        side: OrderSide,
        quantity: f64,
        price: f64,
        timestamp: i64,
        rule_id: Option<String>,
        commission: f64,
        slippage: f64,
    ) {
        let cost = price * quantity + commission + slippage;
        self.cash -= cost;
        self.total_commission += commission;
        self.total_slippage += slippage;

        self.positions.insert(symbol.clone(), SimulatedPosition {
            symbol,
            side,
            quantity,
            entry_price: price,
            entry_time: timestamp,
            entry_rule_id: rule_id,
            commission_paid: commission,
            max_favorable_excursion: 0.0,
            max_adverse_excursion: 0.0,
        });
    }

    fn close_position(
        &mut self,
        symbol: &str,
        price: f64,
        timestamp: i64,
        rule_id: Option<String>,
        commission: f64,
        slippage: f64,
    ) -> Option<BacktestTrade> {
        if let Some(position) = self.positions.remove(symbol) {
            let proceeds = price * position.quantity - commission - slippage;
            self.cash += proceeds;
            self.total_commission += commission;
            self.total_slippage += slippage;

            let trade = position.to_trade(price, timestamp, rule_id, commission + slippage);
            self.realized_pnl += trade.pnl;

            Some(trade)
        } else {
            None
        }
    }

    fn update_excursions(&mut self, prices: &HashMap<String, f64>) {
        for position in self.positions.values_mut() {
            if let Some(&price) = prices.get(&position.symbol) {
                let unrealized = position.unrealized_pnl(price);
                if unrealized > position.max_favorable_excursion {
                    position.max_favorable_excursion = unrealized;
                }
                if unrealized < position.max_adverse_excursion {
                    position.max_adverse_excursion = unrealized;
                }
            }
        }
    }
}

/// Backtesting engine.
pub struct BacktestRunner {
    store: Arc<SqliteStore>,
    strategy_engine: Arc<StrategyEngine>,
    /// Running backtests (backtest_id -> cancel flag)
    running: DashMap<String, bool>,
}

impl BacktestRunner {
    /// Create a new backtest runner.
    pub fn new(store: Arc<SqliteStore>, strategy_engine: Arc<StrategyEngine>) -> Self {
        Self {
            store,
            strategy_engine,
            running: DashMap::new(),
        }
    }

    /// Run a backtest with the given configuration.
    pub fn run_backtest(&self, config: BacktestConfig) -> Result<BacktestResult, BacktestError> {
        // Validate config
        if config.end_time <= config.start_time {
            return Err(BacktestError::InvalidConfig("End time must be after start time".to_string()));
        }
        if config.initial_balance <= 0.0 {
            return Err(BacktestError::InvalidConfig("Initial balance must be positive".to_string()));
        }

        // Get strategy
        let strategy = self.store.get_strategy(&config.strategy_id)
            .ok_or_else(|| BacktestError::StrategyNotFound(config.strategy_id.clone()))?;

        // Create result
        let mut result = BacktestResult::new(config.strategy_id.clone(), config.clone());
        result.start();

        // Track this backtest
        self.running.insert(result.id.clone(), false);

        // Get symbols to test
        let symbols = if config.symbols.is_empty() {
            strategy.symbols.clone()
        } else {
            config.symbols.clone()
        };

        if symbols.is_empty() {
            result.fail("No symbols to backtest".to_string());
            self.running.remove(&result.id);
            return Ok(result);
        }

        // Run the simulation
        match self.simulate(&mut result, &strategy, &symbols) {
            Ok(()) => {
                info!(
                    "Backtest {} completed: {} trades, {:.2}% return",
                    result.id, result.trades.len(), result.metrics.total_return_pct
                );
            }
            Err(e) => {
                warn!("Backtest {} failed: {}", result.id, e);
                result.fail(e.to_string());
            }
        }

        // Clean up
        self.running.remove(&result.id);

        // Save result to database
        if let Err(e) = self.store.create_backtest_result(&result) {
            warn!("Failed to save backtest result: {}", e);
        }

        Ok(result)
    }

    /// Cancel a running backtest.
    pub fn cancel_backtest(&self, backtest_id: &str) -> bool {
        if let Some(mut entry) = self.running.get_mut(backtest_id) {
            *entry = true;
            true
        } else {
            false
        }
    }

    /// Check if backtest was cancelled.
    fn is_cancelled(&self, backtest_id: &str) -> bool {
        self.running.get(backtest_id).map(|v| *v).unwrap_or(false)
    }

    /// Run the simulation.
    fn simulate(
        &self,
        result: &mut BacktestResult,
        strategy: &TradingStrategy,
        symbols: &[String],
    ) -> Result<(), BacktestError> {
        let config = &result.config;
        let mut portfolio = SimulatedPortfolio::new(config.initial_balance);
        let mut daily_returns: Vec<f64> = Vec::new();
        let mut prev_equity = config.initial_balance;
        let mut max_drawdown = 0.0;
        let mut drawdown_sum = 0.0;
        let mut drawdown_count = 0;

        // Get historical data for all symbols
        let mut historical_data: HashMap<String, Vec<BacktestCandle>> = HashMap::new();
        for symbol in symbols {
            let candles = self.get_historical_candles(
                symbol,
                config.start_time,
                config.end_time,
                config.candle_interval,
            )?;
            if candles.is_empty() {
                return Err(BacktestError::NoHistoricalData {
                    symbol: symbol.clone(),
                    start: config.start_time,
                    end: config.end_time,
                });
            }
            historical_data.insert(symbol.clone(), candles);
        }

        // Build timestamp index (union of all candle timestamps)
        let mut timestamps: Vec<i64> = historical_data.values()
            .flat_map(|candles| candles.iter().map(|c| c.timestamp))
            .collect();
        timestamps.sort();
        timestamps.dedup();

        // Track buy-and-hold for comparison
        let mut bnh_start_prices: HashMap<String, f64> = HashMap::new();

        // Sample equity curve (limit to ~1000 points)
        let sample_interval = std::cmp::max(1, timestamps.len() / 1000);
        let mut sample_counter = 0;

        // Iterate through time
        let mut last_day = 0i64;
        for (i, &timestamp) in timestamps.iter().enumerate() {
            // Check cancellation
            if self.is_cancelled(&result.id) {
                return Err(BacktestError::Cancelled);
            }

            // Get current prices
            let mut current_prices: HashMap<String, f64> = HashMap::new();
            for (symbol, candles) in &historical_data {
                // Get the most recent candle at or before this timestamp
                if let Some(candle) = candles.iter().filter(|c| c.timestamp <= timestamp).last() {
                    current_prices.insert(symbol.clone(), candle.close);

                    // Record start prices for buy-and-hold
                    if i == 0 {
                        bnh_start_prices.insert(symbol.clone(), candle.close);
                    }
                }
            }

            // Update portfolio state
            portfolio.update_excursions(&current_prices);
            let equity = portfolio.equity(&current_prices);

            // Track peak and drawdown
            if equity > portfolio.peak_equity {
                portfolio.peak_equity = equity;
            }
            let drawdown = portfolio.drawdown_pct(&current_prices);
            if drawdown > max_drawdown {
                max_drawdown = drawdown;
            }
            drawdown_sum += drawdown;
            drawdown_count += 1;

            // Track daily returns
            let current_day = timestamp / (24 * 60 * 60 * 1000);
            if current_day != last_day && last_day != 0 {
                let daily_return = (equity - prev_equity) / prev_equity;
                daily_returns.push(daily_return);
                prev_equity = equity;
            }
            last_day = current_day;

            // Track time
            portfolio.total_time += config.candle_interval as i64 * 1000;
            if !portfolio.positions.is_empty() {
                portfolio.time_with_positions += config.candle_interval as i64 * 1000;
            }

            // Build indicator snapshots for each symbol
            for symbol in symbols {
                if let Some(&price) = current_prices.get(symbol) {
                    // For simplicity, create a basic snapshot
                    // In production, we'd calculate all indicators from historical data
                    let snapshot = self.build_snapshot_from_history(
                        symbol,
                        timestamp,
                        &historical_data,
                    );
                    self.strategy_engine.update_snapshot(symbol, snapshot);
                }
            }

            // Evaluate strategy
            for symbol in symbols {
                if let Some(&price) = current_prices.get(symbol) {
                    self.process_signals(
                        &mut portfolio,
                        &mut result.trades,
                        strategy,
                        symbol,
                        price,
                        timestamp,
                        config,
                    );
                }
            }

            // Sample equity curve
            sample_counter += 1;
            if sample_counter >= sample_interval {
                sample_counter = 0;
                result.equity_curve.push(EquityPoint {
                    timestamp,
                    equity,
                    cash: portfolio.cash,
                    positions_value: portfolio.positions_value(&current_prices),
                    realized_pnl: portfolio.realized_pnl,
                    unrealized_pnl: portfolio.unrealized_pnl(&current_prices),
                    drawdown_pct: drawdown,
                });
            }
        }

        // Close any remaining positions at end
        let final_prices: HashMap<String, f64> = historical_data.iter()
            .filter_map(|(symbol, candles)| {
                candles.last().map(|c| (symbol.clone(), c.close))
            })
            .collect();

        let open_symbols: Vec<String> = portfolio.positions.keys().cloned().collect();
        for symbol in open_symbols {
            if let Some(&price) = final_prices.get(&symbol) {
                let commission = price * portfolio.positions[&symbol].quantity * config.commission_rate;
                let slippage = price * portfolio.positions[&symbol].quantity * config.slippage_pct;
                if let Some(trade) = portfolio.close_position(&symbol, price, config.end_time, None, commission, slippage) {
                    result.trades.push(trade);
                }
            }
        }

        // Calculate final metrics
        let final_equity = portfolio.equity(&final_prices);
        result.final_balance = final_equity;
        result.metrics = self.calculate_metrics(
            &result.trades,
            config.initial_balance,
            final_equity,
            &daily_returns,
            max_drawdown,
            drawdown_sum / drawdown_count.max(1) as f64,
            portfolio.total_commission,
            portfolio.total_slippage,
            portfolio.time_with_positions as f64 / portfolio.total_time.max(1) as f64 * 100.0,
            config.duration_days(),
        );

        // Calculate buy-and-hold comparison
        result.buy_and_hold = Some(self.calculate_buy_and_hold(
            &bnh_start_prices,
            &final_prices,
            config.initial_balance,
            &result.metrics,
        ));

        // Monte Carlo simulation (if enabled)
        if let Some(runs) = config.monte_carlo_runs {
            result.monte_carlo = Some(self.run_monte_carlo(&result.trades, config.initial_balance, runs));
        }

        result.complete(final_equity);
        Ok(())
    }

    /// Build indicator snapshot from historical data.
    fn build_snapshot_from_history(
        &self,
        symbol: &str,
        timestamp: i64,
        historical_data: &HashMap<String, Vec<BacktestCandle>>,
    ) -> IndicatorSnapshot {
        let candles = historical_data.get(symbol);
        let relevant_candles: Vec<&BacktestCandle> = candles
            .map(|c| c.iter().filter(|candle| candle.timestamp <= timestamp).collect())
            .unwrap_or_default();

        let current_price = relevant_candles.last().map(|c| c.close).unwrap_or(0.0);
        let mut snapshot = IndicatorSnapshot::new(current_price);

        // Calculate indicators from historical data
        if relevant_candles.len() >= 14 {
            let closes: Vec<f64> = relevant_candles.iter().map(|c| c.close).collect();

            // RSI
            snapshot.rsi = self.calculate_rsi(&closes, 14);

            // SMA
            if let Some(sma) = self.calculate_sma(&closes, 20) {
                snapshot.sma.insert(20, sma);
            }
            if let Some(sma) = self.calculate_sma(&closes, 50) {
                snapshot.sma.insert(50, sma);
            }
            if let Some(sma) = self.calculate_sma(&closes, 200) {
                snapshot.sma.insert(200, sma);
            }

            // EMA
            if let Some(ema) = self.calculate_ema(&closes, 12) {
                snapshot.ema.insert(12, ema);
            }
            if let Some(ema) = self.calculate_ema(&closes, 26) {
                snapshot.ema.insert(26, ema);
            }

            // MACD
            if let (Some(ema12), Some(ema26)) = (snapshot.ema.get(&12), snapshot.ema.get(&26)) {
                snapshot.macd = Some(ema12 - ema26);
            }

            // Bollinger Bands (20-period, 2 std dev)
            if closes.len() >= 20 {
                let sma20 = self.calculate_sma(&closes, 20).unwrap_or(0.0);
                let std_dev = self.calculate_std_dev(&closes[closes.len()-20..], sma20);
                snapshot.bollinger_upper = Some(sma20 + 2.0 * std_dev);
                snapshot.bollinger_middle = Some(sma20);
                snapshot.bollinger_lower = Some(sma20 - 2.0 * std_dev);
            }
        }

        snapshot
    }

    /// Calculate RSI.
    fn calculate_rsi(&self, prices: &[f64], period: usize) -> Option<f64> {
        if prices.len() < period + 1 {
            return None;
        }

        let mut gains = 0.0;
        let mut losses = 0.0;

        for i in (prices.len() - period)..prices.len() {
            let change = prices[i] - prices[i - 1];
            if change > 0.0 {
                gains += change;
            } else {
                losses -= change;
            }
        }

        let avg_gain = gains / period as f64;
        let avg_loss = losses / period as f64;

        if avg_loss == 0.0 {
            return Some(100.0);
        }

        let rs = avg_gain / avg_loss;
        Some(100.0 - (100.0 / (1.0 + rs)))
    }

    /// Calculate SMA.
    fn calculate_sma(&self, prices: &[f64], period: usize) -> Option<f64> {
        if prices.len() < period {
            return None;
        }
        let sum: f64 = prices[prices.len() - period..].iter().sum();
        Some(sum / period as f64)
    }

    /// Calculate EMA.
    fn calculate_ema(&self, prices: &[f64], period: usize) -> Option<f64> {
        if prices.len() < period {
            return None;
        }

        let multiplier = 2.0 / (period as f64 + 1.0);
        let mut ema = prices[0];

        for price in prices.iter().skip(1) {
            ema = (price - ema) * multiplier + ema;
        }

        Some(ema)
    }

    /// Calculate standard deviation.
    fn calculate_std_dev(&self, values: &[f64], mean: f64) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        let variance: f64 = values.iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>() / values.len() as f64;
        variance.sqrt()
    }

    /// Process strategy signals for a symbol.
    fn process_signals(
        &self,
        portfolio: &mut SimulatedPortfolio,
        trades: &mut Vec<BacktestTrade>,
        strategy: &TradingStrategy,
        symbol: &str,
        price: f64,
        timestamp: i64,
        config: &BacktestConfig,
    ) {
        // Get current position
        let has_position = portfolio.positions.contains_key(symbol);

        // Evaluate strategy signals (returns all signals, filter for current symbol)
        let signals = match self.strategy_engine.evaluate_strategy(strategy) {
            Ok(sigs) => sigs.into_iter().filter(|s| s.symbol == symbol).collect::<Vec<_>>(),
            Err(_) => return,
        };

        for signal in signals {
            let action = &signal.action;

            // Check if we should enter or exit
            match action.action_type {
                crate::types::RuleActionType::MarketBuy | crate::types::RuleActionType::LimitBuy => {
                    if !has_position {
                        // Calculate position size
                        let size = self.calculate_position_size(
                            portfolio.cash,
                            price,
                            &action.size_type,
                            action.size_value,
                        );

                        if size > 0.0 {
                            let commission = size * price * config.commission_rate;
                            let slippage = size * price * config.slippage_pct;

                            portfolio.open_position(
                                symbol.to_string(),
                                OrderSide::Buy,
                                size,
                                price,
                                timestamp,
                                Some(signal.rule_id.clone()),
                                commission,
                                slippage,
                            );

                            debug!("Opened long {} @ {} (qty: {})", symbol, price, size);
                        }
                    }
                }
                crate::types::RuleActionType::MarketSell | crate::types::RuleActionType::LimitSell => {
                    if !has_position {
                        // Short selling (if margin enabled)
                        if config.enable_margin {
                            let size = self.calculate_position_size(
                                portfolio.cash,
                                price,
                                &action.size_type,
                                action.size_value,
                            );

                            if size > 0.0 {
                                let commission = size * price * config.commission_rate;
                                let slippage = size * price * config.slippage_pct;

                                portfolio.open_position(
                                    symbol.to_string(),
                                    OrderSide::Sell,
                                    size,
                                    price,
                                    timestamp,
                                    Some(signal.rule_id.clone()),
                                    commission,
                                    slippage,
                                );

                                debug!("Opened short {} @ {} (qty: {})", symbol, price, size);
                            }
                        }
                    }
                }
                crate::types::RuleActionType::ClosePosition | crate::types::RuleActionType::ClosePartial => {
                    if has_position {
                        let qty = portfolio.positions.get(symbol).map(|p| p.quantity).unwrap_or(0.0);
                        let commission = qty * price * config.commission_rate;
                        let slippage = qty * price * config.slippage_pct;

                        if let Some(trade) = portfolio.close_position(
                            symbol,
                            price,
                            timestamp,
                            Some(signal.rule_id.clone()),
                            commission,
                            slippage,
                        ) {
                            debug!(
                                "Closed {} @ {} (P&L: {:.2})",
                                symbol, price, trade.pnl
                            );
                            trades.push(trade);
                        }
                    }
                }
            }
        }
    }

    /// Calculate position size based on sizing rules.
    fn calculate_position_size(
        &self,
        available_cash: f64,
        price: f64,
        size_type: &crate::types::PositionSizeType,
        size_value: f64,
    ) -> f64 {
        match size_type {
            crate::types::PositionSizeType::FixedAmount => {
                let amount = size_value.min(available_cash * 0.95);
                amount / price
            }
            crate::types::PositionSizeType::PortfolioPercent => {
                let amount = available_cash * (size_value / 100.0).min(0.95);
                amount / price
            }
            crate::types::PositionSizeType::RiskPercent => {
                // Risk-based: size_value is the % of portfolio to risk
                // Assuming 2% stop loss
                let risk_amount = available_cash * (size_value / 100.0);
                let stop_distance = price * 0.02;
                risk_amount / stop_distance
            }
            crate::types::PositionSizeType::FixedUnits => size_value,
        }
    }

    /// Get historical candles for a symbol.
    fn get_historical_candles(
        &self,
        symbol: &str,
        start: i64,
        end: i64,
        interval_seconds: u32,
    ) -> Result<Vec<BacktestCandle>, BacktestError> {
        // Try to get from chart store first
        let interval_str = match interval_seconds {
            60 => "1m",
            300 => "5m",
            900 => "15m",
            3600 => "1h",
            14400 => "4h",
            86400 => "1d",
            _ => "5m",
        };

        if let Some(candles) = self.store.get_chart_data(symbol, interval_str, start, end) {
            return Ok(candles.iter().map(|c| BacktestCandle {
                timestamp: c.timestamp,
                open: c.open,
                high: c.high,
                low: c.low,
                close: c.close,
                volume: c.volume,
            }).collect());
        }

        // Generate synthetic data for testing (in production, fetch from API)
        warn!("No historical data found for {}, generating synthetic data", symbol);
        Ok(self.generate_synthetic_candles(start, end, interval_seconds))
    }

    /// Generate synthetic candles for testing.
    fn generate_synthetic_candles(&self, start: i64, end: i64, interval_seconds: u32) -> Vec<BacktestCandle> {
        let mut candles = Vec::new();
        let mut rng = rand::thread_rng();
        let mut price = 100.0; // Start price
        let interval_ms = interval_seconds as i64 * 1000;

        let mut timestamp = start;
        while timestamp <= end {
            // Random walk with drift
            let change = rng.gen_range(-0.02..0.02);
            price *= 1.0 + change;

            let high = price * (1.0 + rng.gen_range(0.0..0.01));
            let low = price * (1.0 - rng.gen_range(0.0..0.01));
            let open = price * (1.0 + rng.gen_range(-0.005..0.005));
            let volume = rng.gen_range(1000.0..100000.0);

            candles.push(BacktestCandle {
                timestamp,
                open,
                high,
                low,
                close: price,
                volume,
            });

            timestamp += interval_ms;
        }

        candles
    }

    /// Calculate performance metrics.
    fn calculate_metrics(
        &self,
        trades: &[BacktestTrade],
        initial_balance: f64,
        final_balance: f64,
        daily_returns: &[f64],
        max_drawdown: f64,
        avg_drawdown: f64,
        total_commission: f64,
        total_slippage: f64,
        time_in_market: f64,
        duration_days: f64,
    ) -> BacktestMetrics {
        let total_pnl = final_balance - initial_balance;
        let total_return_pct = (final_balance / initial_balance - 1.0) * 100.0;
        let annualized_return_pct = if duration_days > 0.0 {
            ((final_balance / initial_balance).powf(365.0 / duration_days) - 1.0) * 100.0
        } else {
            0.0
        };

        // Trade statistics
        let total_trades = trades.len() as u32;
        let winning_trades = trades.iter().filter(|t| t.is_winner).count() as u32;
        let losing_trades = total_trades - winning_trades;
        let win_rate_pct = if total_trades > 0 {
            winning_trades as f64 / total_trades as f64 * 100.0
        } else {
            0.0
        };

        let gross_profit: f64 = trades.iter().filter(|t| t.pnl > 0.0).map(|t| t.pnl).sum();
        let gross_loss: f64 = trades.iter().filter(|t| t.pnl < 0.0).map(|t| t.pnl.abs()).sum();
        let profit_factor = if gross_loss > 0.0 { gross_profit / gross_loss } else { f64::INFINITY };

        let avg_trade_pnl = if total_trades > 0 { total_pnl / total_trades as f64 } else { 0.0 };
        let avg_win = if winning_trades > 0 {
            gross_profit / winning_trades as f64
        } else {
            0.0
        };
        let avg_loss = if losing_trades > 0 {
            gross_loss / losing_trades as f64
        } else {
            0.0
        };

        let largest_win = trades.iter().map(|t| t.pnl).fold(0.0f64, |a, b| a.max(b));
        let largest_loss = trades.iter().map(|t| t.pnl).fold(0.0f64, |a, b| a.min(b));

        let avg_trade_duration_ms = if total_trades > 0 {
            trades.iter().filter_map(|t| t.duration_ms()).sum::<i64>() / total_trades as i64
        } else {
            0
        };

        // Streaks
        let mut max_consecutive_wins = 0u32;
        let mut max_consecutive_losses = 0u32;
        let mut current_wins = 0u32;
        let mut current_losses = 0u32;
        let mut current_streak = 0i32;

        for trade in trades {
            if trade.is_winner {
                current_wins += 1;
                current_losses = 0;
                current_streak = current_wins as i32;
                if current_wins > max_consecutive_wins {
                    max_consecutive_wins = current_wins;
                }
            } else {
                current_losses += 1;
                current_wins = 0;
                current_streak = -(current_losses as i32);
                if current_losses > max_consecutive_losses {
                    max_consecutive_losses = current_losses;
                }
            }
        }

        // Risk metrics
        let daily_volatility = self.calculate_std_dev(daily_returns, 0.0);
        let annual_volatility = daily_volatility * (252.0f64).sqrt();

        let risk_free_rate = 0.02; // 2% annual
        let sharpe_ratio = if annual_volatility > 0.0 {
            (annualized_return_pct / 100.0 - risk_free_rate) / annual_volatility
        } else {
            0.0
        };

        // Sortino ratio (downside deviation)
        let downside_returns: Vec<f64> = daily_returns.iter()
            .filter(|&&r| r < 0.0)
            .cloned()
            .collect();
        let downside_deviation = self.calculate_std_dev(&downside_returns, 0.0) * (252.0f64).sqrt();
        let sortino_ratio = if downside_deviation > 0.0 {
            (annualized_return_pct / 100.0 - risk_free_rate) / downside_deviation
        } else {
            0.0
        };

        // Calmar ratio
        let calmar_ratio = if max_drawdown > 0.0 {
            annualized_return_pct / max_drawdown
        } else {
            0.0
        };

        // Expectancy
        let expectancy = (win_rate_pct / 100.0) * avg_win - (1.0 - win_rate_pct / 100.0) * avg_loss;

        BacktestMetrics {
            total_return_pct,
            annualized_return_pct,
            total_pnl,
            gross_profit,
            gross_loss,
            profit_factor,
            max_drawdown_pct: max_drawdown,
            max_drawdown: initial_balance * max_drawdown / 100.0,
            avg_drawdown_pct: avg_drawdown,
            sharpe_ratio,
            sortino_ratio,
            calmar_ratio,
            daily_volatility: daily_volatility * 100.0,
            total_trades,
            winning_trades,
            losing_trades,
            win_rate_pct,
            avg_trade_pnl,
            avg_win,
            avg_loss,
            largest_win,
            largest_loss,
            avg_trade_duration_ms,
            expectancy,
            max_consecutive_wins,
            max_consecutive_losses,
            current_streak,
            time_in_market_pct: time_in_market,
            total_commission,
            total_slippage,
        }
    }

    /// Calculate buy-and-hold comparison.
    fn calculate_buy_and_hold(
        &self,
        start_prices: &HashMap<String, f64>,
        end_prices: &HashMap<String, f64>,
        initial_balance: f64,
        strategy_metrics: &BacktestMetrics,
    ) -> BuyAndHoldComparison {
        // Calculate average buy-and-hold return
        let mut total_return = 0.0;
        let mut count = 0;

        for (symbol, start_price) in start_prices {
            if let Some(end_price) = end_prices.get(symbol) {
                total_return += (end_price / start_price - 1.0) * 100.0;
                count += 1;
            }
        }

        let bnh_return_pct = if count > 0 { total_return / count as f64 } else { 0.0 };

        BuyAndHoldComparison {
            bnh_return_pct,
            outperformance_pct: strategy_metrics.total_return_pct - bnh_return_pct,
            strategy_max_dd: strategy_metrics.max_drawdown_pct,
            bnh_max_dd: 0.0, // Would need to calculate
            strategy_sharpe: strategy_metrics.sharpe_ratio,
            bnh_sharpe: 0.0, // Would need to calculate
        }
    }

    /// Run Monte Carlo simulation.
    fn run_monte_carlo(
        &self,
        trades: &[BacktestTrade],
        initial_balance: f64,
        num_runs: u32,
    ) -> MonteCarloResults {
        if trades.is_empty() {
            return MonteCarloResults {
                num_runs,
                return_p5: 0.0,
                return_p25: 0.0,
                return_p50: 0.0,
                return_p75: 0.0,
                return_p95: 0.0,
                max_dd_p5: 0.0,
                max_dd_p50: 0.0,
                max_dd_p95: 0.0,
                probability_of_profit: 0.0,
                probability_of_ruin: 0.0,
            };
        }

        let mut rng = rand::thread_rng();
        let mut returns: Vec<f64> = Vec::with_capacity(num_runs as usize);
        let mut max_drawdowns: Vec<f64> = Vec::with_capacity(num_runs as usize);
        let mut profitable_runs = 0u32;
        let mut ruin_runs = 0u32;

        // Collect trade P&Ls
        let pnls: Vec<f64> = trades.iter().map(|t| t.pnl).collect();

        for _ in 0..num_runs {
            // Shuffle trades and simulate
            let mut shuffled = pnls.clone();
            shuffled.shuffle(&mut rng);

            let mut balance = initial_balance;
            let mut peak = initial_balance;
            let mut max_dd = 0.0;

            for pnl in &shuffled {
                balance += pnl;
                if balance > peak {
                    peak = balance;
                }
                let dd = (peak - balance) / peak * 100.0;
                if dd > max_dd {
                    max_dd = dd;
                }
            }

            let ret = (balance / initial_balance - 1.0) * 100.0;
            returns.push(ret);
            max_drawdowns.push(max_dd);

            if ret > 0.0 {
                profitable_runs += 1;
            }
            if max_dd > 50.0 {
                ruin_runs += 1;
            }
        }

        // Sort for percentiles
        returns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        max_drawdowns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let percentile = |arr: &[f64], p: f64| -> f64 {
            let idx = ((arr.len() as f64) * p).floor() as usize;
            arr.get(idx.min(arr.len() - 1)).copied().unwrap_or(0.0)
        };

        MonteCarloResults {
            num_runs,
            return_p5: percentile(&returns, 0.05),
            return_p25: percentile(&returns, 0.25),
            return_p50: percentile(&returns, 0.50),
            return_p75: percentile(&returns, 0.75),
            return_p95: percentile(&returns, 0.95),
            max_dd_p5: percentile(&max_drawdowns, 0.05),
            max_dd_p50: percentile(&max_drawdowns, 0.50),
            max_dd_p95: percentile(&max_drawdowns, 0.95),
            probability_of_profit: profitable_runs as f64 / num_runs as f64 * 100.0,
            probability_of_ruin: ruin_runs as f64 / num_runs as f64 * 100.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{BacktestConfig, StrategyStatus, TradingStrategy};

    fn create_test_store() -> Arc<SqliteStore> {
        Arc::new(SqliteStore::new_in_memory().unwrap())
    }

    fn create_test_strategy() -> TradingStrategy {
        TradingStrategy {
            id: "test-strategy".to_string(),
            portfolio_id: "test-portfolio".to_string(),
            name: "Test Strategy".to_string(),
            description: None,
            symbols: vec!["BTC".to_string()],
            asset_class: None,
            rules: Vec::new(),
            status: StrategyStatus::Active,
            cooldown_seconds: 0,
            max_positions: 1,
            max_position_size_pct: 0.1,
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            realized_pnl: 0.0,
            last_trade_at: None,
            created_at: chrono::Utc::now().timestamp_millis(),
            updated_at: chrono::Utc::now().timestamp_millis(),
        }
    }

    #[test]
    fn test_backtest_config_creation() {
        let start = 1704067200000; // 2024-01-01
        let end = 1706745600000;   // 2024-02-01
        let config = BacktestConfig::new("strategy-1".to_string(), start, end);

        assert_eq!(config.strategy_id, "strategy-1");
        assert_eq!(config.initial_balance, 10_000.0);
        assert!(config.duration_days() > 30.0);
    }

    #[test]
    fn test_backtest_trade_open_close() {
        let mut trade = BacktestTrade::open(
            "BTC".to_string(),
            OrderSide::Buy,
            50000.0,
            0.1,
            1704067200000,
            Some("rule-1".to_string()),
            5.0,
        );

        assert!(trade.is_open());
        assert_eq!(trade.pnl, -5.0); // Just commission

        trade.close(52000.0, 1704153600000, Some("rule-2".to_string()), 5.2);

        assert!(!trade.is_open());
        // Gross P&L: (52000 - 50000) * 0.1 = 200
        // Net P&L: 200 - 5.0 - 5.2 = 189.8
        assert!((trade.pnl - 189.8).abs() < 0.01);
        assert!(trade.is_winner);
    }

    #[test]
    fn test_simulated_portfolio() {
        let mut portfolio = SimulatedPortfolio::new(10000.0);

        assert_eq!(portfolio.cash, 10000.0);
        assert!(portfolio.positions.is_empty());

        // Open a position
        portfolio.open_position(
            "BTC".to_string(),
            OrderSide::Buy,
            0.1,
            50000.0,
            1704067200000,
            None,
            5.0,
            2.5,
        );

        // Cash reduced by: price * qty + commission + slippage = 5000 + 5 + 2.5 = 5007.5
        assert!((portfolio.cash - 4992.5).abs() < 0.01);
        assert!(portfolio.positions.contains_key("BTC"));
    }

    #[test]
    fn test_equity_calculation() {
        let mut portfolio = SimulatedPortfolio::new(10000.0);

        portfolio.open_position(
            "BTC".to_string(),
            OrderSide::Buy,
            0.1,
            50000.0,
            1704067200000,
            None,
            0.0,
            0.0,
        );

        let mut prices = HashMap::new();
        prices.insert("BTC".to_string(), 55000.0);

        let equity = portfolio.equity(&prices);
        // Cash: 5000 + Position: 0.1 * 50000 + unrealized: (55000-50000)*0.1 = 5500
        // Total: 5000 + 5500 = 10500
        assert!((equity - 10500.0).abs() < 0.01);
    }

    #[test]
    fn test_indicator_calculation_rsi() {
        let store = create_test_store();
        let strategy_engine = Arc::new(StrategyEngine::new(store.clone()));
        let runner = BacktestRunner::new(store, strategy_engine);

        // Create a series of prices
        let prices: Vec<f64> = (0..20).map(|i| 100.0 + (i as f64) * 0.5).collect();

        let rsi = runner.calculate_rsi(&prices, 14);
        assert!(rsi.is_some());
        assert!(rsi.unwrap() > 50.0); // Uptrend should have RSI > 50
    }

    #[test]
    fn test_indicator_calculation_sma() {
        let store = create_test_store();
        let strategy_engine = Arc::new(StrategyEngine::new(store.clone()));
        let runner = BacktestRunner::new(store, strategy_engine);

        let prices = vec![10.0, 20.0, 30.0, 40.0, 50.0];

        let sma = runner.calculate_sma(&prices, 3);
        assert!(sma.is_some());
        // SMA of last 3: (30 + 40 + 50) / 3 = 40
        assert!((sma.unwrap() - 40.0).abs() < 0.01);
    }

    #[test]
    fn test_monte_carlo_basic() {
        let store = create_test_store();
        let strategy_engine = Arc::new(StrategyEngine::new(store.clone()));
        let runner = BacktestRunner::new(store, strategy_engine);

        // Create sample trades
        let trades = vec![
            BacktestTrade {
                id: "1".to_string(),
                symbol: "BTC".to_string(),
                side: OrderSide::Buy,
                entry_price: 50000.0,
                exit_price: Some(51000.0),
                quantity: 0.1,
                entry_time: 0,
                exit_time: Some(1),
                pnl: 100.0,
                pnl_pct: 2.0,
                commission: 0.0,
                entry_rule_id: None,
                exit_rule_id: None,
                is_winner: true,
                max_favorable_excursion: 100.0,
                max_adverse_excursion: -50.0,
            },
            BacktestTrade {
                id: "2".to_string(),
                symbol: "BTC".to_string(),
                side: OrderSide::Buy,
                entry_price: 50000.0,
                exit_price: Some(49500.0),
                quantity: 0.1,
                entry_time: 2,
                exit_time: Some(3),
                pnl: -50.0,
                pnl_pct: -1.0,
                commission: 0.0,
                entry_rule_id: None,
                exit_rule_id: None,
                is_winner: false,
                max_favorable_excursion: 50.0,
                max_adverse_excursion: -50.0,
            },
        ];

        let mc = runner.run_monte_carlo(&trades, 10000.0, 100);

        assert_eq!(mc.num_runs, 100);
        assert!(mc.probability_of_profit > 0.0);
    }

    #[test]
    fn test_backtest_result_lifecycle() {
        let config = BacktestConfig::new(
            "test-strategy".to_string(),
            1704067200000,
            1706745600000,
        );

        let mut result = BacktestResult::new("test-strategy".to_string(), config);

        assert_eq!(result.status, BacktestStatus::Pending);

        result.start();
        assert_eq!(result.status, BacktestStatus::Running);
        assert!(result.started_at.is_some());

        result.complete(11000.0);
        assert_eq!(result.status, BacktestStatus::Completed);
        assert_eq!(result.final_balance, 11000.0);
        assert!(result.completed_at.is_some());
    }

    #[test]
    fn test_position_size_calculation() {
        let store = create_test_store();
        let strategy_engine = Arc::new(StrategyEngine::new(store.clone()));
        let runner = BacktestRunner::new(store, strategy_engine);

        // Fixed amount
        let size = runner.calculate_position_size(
            10000.0,
            100.0,
            &crate::types::PositionSizeType::FixedAmount,
            1000.0,
        );
        assert!((size - 10.0).abs() < 0.01); // 1000 / 100 = 10 units

        // Portfolio percent
        let size = runner.calculate_position_size(
            10000.0,
            100.0,
            &crate::types::PositionSizeType::PortfolioPercent,
            10.0,
        );
        assert!((size - 10.0).abs() < 0.01); // 10% of 10000 = 1000, 1000/100 = 10 units

        // Fixed units
        let size = runner.calculate_position_size(
            10000.0,
            100.0,
            &crate::types::PositionSizeType::FixedUnits,
            5.0,
        );
        assert!((size - 5.0).abs() < 0.01);
    }
}
