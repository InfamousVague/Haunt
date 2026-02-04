//! Grandma Bot - Conservative rule-based trading strategy
//!
//! Uses 50/200 SMA crossover (golden/death cross) with RSI confirmation.
//! Trades slowly and carefully, like a wise grandmother.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::RwLock;
use tracing::{debug, info};

use crate::error::AppError;
use crate::types::AssetClass;

use super::{
    BotConfig, BotPersonality, DecisionContext, SellReason, SignalStrength, TradeDecision,
    TradeSignal, TradingBot,
};

/// Grandma's internal state for tracking trades
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GrandmaState {
    /// Last trade timestamp per symbol
    pub last_trade: HashMap<String, i64>,
    /// Whether we saw a golden cross recently (and haven't acted on it)
    pub pending_golden_cross: HashMap<String, bool>,
    /// Whether we saw a death cross recently (and haven't acted on it)
    pub pending_death_cross: HashMap<String, bool>,
    /// Previous SMA short values for detecting crossovers
    pub prev_sma_short: HashMap<String, f64>,
    /// Previous SMA long values for detecting crossovers
    pub prev_sma_long: HashMap<String, f64>,
}

/// Grandma Bot - Conservative, patient, rule-based trader
pub struct GrandmaBot {
    config: BotConfig,
    state: RwLock<GrandmaState>,
}

impl GrandmaBot {
    /// Create a new Grandma bot with default configuration
    pub fn new() -> Self {
        Self {
            config: BotConfig::grandma(),
            state: RwLock::new(GrandmaState::default()),
        }
    }

    /// Create a new Grandma bot with custom configuration
    pub fn with_config(config: BotConfig) -> Self {
        Self {
            config,
            state: RwLock::new(GrandmaState::default()),
        }
    }

    /// Check if we can trade this symbol today
    fn can_trade_today(&self, symbol: &str, current_timestamp: i64, trades_today: u32) -> bool {
        if trades_today >= self.config.max_trades_per_day {
            debug!("Grandma: Max trades per day reached for {}", symbol);
            return false;
        }

        let state = self.state.read().unwrap();
        if let Some(last_trade) = state.last_trade.get(symbol) {
            // Require at least 4 hours between trades
            let min_interval = 4 * 60 * 60; // 4 hours in seconds
            if current_timestamp - last_trade < min_interval {
                debug!(
                    "Grandma: Too soon to trade {} (last trade {} seconds ago)",
                    symbol,
                    current_timestamp - last_trade
                );
                return false;
            }
        }

        true
    }

    /// Detect if a golden cross just occurred
    fn detect_golden_cross(&self, ctx: &DecisionContext) -> bool {
        let (sma_short, sma_long) = match (ctx.sma_short, ctx.sma_long) {
            (Some(s), Some(l)) => (s, l),
            _ => return false,
        };

        let state = self.state.read().unwrap();
        let prev_short = state.prev_sma_short.get(&ctx.symbol);
        let prev_long = state.prev_sma_long.get(&ctx.symbol);

        match (prev_short, prev_long) {
            (Some(&ps), Some(&pl)) => {
                // Golden cross: short was below long, now above
                let was_below = ps < pl;
                let now_above = sma_short > sma_long;
                was_below && now_above
            }
            _ => {
                // First time seeing this symbol, check current state
                sma_short > sma_long
            }
        }
    }

    /// Detect if a death cross just occurred
    fn detect_death_cross(&self, ctx: &DecisionContext) -> bool {
        let (sma_short, sma_long) = match (ctx.sma_short, ctx.sma_long) {
            (Some(s), Some(l)) => (s, l),
            _ => return false,
        };

        let state = self.state.read().unwrap();
        let prev_short = state.prev_sma_short.get(&ctx.symbol);
        let prev_long = state.prev_sma_long.get(&ctx.symbol);

        match (prev_short, prev_long) {
            (Some(&ps), Some(&pl)) => {
                // Death cross: short was above long, now below
                let was_above = ps > pl;
                let now_below = sma_short < sma_long;
                was_above && now_below
            }
            _ => {
                // First time seeing this symbol, check current state
                sma_short < sma_long
            }
        }
    }

