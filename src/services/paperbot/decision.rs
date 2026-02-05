//! Trading decision types and context

use serde::{Deserialize, Serialize};
use crate::types::{AssetClass, AggregatedOrderBook};

/// Signal strength for a trade
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SignalStrength {
    /// Very weak signal
    VeryWeak,
    /// Weak signal
    Weak,
    /// Moderate signal
    Moderate,
    /// Strong signal
    Strong,
    /// Very strong signal
    VeryStrong,
}

impl SignalStrength {
    /// Convert to a numeric value (0.0 - 1.0)
    pub fn as_f64(&self) -> f64 {
        match self {
            SignalStrength::VeryWeak => 0.2,
            SignalStrength::Weak => 0.4,
            SignalStrength::Moderate => 0.6,
            SignalStrength::Strong => 0.8,
            SignalStrength::VeryStrong => 1.0,
        }
    }

    /// Create from a numeric value (0.0 - 1.0)
    pub fn from_f64(value: f64) -> Self {
        if value < 0.3 {
            SignalStrength::VeryWeak
        } else if value < 0.5 {
            SignalStrength::Weak
        } else if value < 0.7 {
            SignalStrength::Moderate
        } else if value < 0.9 {
            SignalStrength::Strong
        } else {
            SignalStrength::VeryStrong
        }
    }
}

/// A trading signal from indicator analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSignal {
    /// Signal direction: positive = bullish, negative = bearish
    pub direction: f64,

    /// Signal strength
    pub strength: SignalStrength,

    /// Confidence in the signal (0.0 - 1.0)
    pub confidence: f64,

    /// Which indicator generated this signal
    pub source: String,

    /// Human-readable reason for the signal
    pub reason: String,
}

impl TradeSignal {
    pub fn bullish(strength: SignalStrength, confidence: f64, source: &str, reason: &str) -> Self {
        Self {
            direction: 1.0,
            strength,
            confidence,
            source: source.to_string(),
            reason: reason.to_string(),
        }
    }

    pub fn bearish(strength: SignalStrength, confidence: f64, source: &str, reason: &str) -> Self {
        Self {
            direction: -1.0,
            strength,
            confidence,
            source: source.to_string(),
            reason: reason.to_string(),
        }
    }

    pub fn neutral(source: &str, reason: &str) -> Self {
        Self {
            direction: 0.0,
            strength: SignalStrength::VeryWeak,
            confidence: 0.0,
            source: source.to_string(),
            reason: reason.to_string(),
        }
    }
}

/// Trading decision made by a bot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradeDecision {
    /// Buy the asset
    Buy {
        /// Symbol to buy
        symbol: String,
        /// Quantity to buy (in base currency units)
        quantity: f64,
        /// Confidence in the decision (0.0 - 1.0)
        confidence: f64,
        /// Signals that led to this decision
        signals: Vec<TradeSignal>,
        /// Suggested stop loss price
        stop_loss: Option<f64>,
        /// Suggested take profit price
        take_profit: Option<f64>,
    },
    /// Sell the asset
    Sell {
        /// Symbol to sell
        symbol: String,
        /// Quantity to sell (in base currency units)
        quantity: f64,
        /// Confidence in the decision (0.0 - 1.0)
        confidence: f64,
        /// Signals that led to this decision
        signals: Vec<TradeSignal>,
        /// Reason for selling
        reason: SellReason,
    },
    /// Hold current position (no action)
    Hold {
        /// Symbol being held
        symbol: String,
        /// Why we're holding
        reason: String,
    },
}

impl TradeDecision {
    pub fn symbol(&self) -> &str {
        match self {
            TradeDecision::Buy { symbol, .. } => symbol,
            TradeDecision::Sell { symbol, .. } => symbol,
            TradeDecision::Hold { symbol, .. } => symbol,
        }
    }

    pub fn is_buy(&self) -> bool {
        matches!(self, TradeDecision::Buy { .. })
    }

    pub fn is_sell(&self) -> bool {
        matches!(self, TradeDecision::Sell { .. })
    }

    pub fn is_hold(&self) -> bool {
        matches!(self, TradeDecision::Hold { .. })
    }
}

/// Reason for selling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SellReason {
    /// Taking profits
    TakeProfit,
    /// Stop loss triggered
    StopLoss,
    /// Signal indicates sell
    Signal,
    /// Trailing stop triggered
    TrailingStop,
    /// Position rebalancing
    Rebalance,
    /// Manual close (not typically used by bots)
    Manual,
}

/// Market context for making trading decisions
#[derive(Debug, Clone)]
pub struct DecisionContext {
    /// Symbol being analyzed
    pub symbol: String,

    /// Asset class
    pub asset_class: AssetClass,

    /// Current price
    pub current_price: f64,

