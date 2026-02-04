//! Crypto Bro Bot - Aggressive momentum trading strategy
//!
//! Aggressive momentum chaser using RSI, MACD, and ADX.
//! Trades frequently with higher risk tolerance. "WAGMI! Diamond hands!"

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

/// Crypto Bro's internal state
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CryptoBroState {
    /// Last trade timestamp per symbol
    pub last_trade: HashMap<String, i64>,
    /// Consecutive losses for risk management
    pub consecutive_losses: u32,
    /// Win/loss tracking
    pub total_trades: u32,
    pub winning_trades: u32,
}

/// Crypto Bro Bot - Aggressive momentum chaser
pub struct CryptoBroBot {
    config: BotConfig,
    state: RwLock<CryptoBroState>,
}

impl CryptoBroBot {
    /// Create a new Crypto Bro bot with default configuration
    pub fn new() -> Self {
        Self {
            config: BotConfig::crypto_bro(),
            state: RwLock::new(CryptoBroState::default()),
        }
    }

    /// Check if we can trade this symbol (respects cooldown)
    fn can_trade(&self, symbol: &str, current_timestamp: i64, trades_today: u32) -> bool {
        if trades_today >= self.config.max_trades_per_day {
            debug!("Crypto Bro: Max trades per day reached for {}", symbol);
            return false;
        }

        let state = self.state.read().unwrap();
        if let Some(last_trade) = state.last_trade.get(symbol) {
            let min_interval = self.config.decision_interval_secs as i64;
            if current_timestamp - last_trade < min_interval {
                debug!(
                    "Crypto Bro: Too soon to trade {} (cooldown {} seconds)",
                    symbol, min_interval
                );
                return false;
            }
        }

        true
    }

    /// Calculate position size based on momentum strength
    fn calculate_position_size(&self, ctx: &DecisionContext, momentum_multiplier: f64) -> f64 {
        let base_position_value = ctx.portfolio_value * self.config.max_position_size_pct;

        // Scale up with strong momentum, scale down on weak signals
        let adjusted_value = base_position_value * momentum_multiplier.min(1.5);
        let final_value = adjusted_value.min(ctx.available_cash);

        if ctx.current_price > 0.0 {
            final_value / ctx.current_price
        } else {
            0.0
        }
    }

    /// Analyze buy signals - aggressive momentum chasing
    fn analyze_buy_signals(&self, ctx: &DecisionContext) -> Vec<TradeSignal> {
        let mut signals = Vec::new();

        // 1. RSI oversold - "Buy the dip bro!"
        if let Some(rsi) = ctx.rsi {
            if rsi < 25.0 {
                signals.push(TradeSignal::bullish(
                    SignalStrength::VeryStrong,
                    0.9,
                    "RSI",
                    &format!("Extremely oversold (RSI: {:.1}) - MASSIVE dip!", rsi),
                ));
            } else if rsi < 35.0 {
                signals.push(TradeSignal::bullish(
                    SignalStrength::Strong,
                    0.8,
                    "RSI",
                    &format!("Oversold (RSI: {:.1}) - Buy the dip!", rsi),
                ));
            } else if rsi < 45.0 {
                signals.push(TradeSignal::bullish(
                    SignalStrength::Moderate,
                    0.6,
                    "RSI",
                    &format!("Room to run (RSI: {:.1})", rsi),
                ));
            }
        }

        // 2. MACD bullish momentum - "We're pumping!"
        if let Some(macd) = ctx.macd_histogram {
            if macd > 200.0 {
                signals.push(TradeSignal::bullish(
                    SignalStrength::VeryStrong,
                    0.9,
                    "MACD",
                    "MASSIVE bullish momentum - LFG!",
                ));
            } else if macd > 50.0 {
                signals.push(TradeSignal::bullish(
                    SignalStrength::Strong,
                    0.7,
                    "MACD",
                    "Strong bullish momentum - we pumping!",
                ));
            } else if macd > 0.0 {
                signals.push(TradeSignal::bullish(
                    SignalStrength::Moderate,
                    0.5,
                    "MACD",
                    "Bullish momentum building",
                ));
            }
        }

        // 3. MACD crossover - "Trend reversal bro!"
        if let Some(crossover) = ctx.macd_crossover {
            if crossover > 0 {
                signals.push(TradeSignal::bullish(
                    SignalStrength::Strong,
                    0.8,
                    "MACD Crossover",
                    "Bullish crossover - trend reversal incoming!",
                ));
            }
        }

        // 4. ADX trend strength - "Strong trend = good trade"
        if let Some(adx) = ctx.adx {
            if adx > 40.0 {
                signals.push(TradeSignal::bullish(
                    SignalStrength::Strong,
                    0.7,
                    "ADX",
                    &format!("Strong trend forming (ADX: {:.1})", adx),
                ));
            } else if adx > 25.0 {
                signals.push(TradeSignal::bullish(
                    SignalStrength::Moderate,
                    0.5,
                    "ADX",
                    &format!("Decent trend (ADX: {:.1})", adx),
                ));
            }
        }

        // 5. Price above EMA - riding the wave
        if let (Some(price), Some(ema)) = (Some(ctx.current_price), ctx.ema_short) {
            if price > ema * 1.01 {
                signals.push(TradeSignal::bullish(
                    SignalStrength::Moderate,
                    0.5,
                    "EMA",
                    "Price above EMA - riding the wave!",
                ));
            }
        }

        // 6. Bollinger Band bounce - "Bottom of the band bro!"
        if let (Some(lower), Some(middle)) = (ctx.bb_lower, ctx.bb_middle) {
            let bb_position = (ctx.current_price - lower) / (middle - lower);
            if bb_position < 0.2 {
                signals.push(TradeSignal::bullish(
                    SignalStrength::Strong,
                    0.7,
                    "Bollinger",
                    "Near lower band - bounce incoming!",
                ));
            }
        }

        signals
    }

