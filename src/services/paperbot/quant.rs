//! Quant Bot - ML-powered adaptive trading strategy
//!
//! Uses Thompson Sampling for exploration/exploitation with Bayesian learning.
//! Learns from trade outcomes and adjusts indicator weights dynamically.
//! Data-driven, calculated risk with adaptive strategies.

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

/// Quant's learning state - persisted across sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantState {
    /// Beta distribution parameters for each indicator (alpha, beta)
    /// Used for Thompson Sampling
    pub indicator_params: HashMap<String, (f64, f64)>,
    /// Recent trade outcomes for batch learning
    pub trade_history: Vec<TradeOutcome>,
    /// Total trades executed
    pub total_trades: u32,
    /// Winning trades
    pub winning_trades: u32,
    /// Current learned weights (derived from params)
    pub current_weights: HashMap<String, f64>,
    /// Learning rate decay counter
    pub epoch: u32,
    /// Last trade timestamp per symbol
    pub last_trade: HashMap<String, i64>,
    /// Pending trades awaiting outcome
    pub pending_trades: Vec<PendingTrade>,
}

/// A trade waiting for outcome to learn from
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTrade {
    pub symbol: String,
    pub entry_price: f64,
    pub entry_timestamp: i64,
    pub indicator_scores: HashMap<String, f64>,
    pub action: f64, // 1.0 = buy, -1.0 = sell
}

/// Recorded trade outcome for learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeOutcome {
    pub timestamp: i64,
    pub symbol: String,
    /// Indicator scores at time of trade
    pub indicator_scores: HashMap<String, f64>,
    /// Final action taken (1.0 = buy, -1.0 = sell)
    pub action: f64,
    /// Resulting PnL percentage
    pub pnl_pct: f64,
    /// Whether trade was profitable
    pub profitable: bool,
}

impl Default for QuantState {
    fn default() -> Self {
        // Initialize indicators with uniform Beta priors (alpha=1, beta=1)
        let indicators = vec![
            "rsi", "macd", "macd_crossover", "sma_trend", "adx", "bollinger", "momentum", "ema_trend"
        ];

        let mut indicator_params = HashMap::new();
        let mut current_weights = HashMap::new();

        for ind in &indicators {
            indicator_params.insert(ind.to_string(), (1.0, 1.0)); // Uniform prior
            current_weights.insert(ind.to_string(), 0.5);         // Start neutral
        }

        Self {
            indicator_params,
            trade_history: Vec::new(),
            total_trades: 0,
            winning_trades: 0,
            current_weights,
            epoch: 0,
            last_trade: HashMap::new(),
            pending_trades: Vec::new(),
        }
    }
}

impl QuantState {
    /// Sample weights using Thompson Sampling from Beta distributions
    pub fn sample_weights(&self) -> HashMap<String, f64> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut weights = HashMap::new();

        for (indicator, (alpha, beta)) in &self.indicator_params {
            // Beta distribution mean and variance
            let mean = alpha / (alpha + beta);
            let variance = (alpha * beta) / ((alpha + beta).powi(2) * (alpha + beta + 1.0));
            let std_dev = variance.sqrt();

            // Thompson Sampling: add exploration noise proportional to uncertainty
            let noise: f64 = rng.gen_range(-1.0..1.0) * std_dev * 2.0;
            let sampled = (mean + noise).clamp(0.0, 1.0);

            weights.insert(indicator.clone(), sampled);
        }