    /// 24-hour high
    pub high_24h: Option<f64>,

    /// 24-hour low
    pub low_24h: Option<f64>,

    /// 24-hour volume
    pub volume_24h: Option<f64>,

    /// Price change percentage in last 24h
    pub price_change_24h_pct: Option<f64>,

    /// Current RSI (14-period)
    pub rsi: Option<f64>,

    /// Current MACD histogram
    pub macd_histogram: Option<f64>,

    /// MACD signal crossover: 1 = bullish, -1 = bearish, 0 = none
    pub macd_crossover: Option<i8>,

    /// Short-term SMA (e.g., 50-period)
    pub sma_short: Option<f64>,

    /// Long-term SMA (e.g., 200-period)
    pub sma_long: Option<f64>,

    /// Short-term EMA (e.g., 12-period)
    pub ema_short: Option<f64>,

    /// Long-term EMA (e.g., 26-period)
    pub ema_long: Option<f64>,

    /// Bollinger Band upper
    pub bb_upper: Option<f64>,

    /// Bollinger Band lower
    pub bb_lower: Option<f64>,

    /// Bollinger Band middle (SMA)
    pub bb_middle: Option<f64>,

    /// Average True Range (volatility)
    pub atr: Option<f64>,

    /// ADX (trend strength)
    pub adx: Option<f64>,

    /// Volume ratio vs 20-period average
    pub volume_ratio: Option<f64>,

    /// Order book data (if available)
    pub orderbook: Option<AggregatedOrderBook>,

    /// Current position quantity (if any)
    pub current_position: Option<f64>,

    /// Current position entry price (if any)
    pub position_entry_price: Option<f64>,

    /// Unrealized PnL on current position
    pub unrealized_pnl: Option<f64>,

    /// Number of trades already made today for this symbol
    pub trades_today: u32,

    /// Unix timestamp of last trade for this symbol
    pub last_trade_timestamp: Option<i64>,

    /// Available cash in portfolio
    pub available_cash: f64,

    /// Total portfolio value
    pub portfolio_value: f64,

    /// Current timestamp
    pub timestamp: i64,
}

impl DecisionContext {
    /// Check if there's a golden cross (short SMA crosses above long SMA)
    pub fn is_golden_cross(&self) -> bool {
        match (self.sma_short, self.sma_long) {
            (Some(short), Some(long)) => short > long,
            _ => false,
        }
    }

    /// Check if there's a death cross (short SMA crosses below long SMA)
    pub fn is_death_cross(&self) -> bool {
        match (self.sma_short, self.sma_long) {
            (Some(short), Some(long)) => short < long,
            _ => false,
        }
    }

    /// Check if RSI indicates oversold
    pub fn is_oversold(&self) -> bool {
        self.rsi.map(|r| r < 30.0).unwrap_or(false)
    }

    /// Check if RSI indicates overbought
    pub fn is_overbought(&self) -> bool {
        self.rsi.map(|r| r > 70.0).unwrap_or(false)
    }

    /// Check if price is above long-term SMA (bullish trend)
    pub fn is_above_sma_long(&self) -> bool {
        match self.sma_long {
            Some(sma) => self.current_price > sma,
            None => false,
        }
    }

    /// Check if price is below long-term SMA (bearish trend)
    pub fn is_below_sma_long(&self) -> bool {
        match self.sma_long {
            Some(sma) => self.current_price < sma,
            None => false,
        }
    }

    /// Check if MACD is bullish
    pub fn is_macd_bullish(&self) -> bool {
        self.macd_histogram.map(|h| h > 0.0).unwrap_or(false)
    }

    /// Check if MACD is bearish
    pub fn is_macd_bearish(&self) -> bool {
        self.macd_histogram.map(|h| h < 0.0).unwrap_or(false)
    }

    /// Check if we have an open position
    pub fn has_position(&self) -> bool {
        self.current_position.map(|p| p.abs() > 0.0001).unwrap_or(false)
    }

    /// Calculate position PnL percentage
    pub fn position_pnl_pct(&self) -> Option<f64> {
        match (self.current_position, self.position_entry_price) {
            (Some(pos), Some(entry)) if pos.abs() > 0.0001 => {
                let pnl_pct = if pos > 0.0 {
                    (self.current_price - entry) / entry
                } else {
                    (entry - self.current_price) / entry
                };
                Some(pnl_pct)
            }
            _ => None,
        }
    }

    /// Get position size as percentage of portfolio
    pub fn position_size_pct(&self) -> f64 {
        match self.current_position {
            Some(pos) if self.portfolio_value > 0.0 => {
                (pos.abs() * self.current_price) / self.portfolio_value
            }
            _ => 0.0,
        }
    }