    /// Analyze sell signals
    fn analyze_sell_signals(&self, ctx: &DecisionContext) -> (Vec<TradeSignal>, Option<SellReason>) {
        let mut signals = Vec::new();
        let mut reason = None;

        // 1. Check stop loss (paper hands time)
        if let Some(pnl_pct) = ctx.position_pnl_pct() {
            if pnl_pct <= -self.config.stop_loss_pct {
                signals.push(TradeSignal::bearish(
                    SignalStrength::VeryStrong,
                    1.0,
                    "Stop Loss",
                    &format!("Stop loss hit ({:.1}%) - even diamond hands have limits", pnl_pct * 100.0),
                ));
                reason = Some(SellReason::StopLoss);
                return (signals, reason);
            }

            // 2. Take profit - "Secure the bag!"
            if pnl_pct >= self.config.take_profit_pct {
                signals.push(TradeSignal::bearish(
                    SignalStrength::VeryStrong,
                    1.0,
                    "Take Profit",
                    &format!("Target hit ({:.1}%) - SECURING THE BAG!", pnl_pct * 100.0),
                ));
                reason = Some(SellReason::TakeProfit);
                return (signals, reason);
            }

            // 3. Trailing profit protection at 25%+
            if pnl_pct >= 0.25 {
                signals.push(TradeSignal::bearish(
                    SignalStrength::Moderate,
                    0.6,
                    "Trailing",
                    &format!("Up {:.1}% - consider taking some profit", pnl_pct * 100.0),
                ));
            }
        }

        // 4. RSI overbought - "Top signal bro"
        if let Some(rsi) = ctx.rsi {
            if rsi > 85.0 {
                signals.push(TradeSignal::bearish(
                    SignalStrength::VeryStrong,
                    0.9,
                    "RSI",
                    &format!("Extremely overbought (RSI: {:.1}) - TOP INCOMING!", rsi),
                ));
                if reason.is_none() {
                    reason = Some(SellReason::Signal);
                }
            } else if rsi > 75.0 {
                signals.push(TradeSignal::bearish(
                    SignalStrength::Strong,
                    0.7,
                    "RSI",
                    &format!("Overbought (RSI: {:.1}) - might be time to exit", rsi),
                ));
                if reason.is_none() {
                    reason = Some(SellReason::Signal);
                }
            }
        }

        // 5. MACD bearish crossover
        if let Some(crossover) = ctx.macd_crossover {
            if crossover < 0 {
                signals.push(TradeSignal::bearish(
                    SignalStrength::Strong,
                    0.7,
                    "MACD Crossover",
                    "Bearish crossover - momentum fading!",
                ));
                if reason.is_none() {
                    reason = Some(SellReason::Signal);
                }
            }
        }

        // 6. Bollinger Band top - "Overextended bro"
        if let (Some(upper), Some(middle)) = (ctx.bb_upper, ctx.bb_middle) {
            let bb_position = (ctx.current_price - middle) / (upper - middle);
            if bb_position > 0.9 {
                signals.push(TradeSignal::bearish(
                    SignalStrength::Moderate,
                    0.6,
                    "Bollinger",
                    "Near upper band - overextended!",
                ));
            }
        }

        (signals, reason)
    }

    /// Calculate overall confidence from signals
    fn calculate_confidence(&self, signals: &[TradeSignal]) -> f64 {
        if signals.is_empty() {
            return 0.0;
        }

        let total_weight: f64 = signals
            .iter()
            .map(|s| s.strength.as_f64() * s.confidence)
            .sum();

        let max_possible_weight = signals.len() as f64;
        (total_weight / max_possible_weight).min(1.0)
    }