        weights
    }

    /// Update indicator parameters based on trade outcome (Bayesian update)
    pub fn update_from_outcome(&mut self, outcome: &TradeOutcome) {
        let learning_rate = 1.0 / (1.0 + self.epoch as f64 * 0.01); // Decay learning rate

        for (indicator, score) in &outcome.indicator_scores {
            if let Some((alpha, beta)) = self.indicator_params.get_mut(indicator) {
                // Determine if indicator agreed with the action
                let indicator_direction = score.signum();
                let indicator_agreed = indicator_direction == outcome.action.signum() || indicator_direction == 0.0;

                // Calculate reward based on outcome and agreement
                let reward = if outcome.profitable && indicator_agreed {
                    1.0 * learning_rate  // Correct prediction
                } else if !outcome.profitable && indicator_agreed {
                    0.0  // Indicator was wrong
                } else if outcome.profitable && !indicator_agreed {
                    0.2 * learning_rate  // Got lucky despite disagreement
                } else {
                    0.5 * learning_rate  // Indicator correctly avoided bad trade
                };

                // Bayesian update of Beta distribution parameters
                *alpha += reward;
                *beta += 1.0 - reward;

                // Prevent parameters from growing too large (regularization)
                if *alpha + *beta > 100.0 {
                    let total = *alpha + *beta;
                    *alpha = (*alpha / total) * 50.0 + 1.0;
                    *beta = (*beta / total) * 50.0 + 1.0;
                }
            }
        }

        // Update current weights with exponential moving average
        let sampled = self.sample_weights();
        for (ind, weight) in sampled {
            if let Some(current) = self.current_weights.get_mut(&ind) {
                *current = *current * 0.9 + weight * 0.1;
            }
        }

        self.epoch += 1;
    }

    /// Get win rate
    pub fn win_rate(&self) -> f64 {
        if self.total_trades == 0 {
            0.5 // Default assumption
        } else {
            self.winning_trades as f64 / self.total_trades as f64
        }
    }

    /// Get indicator reliability scores (for transparency)
    pub fn get_indicator_reliability(&self) -> HashMap<String, f64> {
        self.indicator_params
            .iter()
            .map(|(name, (alpha, beta))| {
                let reliability = alpha / (alpha + beta);
                (name.clone(), reliability)
            })
            .collect()
    }
}

/// Quant Bot - Data-driven ML-powered trader
pub struct QuantBot {
    config: BotConfig,
    state: RwLock<QuantState>,
}

impl QuantBot {
    /// Create a new Quant bot with default configuration
    pub fn new() -> Self {
        Self {
            config: BotConfig::quant(),
            state: RwLock::new(QuantState::default()),
        }
    }

    /// Check if we can trade this symbol
    fn can_trade(&self, symbol: &str, current_timestamp: i64, trades_today: u32) -> bool {
        if trades_today >= self.config.max_trades_per_day {
            debug!("Quant: Max trades per day reached for {}", symbol);
            return false;
        }

        let state = self.state.read().unwrap();
        if let Some(last_trade) = state.last_trade.get(symbol) {
            let min_interval = self.config.decision_interval_secs as i64;
            if current_timestamp - last_trade < min_interval {
                return false;
            }
        }

        true
    }

    /// Extract feature scores from context (normalized to [-1, 1])
    fn extract_features(&self, ctx: &DecisionContext) -> HashMap<String, f64> {
        let mut features = HashMap::new();

        // RSI feature: oversold = positive, overbought = negative
        if let Some(rsi) = ctx.rsi {
            let rsi_score = if rsi < 30.0 {
                (30.0 - rsi) / 30.0        // Oversold = positive (0 to 1)
            } else if rsi > 70.0 {
                -(rsi - 70.0) / 30.0       // Overbought = negative (-1 to 0)
            } else {
                (50.0 - rsi) / 50.0 * 0.3  // Neutral zone, slight bias
            };
            features.insert("rsi".to_string(), rsi_score.clamp(-1.0, 1.0));
        }

        // MACD histogram feature
        if let Some(macd) = ctx.macd_histogram {
            let macd_score = (macd / 200.0).clamp(-1.0, 1.0);
            features.insert("macd".to_string(), macd_score);
        }

        // MACD crossover feature
        if let Some(crossover) = ctx.macd_crossover {
            features.insert("macd_crossover".to_string(), crossover as f64);
        }

        // SMA trend feature
        let sma_score = match (ctx.sma_short, ctx.sma_long) {
            (Some(short), Some(long)) if long > 0.0 => {
                ((short - long) / long * 10.0).clamp(-1.0, 1.0)
            }
            _ => 0.0,
        };
        features.insert("sma_trend".to_string(), sma_score);

        // EMA trend feature
        let ema_score = match (ctx.ema_short, ctx.ema_long) {
            (Some(short), Some(long)) if long > 0.0 => {
                ((short - long) / long * 10.0).clamp(-1.0, 1.0)
            }
            _ => 0.0,
        };
        features.insert("ema_trend".to_string(), ema_score);

        // ADX feature (trend strength, always positive for strong trends)
        if let Some(adx) = ctx.adx {
            let adx_score = if adx > 25.0 {
                ((adx - 25.0) / 50.0).min(1.0)
            } else {
                -0.3 // Weak trend penalty
            };
            features.insert("adx".to_string(), adx_score);
        }

        // Bollinger Bands feature
        if let (Some(upper), Some(lower), Some(middle)) = (ctx.bb_upper, ctx.bb_lower, ctx.bb_middle) {
            let bb_width = upper - lower;
            if bb_width > 0.0 {
                let position = (ctx.current_price - middle) / (bb_width / 2.0);
                // Near lower band = positive (buy), near upper = negative (sell)
                let bb_score = -position.clamp(-1.0, 1.0);
                features.insert("bollinger".to_string(), bb_score);
            }
        }

        // Momentum feature (price vs short SMA)
        if let Some(sma) = ctx.sma_short {
            let momentum = ((ctx.current_price - sma) / sma * 10.0).clamp(-1.0, 1.0);
            features.insert("momentum".to_string(), momentum);
        }

        features
    }

