//! Scalper Bot - High-frequency aggressive scalping strategy
//!
//! Makes rapid trades targeting small price movements.
//! Very high frequency, small profits, tight stop losses.
//! "In and out, quick profits!"

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

/// Scalper's internal state
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScalperState {
    /// Last trade timestamp per symbol
    pub last_trade: HashMap<String, i64>,
    /// Entry prices for quick exit decisions
    pub entry_prices: HashMap<String, f64>,
    /// Trade count per symbol today
    pub trades_today: HashMap<String, u32>,
    /// Total trades
    pub total_trades: u32,
    /// Winning trades
    pub winning_trades: u32,
    /// Current streak (positive = wins, negative = losses)
    pub streak: i32,
}

/// Scalper Bot - High-frequency trader targeting small moves
pub struct ScalperBot {
    config: BotConfig,
    state: RwLock<ScalperState>,
}

impl ScalperBot {
    /// Create a new Scalper bot
    pub fn new() -> Self {
        Self {
            config: BotConfig {
                id: "scalper".to_string(),
                name: "Scalper".to_string(),
                personality: BotPersonality::Quant, // Use Quant personality for now
                asset_classes: vec![AssetClass::CryptoSpot],
                symbols: vec![],
                max_position_size_pct: 0.15,      // 15% per trade
                risk_per_trade_pct: 0.05,         // 5% risk per trade
                stop_loss_pct: 0.015,             // Tight 1.5% stop loss
                take_profit_pct: 0.025,           // Quick 2.5% take profit
                max_trades_per_day: 50,           // Very active - 50 trades/day
                decision_interval_secs: 15,       // Check every 15 seconds!
                enabled: true,
                initial_capital: 250_000.0,
            },
            state: RwLock::new(ScalperState::default()),
        }
    }

    /// Check if we can trade (very short cooldown)
    fn can_trade(&self, symbol: &str, current_timestamp: i64) -> bool {
        let state = self.state.read().unwrap();

        // Check daily limit
        let today_count = state.trades_today.get(symbol).copied().unwrap_or(0);
        if today_count >= self.config.max_trades_per_day {
            return false;
        }

        // Very short cooldown - 15 seconds
        if let Some(last_trade) = state.last_trade.get(symbol) {
            if current_timestamp - last_trade < self.config.decision_interval_secs as i64 {
                return false;
            }
        }

        true
    }

    /// Calculate position size - scales with streak
    fn calculate_position_size(&self, ctx: &DecisionContext) -> f64 {
        let state = self.state.read().unwrap();

        // Base position
        let base_pct = self.config.max_position_size_pct;

        // Scale down after losses, up after wins (martingale-lite)
        let streak_multiplier = if state.streak >= 3 {
            1.25  // Winning streak - slightly bigger
        } else if state.streak <= -2 {
            0.5   // Losing streak - cut size
        } else {
            1.0
        };

        let position_value = ctx.portfolio_value * base_pct * streak_multiplier;
        let final_value = position_value.min(ctx.available_cash);

        if ctx.current_price > 0.0 {
            final_value / ctx.current_price
        } else {
            0.0
        }
    }