    /// Update internal state after analysis
    fn update_sma_state(&self, ctx: &DecisionContext) {
        if let (Some(short), Some(long)) = (ctx.sma_short, ctx.sma_long) {
            let mut state = self.state.write().unwrap();
            state.prev_sma_short.insert(ctx.symbol.clone(), short);
            state.prev_sma_long.insert(ctx.symbol.clone(), long);
        }
    }

    /// Calculate position size based on available cash and risk parameters
    fn calculate_position_size(&self, ctx: &DecisionContext) -> f64 {
        // Position size = min(max_position_size, risk-based size)
        let max_position_value = ctx.portfolio_value * self.config.max_position_size_pct;
        let risk_position_value = ctx.portfolio_value * self.config.risk_per_trade_pct;

        // Use the smaller of the two
        let position_value = max_position_value.min(risk_position_value).min(ctx.available_cash);

        // Convert to quantity
        if ctx.current_price > 0.0 {
            position_value / ctx.current_price
        } else {
            0.0
        }
    }

    /// Analyze buy signals
    fn analyze_buy_signals(&self, ctx: &DecisionContext) -> Vec<TradeSignal> {
        let mut signals = Vec::new();

        // 1. Golden Cross Signal (primary)
        if self.detect_golden_cross(ctx) {
            signals.push(TradeSignal::bullish(
                SignalStrength::Strong,
                0.8,
                "SMA Crossover",
                "50 SMA crossed above 200 SMA (Golden Cross)",
            ));
        } else if ctx.is_golden_cross() {
            // Already in golden cross territory
            signals.push(TradeSignal::bullish(
                SignalStrength::Moderate,
                0.6,
                "SMA Position",
                "Price in bullish territory (50 SMA > 200 SMA)",
            ));
        }

        // 2. RSI Confirmation
        if let Some(rsi) = ctx.rsi {
            if rsi < 30.0 {
                signals.push(TradeSignal::bullish(
                    SignalStrength::Strong,
                    0.8,
                    "RSI",
                    &format!("Oversold conditions (RSI: {:.1})", rsi),
                ));
            } else if rsi < 50.0 {
                signals.push(TradeSignal::bullish(
                    SignalStrength::Moderate,
                    0.6,
                    "RSI",
                    &format!("Room to grow (RSI: {:.1})", rsi),
                ));
            } else if rsi > 70.0 {
                signals.push(TradeSignal::bearish(
                    SignalStrength::Moderate,
                    0.6,
                    "RSI",
                    &format!("Overbought warning (RSI: {:.1})", rsi),
                ));
            }
        }

        // 3. Price above 200 SMA (trend confirmation)
        if ctx.is_above_sma_long() {
            signals.push(TradeSignal::bullish(
                SignalStrength::Moderate,
                0.5,
                "Trend",
                "Price above 200 SMA (bullish trend)",
            ));
        }

        // 4. MACD Confirmation (secondary)
        if ctx.is_macd_bullish() {
            signals.push(TradeSignal::bullish(
                SignalStrength::Weak,
                0.4,
                "MACD",
                "MACD histogram positive",
            ));
        }

        signals
    }

