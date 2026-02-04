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
            200 => "sma200",
            _ => "sma",
        }
    }

    fn name(&self) -> &str {
        match self.period {
            20 => "SMA (20)",
            50 => "SMA (50)",
            200 => "SMA (200)",
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

impl Default for Sma {
    fn default() -> Self {
        Self { period: 20 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_uptrend_candles(count: usize) -> Vec<OhlcPoint> {
        (0..count)
            .map(|i| {
                let base = 100.0 + i as f64 * 1.5;
                OhlcPoint {
                    time: 1000000 + i as i64 * 60000,
                    open: base,
                    high: base + 2.0,
                    low: base - 1.0,
                    close: base + 1.0,
                    volume: Some(1000.0),
                }
            })
            .collect()
    }

    fn create_downtrend_candles(count: usize) -> Vec<OhlcPoint> {
        (0..count)
            .map(|i| {
                let base = 200.0 - i as f64 * 1.5;
                OhlcPoint {
                    time: 1000000 + i as i64 * 60000,
                    open: base,
                    high: base + 1.0,
                    low: base - 2.0,
                    close: base - 1.0,
                    volume: Some(1000.0),
                }
            })
            .collect()
    }

    #[test]
    fn test_sma_id_and_name() {
        let sma = Sma::new(20);
        assert_eq!(sma.id(), "sma20");
        assert_eq!(sma.name(), "SMA (20)");
    }

    #[test]
    fn test_sma_category() {
        let sma = Sma::default();
        assert_eq!(sma.category(), SignalCategory::Trend);
    }

    #[test]
    fn test_sma_min_periods() {
        let sma = Sma::new(20);
        assert_eq!(sma.min_periods(), 20);
    }

    #[test]
    fn test_sma_insufficient_data() {
        let sma = Sma::default();
        let candles = create_uptrend_candles(15);
        let result = sma.calculate(&candles);
        assert!(result.is_none());
    }

    #[test]
    fn test_sma_uptrend_positive_score() {
        let sma = Sma::default();
        let candles = create_uptrend_candles(50);
        let result = sma.calculate(&candles);
        assert!(result.is_some());
        let output = result.unwrap();
        assert!(
            output.score > 0,
            "SMA score in uptrend should be positive, got {}",
            output.score
        );
    }

    #[test]
    fn test_sma_downtrend_negative_score() {
        let sma = Sma::default();
        let candles = create_downtrend_candles(50);
        let result = sma.calculate(&candles);
        assert!(result.is_some());
        let output = result.unwrap();
        assert!(
            output.score < 0,
            "SMA score in downtrend should be negative, got {}",
            output.score
        );
    }

    #[test]
    fn test_sma_score_range() {
        let sma = Sma::default();
        let candles = create_uptrend_candles(50);
        let result = sma.calculate(&candles).unwrap();
        assert!(result.score >= -100 && result.score <= 100);
    }
}