    /// Analyze for quick scalp opportunities
    fn analyze_scalp_signals(&self, ctx: &DecisionContext) -> Vec<TradeSignal> {
        let mut signals = Vec::new();

        // 1. RSI extremes - quick mean reversion
        if let Some(rsi) = ctx.rsi {
            if rsi < 25.0 {
                signals.push(TradeSignal::bullish(
                    SignalStrength::VeryStrong,
                    0.9,
                    "Scalp:RSI",
                    &format!("RSI extreme low {:.1} - bounce play", rsi),
                ));
            } else if rsi < 35.0 {
                signals.push(TradeSignal::bullish(
                    SignalStrength::Strong,
                    0.7,
                    "Scalp:RSI",
                    &format!("RSI oversold {:.1} - scalp entry", rsi),
                ));
            } else if rsi < 45.0 && rsi > 40.0 {
                // Neutral zone momentum
                signals.push(TradeSignal::bullish(
                    SignalStrength::Moderate,
                    0.5,
                    "Scalp:RSI",
                    "RSI neutral - momentum play",
                ));
            }
        }

        // 2. MACD momentum - quick moves
        if let Some(macd) = ctx.macd_histogram {
            if macd > 50.0 {
                signals.push(TradeSignal::bullish(
                    SignalStrength::Strong,
                    0.7,
                    "Scalp:MACD",
                    "Strong MACD momentum",
                ));
            } else if macd > 10.0 {
                signals.push(TradeSignal::bullish(
                    SignalStrength::Moderate,
                    0.5,
                    "Scalp:MACD",
                    "Positive MACD momentum",
                ));
            }
        }

        // 3. Bollinger Band touch - mean reversion scalp
        if let (Some(lower), Some(middle), Some(upper)) = (ctx.bb_lower, ctx.bb_middle, ctx.bb_upper) {
            let bb_width = upper - lower;
            if bb_width > 0.0 {
                let position = (ctx.current_price - lower) / bb_width;

                if position < 0.15 {
                    signals.push(TradeSignal::bullish(
                        SignalStrength::VeryStrong,
                        0.85,
                        "Scalp:BB",
                        "Price at lower BB - bounce scalp",
                    ));
                } else if position < 0.3 {
                    signals.push(TradeSignal::bullish(
                        SignalStrength::Strong,
                        0.7,
                        "Scalp:BB",
                        "Price near lower BB",
                    ));
                }
            }
        }

        // 4. Price momentum - quick trend following
        if let Some(ema_short) = ctx.ema_short {
            let momentum_pct = (ctx.current_price - ema_short) / ema_short * 100.0;

            if momentum_pct > 0.5 && momentum_pct < 2.0 {
                signals.push(TradeSignal::bullish(
                    SignalStrength::Moderate,
                    0.6,
                    "Scalp:Momentum",
                    &format!("Positive momentum {:.2}%", momentum_pct),
                ));
            }
        }

        // 5. Volume spike - activity indicator
        if let Some(vol_ratio) = ctx.volume_ratio {
            if vol_ratio > 1.5 {
                signals.push(TradeSignal::bullish(
                    SignalStrength::Moderate,
                    0.5,
                    "Scalp:Volume",
                    &format!("Volume spike {:.1}x", vol_ratio),
                ));
            }
        }

        // 6. Price change momentum
        if let Some(change_24h) = ctx.price_change_24h_pct {
            if change_24h > 2.0 && change_24h < 8.0 {
                signals.push(TradeSignal::bullish(
                    SignalStrength::Moderate,
                    0.5,
                    "Scalp:24hMom",
                    &format!("Positive 24h momentum {:.1}%", change_24h),
                ));
            }
        }

        signals
    }

    /// Check for quick exit signals
    fn analyze_exit_signals(&self, ctx: &DecisionContext) -> (Vec<TradeSignal>, Option<SellReason>) {
        let mut signals = Vec::new();
        let mut reason = None;

        if let Some(pnl_pct) = ctx.position_pnl_pct() {
            // Quick stop loss - 1.5%
            if pnl_pct <= -self.config.stop_loss_pct {
                signals.push(TradeSignal::bearish(
                    SignalStrength::VeryStrong,
                    1.0,
                    "Scalp:StopLoss",
                    &format!("Stop hit {:.2}% - cut quick", pnl_pct * 100.0),
                ));
                reason = Some(SellReason::StopLoss);
                return (signals, reason);
            }

            // Quick take profit - 2.5%
            if pnl_pct >= self.config.take_profit_pct {
                signals.push(TradeSignal::bearish(
                    SignalStrength::VeryStrong,
                    1.0,
                    "Scalp:TakeProfit",
                    &format!("Target hit {:.2}% - banking it", pnl_pct * 100.0),
                ));
                reason = Some(SellReason::TakeProfit);
                return (signals, reason);
            }

            // Quick profit lock at 1.5%
            if pnl_pct >= 0.015 {
                signals.push(TradeSignal::bearish(
                    SignalStrength::Strong,
                    0.7,
                    "Scalp:QuickProfit",
                    &format!("Lock in {:.2}% profit", pnl_pct * 100.0),
                ));
                if reason.is_none() {
                    reason = Some(SellReason::TakeProfit);
                }
            }
        }

        // RSI overbought - exit scalp
        if let Some(rsi) = ctx.rsi {
            if rsi > 70.0 {
                signals.push(TradeSignal::bearish(
                    SignalStrength::Strong,
                    0.7,
                    "Scalp:RSI",
                    &format!("RSI overbought {:.1} - exit", rsi),
                ));
                if reason.is_none() {
                    reason = Some(SellReason::Signal);
                }
            }
        }

        // BB upper touch - exit
        if let (Some(upper), Some(middle)) = (ctx.bb_upper, ctx.bb_middle) {
            let position = (ctx.current_price - middle) / (upper - middle);
            if position > 0.85 {
                signals.push(TradeSignal::bearish(
                    SignalStrength::Moderate,
                    0.6,
                    "Scalp:BB",
                    "Near upper BB - exit scalp",
                ));
                if reason.is_none() {
                    reason = Some(SellReason::Signal);
                }
            }
        }

        (signals, reason)
    }

    fn calculate_confidence(&self, signals: &[TradeSignal]) -> f64 {
        if signals.is_empty() {
            return 0.0;
        }

        let total: f64 = signals.iter()
            .map(|s| s.strength.as_f64() * s.confidence)
            .sum();

        (total / signals.len() as f64).min(1.0)
    }
}

impl Default for ScalperBot {
    fn default() -> Self {
        Self::new()
    }
}

impl TradingBot for ScalperBot {
    fn name(&self) -> &str {
        &self.config.name
    }

