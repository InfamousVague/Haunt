//! Commodity Channel Index (CCI) indicator.

use crate::services::signals::{clamp_score, make_signal_output, Signal};
use crate::types::{OhlcPoint, SignalCategory, SignalOutput};

/// CCI (Commodity Channel Index) indicator.
///
/// Measures the current price level relative to an average price level:
/// CCI = (TP - SMA) / (0.015 * Mean Deviation)
/// where TP = Typical Price = (High + Low + Close) / 3
///
/// Signals:
/// - Below -100: Oversold (bullish)
/// - Above +100: Overbought (bearish)
pub struct Cci {
    period: usize,
}

impl Default for Cci {
    fn default() -> Self {
        Self { period: 20 }
    }
}

impl Cci {
    /// Calculate typical price.
    fn typical_price(candle: &OhlcPoint) -> f64 {
        (candle.high + candle.low + candle.close) / 3.0
    }

    /// Calculate mean deviation.
    fn mean_deviation(values: &[f64], mean: f64) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        values.iter().map(|v| (v - mean).abs()).sum::<f64>() / values.len() as f64
    }
}

impl Signal for Cci {
    fn id(&self) -> &str {
        "cci"
    }

    fn name(&self) -> &str {
        "CCI (20)"
    }

    fn category(&self) -> SignalCategory {
        SignalCategory::Momentum
    }

    fn min_periods(&self) -> usize {
        self.period
    }

    fn calculate(&self, candles: &[OhlcPoint]) -> Option<SignalOutput> {
        if candles.len() < self.period {
            return None;
        }

        // Calculate typical prices for the period
        let typical_prices: Vec<f64> = candles
            .iter()
            .rev()
            .take(self.period)
            .map(Self::typical_price)
            .collect();

        // Calculate SMA of typical prices
        let sma = typical_prices.iter().sum::<f64>() / self.period as f64;

        // Calculate mean deviation
        let mean_dev = Self::mean_deviation(&typical_prices, sma);

        // Calculate CCI
        let current_tp = Self::typical_price(candles.last()?);
        let cci = if mean_dev != 0.0 {
            (current_tp - sma) / (0.015 * mean_dev)
        } else {
            0.0
        };

        // Score based on CCI value
        // Below -100 = oversold = bullish
        // Above +100 = overbought = bearish
        let score = if cci <= -100.0 {
            // Oversold zone
            ((-100.0 - cci) / 100.0 * 50.0 + 50.0).min(100.0)
        } else if cci >= 100.0 {
            // Overbought zone
            (-((cci - 100.0) / 100.0 * 50.0 + 50.0)).max(-100.0)
        } else {
            // Neutral zone - scale linearly
            -cci / 100.0 * 50.0
        };

        Some(make_signal_output(
            self.name(),
            self.category(),
            cci,
            clamp_score(score),
        ))
    }
}