    /// Get momentum multiplier for position sizing
    fn get_momentum_multiplier(&self, signals: &[TradeSignal]) -> f64 {
        let strong_count = signals.iter()
            .filter(|s| matches!(s.strength, SignalStrength::Strong | SignalStrength::VeryStrong))
            .count();

        match strong_count {
            0 => 0.5,
            1 => 0.75,
            2 => 1.0,
            _ => 1.25, // Multiple strong signals = bigger position
        }
    }
}

impl Default for CryptoBroBot {
    fn default() -> Self {
        Self::new()
    }
}

impl TradingBot for CryptoBroBot {
    fn name(&self) -> &str {
        &self.config.name
    }

    fn personality(&self) -> BotPersonality {
        BotPersonality::CryptoBro
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
            // Check if we have a position - analyze sell signals first
            if ctx.has_position() {
                let (signals, sell_reason) = self.analyze_sell_signals(ctx);

                let bearish_signals: Vec<_> = signals
                    .iter()
                    .filter(|s| s.direction < 0.0)
                    .cloned()
                    .collect();

                if !bearish_signals.is_empty() {
                    let confidence = self.calculate_confidence(&bearish_signals);

                    // Crypto Bro exits faster than Grandma
                    if confidence >= 0.4 || sell_reason == Some(SellReason::StopLoss) || sell_reason == Some(SellReason::TakeProfit) {
                        let quantity = ctx.current_position.unwrap_or(0.0).abs();

                        info!(
                            "Crypto Bro: SELL {} {} @ {:.2} (confidence: {:.1}%, reason: {:?})",
                            quantity,
                            ctx.symbol,
                            ctx.current_price,
                            confidence * 100.0,
                            sell_reason
                        );

                        return Ok(TradeDecision::Sell {
                            symbol: ctx.symbol.clone(),
                            quantity,
                            confidence,
                            signals: bearish_signals,
                            reason: sell_reason.unwrap_or(SellReason::Signal),
                        });
                    }
                }

                return Ok(TradeDecision::Hold {
                    symbol: ctx.symbol.clone(),
                    reason: "HODL! Diamond hands activated!".to_string(),
                });
            }

            // No position - check if we can trade
            if !self.can_trade(&ctx.symbol, ctx.timestamp, ctx.trades_today) {
                return Ok(TradeDecision::Hold {
                    symbol: ctx.symbol.clone(),
                    reason: "Waiting for the right moment to ape in...".to_string(),
                });
            }

            // Analyze buy signals
            let signals = self.analyze_buy_signals(ctx);

            let bullish_signals: Vec<_> = signals
                .iter()
                .filter(|s| s.direction > 0.0)
                .cloned()
                .collect();

            if !bullish_signals.is_empty() {
                let confidence = self.calculate_confidence(&bullish_signals);
                let momentum = self.get_momentum_multiplier(&bullish_signals);

                // Crypto Bro is more aggressive - lower confidence threshold
                // Need at least 1 strong signal or 2+ moderate signals
                let strong_count = bullish_signals.iter()
                    .filter(|s| matches!(s.strength, SignalStrength::Strong | SignalStrength::VeryStrong))
                    .count();
                let moderate_count = bullish_signals.iter()
                    .filter(|s| matches!(s.strength, SignalStrength::Moderate))
                    .count();

                if confidence >= 0.3 && (strong_count >= 1 || moderate_count >= 2) {
                    let quantity = self.calculate_position_size(ctx, momentum);

                    if quantity > 0.0 {
                        let stop_loss = Some(ctx.current_price * (1.0 - self.config.stop_loss_pct));
                        let take_profit = Some(ctx.current_price * (1.0 + self.config.take_profit_pct));

                        info!(
                            "Crypto Bro: BUY {} {} @ {:.2} (confidence: {:.1}%) - LFG!",
                            quantity,
                            ctx.symbol,
                            ctx.current_price,
                            confidence * 100.0
                        );

                        return Ok(TradeDecision::Buy {
                            symbol: ctx.symbol.clone(),
                            quantity,
                            confidence,
                            signals: bullish_signals,
                            stop_loss,
                            take_profit,
                        });
                    }
                }
            }

            Ok(TradeDecision::Hold {
                symbol: ctx.symbol.clone(),
                reason: "Waiting for the right moment to ape in...".to_string(),
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
                state.total_trades += 1;
            }
            Ok(())
        })
    }

    fn get_state(&self) -> serde_json::Value {
        let state = self.state.read().unwrap();
        serde_json::to_value(&*state).unwrap_or_default()
    }

    fn restore_state(&mut self, state: serde_json::Value) -> Result<(), AppError> {
        if let Ok(restored) = serde_json::from_value::<CryptoBroState>(state) {
            *self.state.write().unwrap() = restored;
        }
        Ok(())
    }
}