    fn personality(&self) -> BotPersonality {
        self.config.personality
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
            // Check for exit if we have a position
            if ctx.has_position() {
                let (signals, exit_reason) = self.analyze_exit_signals(ctx);

                let bearish: Vec<_> = signals.iter()
                    .filter(|s| s.direction < 0.0)
                    .cloned()
                    .collect();

                if !bearish.is_empty() {
                    let confidence = self.calculate_confidence(&bearish);

                    // Scalper exits quickly
                    if confidence >= 0.3 || exit_reason == Some(SellReason::StopLoss) || exit_reason == Some(SellReason::TakeProfit) {
                        let quantity = ctx.current_position.unwrap_or(0.0).abs();

                        info!(
                            "Scalper: SELL {} {} @ {:.2} ({:?})",
                            quantity, ctx.symbol, ctx.current_price, exit_reason
                        );

                        return Ok(TradeDecision::Sell {
                            symbol: ctx.symbol.clone(),
                            quantity,
                            confidence,
                            signals: bearish,
                            reason: exit_reason.unwrap_or(SellReason::Signal),
                        });
                    }
                }

                return Ok(TradeDecision::Hold {
                    symbol: ctx.symbol.clone(),
                    reason: "Scalper: Holding for target".to_string(),
                });
            }

            // No position - look for scalp entry
            if !self.can_trade(&ctx.symbol, ctx.timestamp) {
                return Ok(TradeDecision::Hold {
                    symbol: ctx.symbol.clone(),
                    reason: "Scalper: Cooling down".to_string(),
                });
            }

            let signals = self.analyze_scalp_signals(ctx);

            let bullish: Vec<_> = signals.iter()
                .filter(|s| s.direction > 0.0)
                .cloned()
                .collect();

            if !bullish.is_empty() {
                let confidence = self.calculate_confidence(&bullish);

                // Scalper enters with lower threshold - needs just 1 strong or 2 moderate signals
                let strong = bullish.iter()
                    .filter(|s| matches!(s.strength, SignalStrength::Strong | SignalStrength::VeryStrong))
                    .count();
                let moderate = bullish.iter()
                    .filter(|s| matches!(s.strength, SignalStrength::Moderate))
                    .count();

                if confidence >= 0.25 && (strong >= 1 || moderate >= 2) {
                    let quantity = self.calculate_position_size(ctx);

                    if quantity > 0.0 {
                        let stop_loss = Some(ctx.current_price * (1.0 - self.config.stop_loss_pct));
                        let take_profit = Some(ctx.current_price * (1.0 + self.config.take_profit_pct));

                        info!(
                            "Scalper: BUY {} {} @ {:.2} (conf: {:.0}%)",
                            quantity, ctx.symbol, ctx.current_price, confidence * 100.0
                        );

                        return Ok(TradeDecision::Buy {
                            symbol: ctx.symbol.clone(),
                            quantity,
                            confidence,
                            signals: bullish,
                            stop_loss,
                            take_profit,
                        });
                    }
                }
            }

            Ok(TradeDecision::Hold {
                symbol: ctx.symbol.clone(),
                reason: "Scalper: Waiting for setup".to_string(),
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

                // Track daily trades
                *state.trades_today.entry(symbol.to_string()).or_insert(0) += 1;
                state.total_trades += 1;

                // Update streak based on sell reason
                if decision.is_sell() {
                    if let TradeDecision::Sell { reason, .. } = decision {
                        match reason {
                            SellReason::TakeProfit => {
                                state.winning_trades += 1;
                                state.streak = state.streak.max(0) + 1;
                            }
                            SellReason::StopLoss => {
                                state.streak = state.streak.min(0) - 1;
                            }
                            _ => {}
                        }
                    }
                }

                info!(
                    "Scalper executed: {} {}, streak: {}, total: {}",
                    if decision.is_buy() { "BUY" } else { "SELL" },
                    symbol,
                    state.streak,
                    state.total_trades
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
        if let Ok(restored) = serde_json::from_value::<ScalperState>(state_value) {
            info!(
                "Scalper restored: {} trades, {} wins, streak {}",
                restored.total_trades, restored.winning_trades, restored.streak
            );
            *self.state.write().unwrap() = restored;
        }
        Ok(())
    }

    fn tick(&self) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + '_>> {
        Box::pin(async {
            // Reset daily trade counts at midnight (simplified - just log status)
            let state = self.state.read().unwrap();
            if state.total_trades > 0 && state.total_trades % 10 == 0 {
                debug!(
                    "Scalper status: {} trades, {} wins ({:.1}%), streak {}",
                    state.total_trades,
                    state.winning_trades,
                    if state.total_trades > 0 { state.winning_trades as f64 / state.total_trades as f64 * 100.0 } else { 0.0 },
                    state.streak
                );
            }
            Ok(())
        })
    }
}
