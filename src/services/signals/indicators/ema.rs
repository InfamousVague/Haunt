//! Exponential Moving Average (EMA) indicator.

use crate::services::signals::{clamp_score, make_signal_output, Signal};
use crate::types::{OhlcPoint, SignalCategory, SignalOutput};

/// EMA (Exponential Moving Average) indicator.
///
/// Like SMA but gives more weight to recent prices.
/// Signal based on price position relative to EMA:
/// - Price above EMA = bullish
/// - Price below EMA = bearish
pub struct Ema {
    period: usize,
}

impl Ema {
    pub fn new(period: usize) -> Self {
        Self { period }
    }

    /// Calculate EMA value.
    fn calculate_ema(candles: &[OhlcPoint], period: usize) -> Option<f64> {
        if candles.len() < period {
            return None;
        }

        let multiplier = 2.0 / (period as f64 + 1.0);

        // First EMA is SMA
        let sma: f64 = candles.iter().take(period).map(|c| c.close).sum::<f64>() / period as f64;

        // Calculate EMA for remaining data
        let mut ema = sma;
        for candle in candles.iter().skip(period) {
            ema = (candle.close - ema) * multiplier + ema;
        }

        Some(ema)
    }
}

impl Signal for Ema {
    fn id(&self) -> &str {
        match self.period {
            12 => "ema12",
            26 => "ema26",
            _ => "ema",
        }
    }

    fn name(&self) -> &str {
        match self.period {
            12 => "EMA (12)",
            26 => "EMA (26)",
            _ => "EMA",
        }
    }

    fn category(&self) -> SignalCategory {
        SignalCategory::Trend
    }

    fn min_periods(&self) -> usize {
        self.period
    }

    fn calculate(&self, candles: &[OhlcPoint]) -> Option<SignalOutput> {
        let ema = Self::calculate_ema(candles, self.period)?;
        let current_price = candles.last()?.close;

        // Calculate percentage difference from EMA
        let pct_diff = ((current_price - ema) / ema) * 100.0;

        // Score: price above EMA is bullish, below is bearish
        // Scale so that 5% deviation = full signal
        let score = (pct_diff * 20.0).clamp(-100.0, 100.0);

        Some(make_signal_output(
            self.name(),
            self.category(),
            ema,
            clamp_score(score),
        ))
    }
}