    /// Calculate order book imbalance (-1 to 1, positive = more bids)
    pub fn orderbook_imbalance(&self) -> Option<f64> {
        self.orderbook.as_ref().map(|ob| {
            let bid_volume: f64 = ob.bids.iter().take(10).map(|l| l.total_quantity).sum();
            let ask_volume: f64 = ob.asks.iter().take(10).map(|l| l.total_quantity).sum();
            let total = bid_volume + ask_volume;
            if total > 0.0 {
                (bid_volume - ask_volume) / total
            } else {
                0.0
            }
        })
    }

    // =========================================================================
    // Momentum Fallback Methods (used when technical indicators aren't available)
    // =========================================================================

    /// Check if we have basic technical indicators (RSI, SMA)
    pub fn has_indicators(&self) -> bool {
        self.rsi.is_some() || self.sma_short.is_some() || self.macd_histogram.is_some()
    }

    /// Check if 24h price change shows bullish momentum
    pub fn is_momentum_bullish(&self) -> bool {
        self.price_change_24h_pct.map(|pct| pct > 2.0).unwrap_or(false)
    }

    /// Check if 24h price change shows bearish momentum
    pub fn is_momentum_bearish(&self) -> bool {
        self.price_change_24h_pct.map(|pct| pct < -2.0).unwrap_or(false)
    }

    /// Check if 24h price change shows strong bullish momentum (>5%)
    pub fn is_strong_momentum_bullish(&self) -> bool {
        self.price_change_24h_pct.map(|pct| pct > 5.0).unwrap_or(false)
    }

    /// Check if 24h price change shows strong bearish momentum (<-5%)
    pub fn is_strong_momentum_bearish(&self) -> bool {
        self.price_change_24h_pct.map(|pct| pct < -5.0).unwrap_or(false)
    }

    /// Check if price is near 24h low (potential buy opportunity)
    pub fn is_near_24h_low(&self) -> bool {
        match (self.high_24h, self.low_24h) {
            (Some(high), Some(low)) if high > low => {
                let range = high - low;
                let position_in_range = (self.current_price - low) / range;
                position_in_range < 0.2 // Bottom 20% of range
            }
            _ => false,
        }
    }

    /// Check if price is near 24h high (potential sell opportunity)
    pub fn is_near_24h_high(&self) -> bool {
        match (self.high_24h, self.low_24h) {
            (Some(high), Some(low)) if high > low => {
                let range = high - low;
                let position_in_range = (self.current_price - low) / range;
                position_in_range > 0.8 // Top 20% of range
            }
            _ => false,
        }
    }

    /// Get momentum score (-1.0 to 1.0) based on available data
    /// Positive = bullish, Negative = bearish
    pub fn momentum_score(&self) -> f64 {
        let mut score = 0.0;
        let mut factors = 0;

        // 24h price change
        if let Some(pct) = self.price_change_24h_pct {
            score += (pct / 10.0).clamp(-1.0, 1.0); // 10% = full score
            factors += 1;
        }

        // RSI if available
        if let Some(rsi) = self.rsi {
            // RSI 30 = +1, RSI 70 = -1, RSI 50 = 0
            score += ((50.0 - rsi) / 20.0).clamp(-1.0, 1.0);
            factors += 1;
        }

        // MACD if available
        if let Some(macd) = self.macd_histogram {
            score += macd.signum() * 0.5; // Simple direction
            factors += 1;
        }

        // Order book imbalance
        if let Some(imbalance) = self.orderbook_imbalance() {
            score += imbalance * 0.5;
            factors += 1;
        }

        if factors > 0 {
            score / factors as f64
        } else {
            0.0
        }
    }

    /// Debug string showing what data is available
    pub fn debug_data_availability(&self) -> String {
        let mut available = Vec::new();
        let mut missing = Vec::new();

        if self.rsi.is_some() { available.push("RSI"); } else { missing.push("RSI"); }
        if self.sma_short.is_some() { available.push("SMA_short"); } else { missing.push("SMA_short"); }
        if self.sma_long.is_some() { available.push("SMA_long"); } else { missing.push("SMA_long"); }
        if self.macd_histogram.is_some() { available.push("MACD"); } else { missing.push("MACD"); }
        if self.bb_upper.is_some() { available.push("BB"); } else { missing.push("BB"); }
        if self.atr.is_some() { available.push("ATR"); } else { missing.push("ATR"); }
        if self.price_change_24h_pct.is_some() { available.push("24h_pct"); } else { missing.push("24h_pct"); }
        if self.high_24h.is_some() { available.push("24h_HL"); } else { missing.push("24h_HL"); }
        if self.orderbook.is_some() { available.push("orderbook"); } else { missing.push("orderbook"); }

        format!(
            "Available: [{}] | Missing: [{}]",
            available.join(", "),
            missing.join(", ")
        )
    }
}