    /// Analyze sell signals
    fn analyze_sell_signals(&self, ctx: &DecisionContext) -> (Vec<TradeSignal>, Option<SellReason>) {
        let mut signals = Vec::new();
        let mut reason = None;

        // 1. Check stop loss
        if let Some(pnl_pct) = ctx.position_pnl_pct() {
            if pnl_pct <= -self.config.stop_loss_pct {
                signals.push(TradeSignal::bearish(
                    SignalStrength::VeryStrong,
                    1.0,
                    "Risk Management",
                    &format!("Stop loss triggered ({:.1}% loss)", pnl_pct * 100.0),
                ));
                reason = Some(SellReason::StopLoss);
                return (signals, reason);
            }

            // 2. Check take profit
            if pnl_pct >= self.config.take_profit_pct {
                signals.push(TradeSignal::bearish(
                    SignalStrength::Strong,
                    0.9,
                    "Risk Management",
                    &format!("Take profit triggered ({:.1}% gain)", pnl_pct * 100.0),
                ));
                reason = Some(SellReason::TakeProfit);
                return (signals, reason);
            }
        }

        // 3. Death Cross Signal
        if self.detect_death_cross(ctx) {
            signals.push(TradeSignal::bearish(
                SignalStrength::Strong,
                0.8,
                "SMA Crossover",
                "50 SMA crossed below 200 SMA (Death Cross)",
            ));
            reason = Some(SellReason::Signal);
        }

        // 4. RSI Overbought (take profits early)
        if let Some(rsi) = ctx.rsi {
            if rsi > 80.0 {
                signals.push(TradeSignal::bearish(
                    SignalStrength::Strong,
                    0.7,
                    "RSI",
                    &format!("Extremely overbought (RSI: {:.1})", rsi),
                ));
                if reason.is_none() {
                    reason = Some(SellReason::TakeProfit);
                }
            }
        }

        // 5. Price below 200 SMA (trend reversal)
        if ctx.is_below_sma_long() && ctx.has_position() {
            signals.push(TradeSignal::bearish(
                SignalStrength::Moderate,
                0.6,
                "Trend",
                "Price fell below 200 SMA",
            ));
            if reason.is_none() {
                reason = Some(SellReason::Signal);
            }
        }

        (signals, reason)
    }

    /// Calculate overall confidence from signals
    fn calculate_confidence(&self, signals: &[TradeSignal]) -> f64 {
        if signals.is_empty() {
            return 0.0;
        }

        // Weight signals by their strength and confidence
        let total_weight: f64 = signals
            .iter()
            .map(|s| s.strength.as_f64() * s.confidence)
            .sum();

        let max_possible_weight = signals.len() as f64 * 1.0; // Max strength * max confidence
        (total_weight / max_possible_weight).min(1.0)
    }
}

impl Default for GrandmaBot {
    fn default() -> Self {
        Self::new()
    }
}

impl TradingBot for GrandmaBot {
    fn name(&self) -> &str {
        &self.config.name
    }

    fn personality(&self) -> BotPersonality {
        BotPersonality::Grandma
    }

    fn config(&self) -> &BotConfig {
        &self.config
    }

    fn supported_asset_classes(&self) -> Vec<AssetClass> {
        self.config.asset_classes.clone()
    }

