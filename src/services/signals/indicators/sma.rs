//! Simple Moving Average (SMA) indicator.

use crate::services::signals::{clamp_score, make_signal_output, Signal};
use crate::types::{OhlcPoint, SignalCategory, SignalOutput};

/// SMA (Simple Moving Average) indicator.
///
/// Calculates the average price over a period.
/// Signal based on price position relative to SMA:
/// - Price above SMA = bullish
/// - Price below SMA = bearish
pub struct Sma {
    period: usize,
}

impl Sma {
    pub fn new(period: usize) -> Self {
        Self { period }
    }
}

impl Signal for Sma {
    fn id(&self) -> &str {
        match self.period {
            20 => "sma20",
            50 => "sma50",
            _ => "sma",
        }
    }

    fn name(&self) -> &str {
        match self.period {
            20 => "SMA (20)",
            50 => "SMA (50)",
            _ => "SMA",
        }
    }

    fn category(&self) -> SignalCategory {
        SignalCategory::Trend
    }

    fn min_periods(&self) -> usize {
        self.period
    }

    fn calculate(&self, candles: &[OhlcPoint]) -> Option<SignalOutput> {
        if candles.len() < self.period {
            return None;
        }

        // Calculate SMA
        let sma: f64 = candles
            .iter()
            .rev()
            .take(self.period)
            .map(|c| c.close)
            .sum::<f64>()
            / self.period as f64;

        let current_price = candles.last()?.close;

        // Calculate percentage difference from SMA
        let pct_diff = ((current_price - sma) / sma) * 100.0;

        // Score: price above SMA is bullish, below is bearish
        // Scale so that 5% deviation = full signal
        let score = (pct_diff * 20.0).clamp(-100.0, 100.0);

        Some(make_signal_output(
            self.name(),
            self.category(),
            sma,
            clamp_score(score),
        ))
    }
}
