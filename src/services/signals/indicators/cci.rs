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
    fn test_cci_id_and_name() {
        let cci = Cci::default();
        assert_eq!(cci.id(), "cci");
        assert_eq!(cci.name(), "CCI (20)");
    }

    #[test]
    fn test_cci_category() {
        let cci = Cci::default();
        assert_eq!(cci.category(), SignalCategory::Momentum);
    }

    #[test]
    fn test_cci_min_periods() {
        let cci = Cci::default();
        assert_eq!(cci.min_periods(), 20);
    }

    #[test]
    fn test_cci_insufficient_data() {
        let cci = Cci::default();
        let candles = create_uptrend_candles(15);
        let result = cci.calculate(&candles);
        assert!(result.is_none());
    }

    #[test]
    fn test_cci_uptrend_positive() {
        let cci = Cci::default();
        let candles = create_uptrend_candles(30);
        let result = cci.calculate(&candles);
        assert!(result.is_some());
        let output = result.unwrap();
        assert!(
            output.value > 0.0,
            "CCI in uptrend should be positive, got {}",
            output.value
        );
    }

    #[test]
    fn test_cci_downtrend_negative() {
        let cci = Cci::default();
        let candles = create_downtrend_candles(30);
        let result = cci.calculate(&candles);
        assert!(result.is_some());
        let output = result.unwrap();
        assert!(
            output.value < 0.0,
            "CCI in downtrend should be negative, got {}",
            output.value
        );
    }

    #[test]
    fn test_cci_score_range() {
        let cci = Cci::default();
        let candles = create_uptrend_candles(30);
        let result = cci.calculate(&candles).unwrap();
        assert!(result.score >= -100 && result.score <= 100);
    }
}