    fn analyze<'a>(
        &'a self,
        ctx: &'a DecisionContext,
    ) -> Pin<Box<dyn Future<Output = Result<TradeDecision, AppError>> + Send + 'a>> {
        Box::pin(async move {
            // Determine the decision first
            let decision: TradeDecision = if ctx.has_position() {
                // Check if we have a position - analyze sell signals
                let (signals, sell_reason) = self.analyze_sell_signals(ctx);

                // Filter for bearish signals
                let bearish_signals: Vec<_> = signals
                    .iter()
                    .filter(|s| s.direction < 0.0)
                    .cloned()
                    .collect();

                if !bearish_signals.is_empty() {
                    let confidence = self.calculate_confidence(&bearish_signals);

                    // Grandma needs strong conviction to sell
                    if confidence >= 0.5 || sell_reason == Some(SellReason::StopLoss) {
                        let quantity = ctx.current_position.unwrap_or(0.0).abs();

                        info!(
                            "Grandma: SELL {} {} @ {:.2} (confidence: {:.1}%, reason: {:?})",
                            quantity,
                            ctx.symbol,
                            ctx.current_price,
                            confidence * 100.0,
                            sell_reason
                        );

                        TradeDecision::Sell {
                            symbol: ctx.symbol.clone(),
                            quantity,
                            confidence,
                            signals: bearish_signals,
                            reason: sell_reason.unwrap_or(SellReason::Signal),
                        }
                    } else {
                        TradeDecision::Hold {
                            symbol: ctx.symbol.clone(),
                            reason: "Waiting patiently for better exit".to_string(),
                        }
                    }
                } else {
                    TradeDecision::Hold {
                        symbol: ctx.symbol.clone(),
                        reason: "Waiting patiently for better exit".to_string(),
                    }
                }
            } else if !self.can_trade_today(&ctx.symbol, ctx.timestamp, ctx.trades_today) {
                // No position but can't trade today
                TradeDecision::Hold {
                    symbol: ctx.symbol.clone(),
                    reason: "Grandma says: 'Not today, dear. Let's wait.'".to_string(),
                }
            } else {
                // No position - analyze buy signals
                let signals = self.analyze_buy_signals(ctx);

                // Filter for bullish signals and check for bearish warnings
                let bullish_signals: Vec<_> = signals
                    .iter()
                    .filter(|s| s.direction > 0.0)
                    .cloned()
                    .collect();

                let has_bearish_warning = signals.iter().any(|s| s.direction < 0.0);

                if !bullish_signals.is_empty() && !has_bearish_warning {
                    let confidence = self.calculate_confidence(&bullish_signals);

                    // Grandma needs strong conviction to buy
                    // Require at least golden cross or oversold + uptrend
                    let has_golden_cross = bullish_signals
                        .iter()
                        .any(|s| s.source == "SMA Crossover" && s.strength.as_f64() >= 0.8);

                    let has_oversold_uptrend = bullish_signals
                        .iter()
                        .any(|s| s.source == "RSI" && s.reason.contains("Oversold"))
                        && ctx.is_above_sma_long();

                    if confidence >= 0.4 && (has_golden_cross || has_oversold_uptrend) {
                        let quantity = self.calculate_position_size(ctx);

                        if quantity > 0.0 {
                            let stop_loss = Some(ctx.current_price * (1.0 - self.config.stop_loss_pct));
                            let take_profit =
                                Some(ctx.current_price * (1.0 + self.config.take_profit_pct));

                            info!(
                                "Grandma: BUY {} {} @ {:.2} (confidence: {:.1}%)",
                                quantity,
                                ctx.symbol,
                                ctx.current_price,
                                confidence * 100.0
                            );

                            TradeDecision::Buy {
                                symbol: ctx.symbol.clone(),
                                quantity,
                                confidence,
                                signals: bullish_signals,
                                stop_loss,
                                take_profit,
                            }
                        } else {
                            TradeDecision::Hold {
                                symbol: ctx.symbol.clone(),
                                reason: "Grandma says: 'Patience is a virtue, dear.'".to_string(),
                            }
                        }
                    } else {
                        TradeDecision::Hold {
                            symbol: ctx.symbol.clone(),
                            reason: "Grandma says: 'Patience is a virtue, dear.'".to_string(),
                        }
                    }
                } else {
                    TradeDecision::Hold {
                        symbol: ctx.symbol.clone(),
                        reason: "Grandma says: 'Patience is a virtue, dear.'".to_string(),
                    }
                }
            };

            // Update SMA state for next crossover detection (always do this)
            self.update_sma_state(ctx);

            Ok(decision)
        })
    }

    fn on_trade_executed<'a>(
        &'a self,
        symbol: &'a str,
        decision: &'a TradeDecision,
        _execution_price: f64,
    ) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + 'a>> {
        Box::pin(async move {
            if !decision.is_hold() {
                let mut state = self.state.write().unwrap();
                state
                    .last_trade
                    .insert(symbol.to_string(), chrono::Utc::now().timestamp());

                // Clear pending crossover flags
                state.pending_golden_cross.remove(symbol);
                state.pending_death_cross.remove(symbol);
            }
            Ok(())
        })
    }

    fn get_state(&self) -> serde_json::Value {
        let state = self.state.read().unwrap();
        serde_json::to_value(&*state).unwrap_or_default()
    }

    fn restore_state(&mut self, state: serde_json::Value) -> Result<(), AppError> {
        if let Ok(restored) = serde_json::from_value::<GrandmaState>(state) {
            *self.state.write().unwrap() = restored;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_context(symbol: &str, price: f64) -> DecisionContext {
        DecisionContext {
            symbol: symbol.to_string(),
            asset_class: AssetClass::CryptoSpot,
            current_price: price,
            high_24h: Some(price * 1.05),
            low_24h: Some(price * 0.95),
            volume_24h: Some(1_000_000.0),
            price_change_24h_pct: Some(2.0),
            rsi: Some(45.0),
            macd_histogram: Some(0.5),
            macd_crossover: Some(1),
            sma_short: Some(price * 1.02), // 50 SMA above price
            sma_long: Some(price * 0.98),  // 200 SMA below price
            ema_short: Some(price * 1.01),
            ema_long: Some(price * 0.99),
            bb_upper: Some(price * 1.1),
            bb_lower: Some(price * 0.9),
            bb_middle: Some(price),
            atr: Some(price * 0.02),
            adx: Some(25.0),
            volume_ratio: Some(1.2),
            orderbook: None,
            current_position: None,
            position_entry_price: None,
            unrealized_pnl: None,
            trades_today: 0,
            last_trade_timestamp: None,
            available_cash: 50_000.0,
            portfolio_value: 100_000.0,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    #[tokio::test]
    async fn test_grandma_hold_on_neutral() {
        let bot = GrandmaBot::new();
        let ctx = create_test_context("BTC", 50000.0);

        // First call to set up SMA state
        let _ = bot.analyze(&ctx).await;

        // Second call should hold since no crossover happened
        let decision = bot.analyze(&ctx).await.unwrap();
        assert!(decision.is_hold());
    }

    #[tokio::test]
    async fn test_grandma_buy_on_golden_cross() {
        let bot = GrandmaBot::new();

        // Set up previous state with death cross
        {
            let mut state = bot.state.write().unwrap();
            state.prev_sma_short.insert("BTC".to_string(), 48000.0);
            state.prev_sma_long.insert("BTC".to_string(), 50000.0);
        }

        // Now create a golden cross scenario with oversold RSI for stronger signal
        let mut ctx = create_test_context("BTC", 51000.0);
        ctx.sma_short = Some(51000.0); // Now above
        ctx.sma_long = Some(50000.0);  // Below short
        ctx.rsi = Some(25.0);          // Oversold for strong buy signal
        ctx.macd_histogram = None;     // Remove weak MACD signal to boost confidence

        let decision = bot.analyze(&ctx).await.unwrap();
        assert!(decision.is_buy(), "Expected buy on golden cross with oversold RSI");
    }

    #[tokio::test]
    async fn test_grandma_sell_on_stop_loss() {
        let bot = GrandmaBot::new();

        let mut ctx = create_test_context("BTC", 45000.0);
        ctx.current_position = Some(1.0);
        ctx.position_entry_price = Some(50000.0); // 10% loss

        let decision = bot.analyze(&ctx).await.unwrap();
        assert!(decision.is_sell(), "Expected sell on stop loss");

        if let TradeDecision::Sell { reason, .. } = decision {
            assert!(matches!(reason, SellReason::StopLoss));
        }
    }

    #[tokio::test]
    async fn test_grandma_respects_max_trades() {
        let bot = GrandmaBot::new();

        let mut ctx = create_test_context("BTC", 50000.0);
        ctx.trades_today = 1; // Already traded today

        let decision = bot.analyze(&ctx).await.unwrap();
        assert!(decision.is_hold(), "Grandma should not trade twice in a day");
    }

    #[tokio::test]
    async fn test_grandma_position_sizing() {
        let bot = GrandmaBot::new();

        let ctx = create_test_context("BTC", 50000.0);
        let size = bot.calculate_position_size(&ctx);

        // Max position is 5% of 100k = 5k
        // Risk per trade is 2% of 100k = 2k
        // Should use the smaller (2k) / price
        let expected_max = 2000.0 / 50000.0;
        assert!(
            (size - expected_max).abs() < 0.001,
            "Position size should be ~{}, got {}",
            expected_max,
            size
        );
    }
}