    /// Calculate composite signal using learned weights
    fn calculate_signal(&self, features: &HashMap<String, f64>) -> f64 {
        let state = self.state.read().unwrap();

        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;

        for (feature, &score) in features {
            if let Some(&weight) = state.current_weights.get(feature) {
                weighted_sum += score * weight;
                total_weight += weight;
            }
        }

        if total_weight > 0.0 {
            weighted_sum / total_weight
        } else {
            0.0
        }
    }

    /// Calculate position size using Kelly criterion
    fn calculate_position_size(&self, ctx: &DecisionContext, conviction: f64) -> f64 {
        let state = self.state.read().unwrap();

        // Simplified Kelly criterion
        let win_rate = state.win_rate().max(0.4).min(0.7); // Bounded estimate
        let avg_win = self.config.take_profit_pct;
        let avg_loss = self.config.stop_loss_pct;

        // Kelly fraction = (win_rate * avg_win - (1-win_rate) * avg_loss) / avg_win
        let kelly = (win_rate * avg_win - (1.0 - win_rate) * avg_loss) / avg_win;
        let kelly_fraction = kelly.max(0.0).min(0.25); // Cap at 25%

        // Scale by conviction
        let position_pct = kelly_fraction * conviction.abs() * (self.config.max_position_size_pct / 0.25);

        let position_value = (ctx.portfolio_value * position_pct).min(ctx.available_cash);

        if ctx.current_price > 0.0 {
            position_value / ctx.current_price
        } else {
            0.0
        }
    }

    /// Create trade signals from features for transparency
    fn create_signals_from_features(&self, features: &HashMap<String, f64>, direction: f64) -> Vec<TradeSignal> {
        let state = self.state.read().unwrap();

        features.iter()
            .filter(|(_, &score)| score.abs() > 0.2) // Only significant signals
            .filter(|(_, &score)| score.signum() == direction.signum() || direction == 0.0)
            .map(|(name, &score)| {
                let weight = state.current_weights.get(name).copied().unwrap_or(0.5);
                let strength = if score.abs() > 0.7 {
                    SignalStrength::Strong
                } else if score.abs() > 0.4 {
                    SignalStrength::Moderate
                } else {
                    SignalStrength::Weak
                };

                if direction > 0.0 {
                    TradeSignal::bullish(
                        strength,
                        weight,
                        &format!("Quant:{}", name),
                        &format!("{} signal: {:.2} (weight: {:.2})", name, score, weight),
                    )
                } else {
                    TradeSignal::bearish(
                        strength,
                        weight,
                        &format!("Quant:{}", name),
                        &format!("{} signal: {:.2} (weight: {:.2})", name, score, weight),
                    )
                }
            })
            .collect()
    }

