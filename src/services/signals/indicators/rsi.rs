//! Relative Strength Index (RSI) indicator.

use crate::services::signals::{clamp_score, make_signal_output, Signal};
use crate::types::{OhlcPoint, SignalCategory, SignalOutput};

/// RSI (Relative Strength Index) indicator.
///
/// Measures momentum by comparing the magnitude of recent gains to recent losses.
/// Values range from 0-100:
/// - Below 30: Oversold (potential buy signal)
/// - Above 70: Overbought (potential sell signal)
pub struct Rsi {
    period: usize,
}

impl Default for Rsi {
    fn default() -> Self {
        Self { period: 14 }
    }
}

impl Rsi {
    #[allow(dead_code)]
    pub fn new(period: usize) -> Self {
        Self { period }
    }

    /// Calculate RSI value from price changes.
    fn calculate_rsi(candles: &[OhlcPoint], period: usize) -> Option<f64> {
        if candles.len() < period + 1 {
            return None;
        }

        let mut gains = Vec::new();
        let mut losses = Vec::new();

        for i in 1..candles.len() {
            let change = candles[i].close - candles[i - 1].close;
            if change > 0.0 {
                gains.push(change);
                losses.push(0.0);
            } else {
                gains.push(0.0);
                losses.push(-change);
            }
        }

        // Calculate initial averages
        let initial_avg_gain: f64 = gains.iter().take(period).sum::<f64>() / period as f64;
        let initial_avg_loss: f64 = losses.iter().take(period).sum::<f64>() / period as f64;

        // Use smoothed averages for remaining data
        let mut avg_gain = initial_avg_gain;
        let mut avg_loss = initial_avg_loss;

        for i in period..gains.len() {
            avg_gain = (avg_gain * (period - 1) as f64 + gains[i]) / period as f64;
            avg_loss = (avg_loss * (period - 1) as f64 + losses[i]) / period as f64;
        }

        if avg_loss == 0.0 {
            return Some(100.0);
        }

        let rs = avg_gain / avg_loss;
        Some(100.0 - (100.0 / (1.0 + rs)))
    }
}

impl Signal for Rsi {
    fn id(&self) -> &str {
        "rsi"
    }

    fn name(&self) -> &str {
        "RSI (14)"
    }

    fn category(&self) -> SignalCategory {
        SignalCategory::Momentum
    }

    fn min_periods(&self) -> usize {
        self.period + 1
    }

    fn calculate(&self, candles: &[OhlcPoint]) -> Option<SignalOutput> {
        let rsi = Self::calculate_rsi(candles, self.period)?;

        // Convert RSI to score:
        // RSI 30 or below = +100 (strong buy - oversold)
        // RSI 50 = 0 (neutral)
        // RSI 70 or above = -100 (strong sell - overbought)
        let score = if rsi <= 30.0 {
            // Oversold zone - bullish
            ((30.0 - rsi) / 30.0 * 100.0).min(100.0)
        } else if rsi >= 70.0 {
            // Overbought zone - bearish
            -((rsi - 70.0) / 30.0 * 100.0).max(-100.0)
        } else {
            // Neutral zone - linear interpolation
            (50.0 - rsi) / 20.0 * 50.0
        };

        Some(make_signal_output(
            self.name(),
            self.category(),
            rsi,
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
    fn test_rsi_id_and_name() {
        let rsi = Rsi::default();
        assert_eq!(rsi.id(), "rsi");
        assert_eq!(rsi.name(), "RSI (14)");
    }

    #[test]
    fn test_rsi_category() {
        let rsi = Rsi::default();
        assert_eq!(rsi.category(), SignalCategory::Momentum);
    }

    #[test]
    fn test_rsi_min_periods() {
        let rsi = Rsi::default();
        assert_eq!(rsi.min_periods(), 15);
    }

    #[test]
    fn test_rsi_insufficient_data() {
        let rsi = Rsi::default();
        let candles = create_uptrend_candles(10);
        let result = rsi.calculate(&candles);
        assert!(result.is_none());
    }

    #[test]
    fn test_rsi_uptrend_high_value() {
        let rsi = Rsi::default();
        let candles = create_uptrend_candles(50);
        let result = rsi.calculate(&candles);
        assert!(result.is_some());
        let output = result.unwrap();
        assert!(
            output.value > 50.0,
            "RSI in uptrend should be > 50, got {}",
            output.value
        );
    }

    #[test]
    fn test_rsi_downtrend_low_value() {
        let rsi = Rsi::default();
        let candles = create_downtrend_candles(50);
        let result = rsi.calculate(&candles);
        assert!(result.is_some());
        let output = result.unwrap();
        assert!(
            output.value < 50.0,
            "RSI in downtrend should be < 50, got {}",
            output.value
        );
    }

    #[test]
    fn test_rsi_score_range() {
        let rsi = Rsi::default();
        let candles = create_uptrend_candles(50);
        let result = rsi.calculate(&candles).unwrap();
        assert!(result.score >= -100 && result.score <= 100);
    }

    #[test]
    fn test_rsi_value_range() {
        let rsi = Rsi::default();
        let candles = create_uptrend_candles(50);
        let result = rsi.calculate(&candles).unwrap();
        assert!(result.value >= 0.0 && result.value <= 100.0);
    }

    #[test]
    fn test_rsi_custom_period() {
        let rsi = Rsi::new(7);
        assert_eq!(rsi.min_periods(), 8);
        let candles = create_uptrend_candles(20);
        let result = rsi.calculate(&candles);
        assert!(result.is_some());
    }
}