    /// Record a pending trade for later learning
    fn record_pending_trade(&self, ctx: &DecisionContext, features: &HashMap<String, f64>, action: f64) {
        let mut state = self.state.write().unwrap();

        state.pending_trades.push(PendingTrade {
            symbol: ctx.symbol.clone(),
            entry_price: ctx.current_price,
            entry_timestamp: ctx.timestamp,
            indicator_scores: features.clone(),
            action,
        });

        // Keep only recent pending trades (max 100)
        if state.pending_trades.len() > 100 {
            state.pending_trades.remove(0);
        }
    }

    /// Process closed trades and learn from outcomes
    fn process_trade_outcomes(&self, symbol: &str, exit_price: f64, entry_price: f64) {
        let mut state = self.state.write().unwrap();

        // Find matching pending trade
        if let Some(idx) = state.pending_trades.iter().position(|t| t.symbol == symbol) {
            let pending = state.pending_trades.remove(idx);

            let pnl_pct = if pending.action > 0.0 {
                (exit_price - entry_price) / entry_price
            } else {
                (entry_price - exit_price) / entry_price
            };

            let profitable = pnl_pct > 0.0;

            let outcome = TradeOutcome {
                timestamp: chrono::Utc::now().timestamp(),
                symbol: symbol.to_string(),
                indicator_scores: pending.indicator_scores,
                action: pending.action,
                pnl_pct,
                profitable,
            };

            // Update statistics
            state.total_trades += 1;
            if profitable {
                state.winning_trades += 1;
            }

            // Learn from outcome
            let outcome_clone = outcome.clone();
            state.trade_history.push(outcome);

            // Keep history bounded
            if state.trade_history.len() > 1000 {
                state.trade_history.remove(0);
            }

            // Perform Bayesian update
            drop(state); // Release lock before recursive call
            let mut state = self.state.write().unwrap();
            state.update_from_outcome(&outcome_clone);

            info!(
                "Quant learned from {} trade: PnL {:.2}%, win_rate now {:.1}%",
                symbol,
                pnl_pct * 100.0,
                state.win_rate() * 100.0
            );
        }
    }
}

impl Default for QuantBot {
    fn default() -> Self {
        Self::new()
    }
}

impl TradingBot for QuantBot {
    fn name(&self) -> &str {
        &self.config.name
    }

    fn personality(&self) -> BotPersonality {
        BotPersonality::Quant
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
            let features = self.extract_features(ctx);
            let signal = self.calculate_signal(&features);

            debug!(
                "Quant analyzing {}: signal={:.3}, features={:?}",
                ctx.symbol, signal, features
            );

            // Check for exit if we have a position
            if ctx.has_position() {
                // Strict stop loss
                if let Some(pnl_pct) = ctx.position_pnl_pct() {
                    if pnl_pct <= -self.config.stop_loss_pct {
                        let signals = vec![TradeSignal::bearish(
                            SignalStrength::VeryStrong,
                            1.0,
                            "Quant:RiskMgmt",
                            &format!("Stop loss at {:.1}%", pnl_pct * 100.0),
                        )];

                        let quantity = ctx.current_position.unwrap_or(0.0).abs();

                        // Learn from this loss
                        if let Some(entry) = ctx.position_entry_price {
                            self.process_trade_outcomes(&ctx.symbol, ctx.current_price, entry);
                        }

                        info!("Quant: SELL {} {} @ {:.2} (stop loss)", quantity, ctx.symbol, ctx.current_price);

                        return Ok(TradeDecision::Sell {
                            symbol: ctx.symbol.clone(),
                            quantity,
                            confidence: 1.0,
                            signals,
                            reason: SellReason::StopLoss,
                        });
                    }

                    // Take profit
                    if pnl_pct >= self.config.take_profit_pct {
                        let signals = vec![TradeSignal::bearish(
                            SignalStrength::Strong,
                            0.9,
                            "Quant:RiskMgmt",
                            &format!("Take profit at {:.1}%", pnl_pct * 100.0),
                        )];

                        let quantity = ctx.current_position.unwrap_or(0.0).abs();

                        // Learn from this win
                        if let Some(entry) = ctx.position_entry_price {
                            self.process_trade_outcomes(&ctx.symbol, ctx.current_price, entry);
                        }

                        info!("Quant: SELL {} {} @ {:.2} (take profit)", quantity, ctx.symbol, ctx.current_price);

                        return Ok(TradeDecision::Sell {
                            symbol: ctx.symbol.clone(),
                            quantity,
                            confidence: 0.9,
                            signals,
                            reason: SellReason::TakeProfit,
                        });
                    }
                }

                // Signal reversal (strong bearish signal)
                if signal < -0.35 {
                    let signals = self.create_signals_from_features(&features, -1.0);
                    let quantity = ctx.current_position.unwrap_or(0.0).abs();

                    // Learn from this exit
                    if let Some(entry) = ctx.position_entry_price {
                        self.process_trade_outcomes(&ctx.symbol, ctx.current_price, entry);
                    }

                    info!(
                        "Quant: SELL {} {} @ {:.2} (bearish signal: {:.2})",
                        quantity, ctx.symbol, ctx.current_price, signal
                    );

                    return Ok(TradeDecision::Sell {
                        symbol: ctx.symbol.clone(),
                        quantity,
                        confidence: signal.abs(),
                        signals,
                        reason: SellReason::Signal,
                    });
                }

                return Ok(TradeDecision::Hold {
                    symbol: ctx.symbol.clone(),
                    reason: format!("Quant: Holding position (signal: {:.2})", signal),
                });
            }

            // Check for entry if no position
            if !self.can_trade(&ctx.symbol, ctx.timestamp, ctx.trades_today) {
                return Ok(TradeDecision::Hold {
                    symbol: ctx.symbol.clone(),
                    reason: "Quant: Waiting for decision interval".to_string(),
                });
            }

            // Need strong positive signal (>0.35) to enter
            if signal > 0.35 {
                let position_size = self.calculate_position_size(ctx, signal);
                let signals = self.create_signals_from_features(&features, 1.0);

                if position_size > 0.0 && !signals.is_empty() {
                    let stop_loss = Some(ctx.current_price * (1.0 - self.config.stop_loss_pct));
                    let take_profit = Some(ctx.current_price * (1.0 + self.config.take_profit_pct));

                    // Record this trade for learning
                    self.record_pending_trade(ctx, &features, 1.0);

                    info!(
                        "Quant: BUY {} {} @ {:.2} (signal: {:.2}, confidence: {:.1}%)",
                        position_size, ctx.symbol, ctx.current_price, signal, signal.abs() * 100.0
                    );

                    return Ok(TradeDecision::Buy {
                        symbol: ctx.symbol.clone(),
                        quantity: position_size,
                        confidence: signal.abs(),
                        signals,
                        stop_loss,
                        take_profit,
                    });
                }
            }

            Ok(TradeDecision::Hold {
                symbol: ctx.symbol.clone(),
                reason: format!("Quant: Signal insufficient ({:.2})", signal),
            })
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
                state.last_trade.insert(symbol.to_string(), chrono::Utc::now().timestamp());

                info!(
                    "Quant trade executed: {} @ symbol {}, total trades: {}, win rate: {:.1}%",
                    if decision.is_buy() { "BUY" } else { "SELL" },
                    symbol,
                    state.total_trades,
                    state.win_rate() * 100.0
                );
            }
            Ok(())
        })
    }

    fn get_state(&self) -> serde_json::Value {
        let state = self.state.read().unwrap();
        serde_json::to_value(&*state).unwrap_or_default()
    }

    fn restore_state(&mut self, state_value: serde_json::Value) -> Result<(), AppError> {
        if let Ok(restored) = serde_json::from_value::<QuantState>(state_value) {
            info!(
                "Quant restored state: {} trades, {:.1}% win rate, epoch {}",
                restored.total_trades,
                restored.win_rate() * 100.0,
                restored.epoch
            );
            *self.state.write().unwrap() = restored;
        }
        Ok(())
    }

    fn tick(&self) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + '_>> {
        Box::pin(async {
            // Periodic maintenance - could do batch learning here
            let state = self.state.read().unwrap();
            if state.trade_history.len() > 0 && state.epoch % 100 == 0 {
                debug!(
                    "Quant status: {} trades, {:.1}% win rate, {} indicators tracked",
                    state.total_trades,
                    state.win_rate() * 100.0,
                    state.indicator_params.len()
                );
            }
            Ok(())
        })
    }
}
