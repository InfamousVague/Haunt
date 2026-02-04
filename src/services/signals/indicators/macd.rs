//! MACD (Moving Average Convergence Divergence) indicator.

use crate::services::signals::{clamp_score, make_signal_output, Signal};
use crate::types::{OhlcPoint, SignalCategory, SignalOutput};

/// MACD indicator.
///
/// Shows the relationship between two EMAs:
/// - MACD Line = EMA(12) - EMA(26)
/// - Signal Line = EMA(9) of MACD Line
/// - Histogram = MACD Line - Signal Line
///
/// Buy signal: MACD crosses above signal line
/// Sell signal: MACD crosses below signal line
pub struct Macd {
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
}

impl Default for Macd {
    fn default() -> Self {
        Self {
            fast_period: 12,
            slow_period: 26,
            signal_period: 9,
        }
    }
}

impl Macd {
    /// Calculate EMA for a series of values.
    fn calculate_ema(values: &[f64], period: usize) -> Vec<f64> {
        if values.len() < period {
            return Vec::new();
        }

        let multiplier = 2.0 / (period as f64 + 1.0);
        let mut ema = Vec::with_capacity(values.len());

        // First EMA is SMA
        let sma: f64 = values.iter().take(period).sum::<f64>() / period as f64;
        ema.push(sma);

        // Calculate subsequent EMAs
        for value in values.iter().skip(period) {
            let new_ema = (value - ema.last().unwrap()) * multiplier + ema.last().unwrap();
            ema.push(new_ema);
        }

        ema
    }
}

impl Signal for Macd {
    fn id(&self) -> &str {
        "macd"
    }

    fn name(&self) -> &str {
        "MACD"
    }

    fn category(&self) -> SignalCategory {
        SignalCategory::Trend
    }

    fn min_periods(&self) -> usize {
        self.slow_period + self.signal_period
    }

    fn calculate(&self, candles: &[OhlcPoint]) -> Option<SignalOutput> {
        if candles.len() < self.min_periods() {
            return None;
        }

        let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();

        let fast_ema = Self::calculate_ema(&closes, self.fast_period);
        let slow_ema = Self::calculate_ema(&closes, self.slow_period);

        if fast_ema.is_empty() || slow_ema.is_empty() {
            return None;
        }

        // Calculate MACD line (fast EMA - slow EMA)
        // Align the EMAs (fast starts earlier)
        let offset = self.slow_period - self.fast_period;
        let macd_line: Vec<f64> = fast_ema
            .iter()
            .skip(offset)
            .zip(slow_ema.iter())
            .map(|(f, s)| f - s)
            .collect();

        if macd_line.len() < self.signal_period {
            return None;
        }

        // Calculate signal line (EMA of MACD)
        let signal_line = Self::calculate_ema(&macd_line, self.signal_period);

        if signal_line.is_empty() {
            return None;
        }

        // Get current values
        let macd = *macd_line.last().unwrap();
        let signal = *signal_line.last().unwrap();
        let histogram = macd - signal;

        // Get previous histogram for momentum
        let prev_histogram = if macd_line.len() > 1 && signal_line.len() > 1 {
            macd_line[macd_line.len() - 2]
                - signal_line.get(signal_line.len() - 2).unwrap_or(&signal)
        } else {
            histogram
        };

        // Score based on histogram and its direction
        // Positive histogram = bullish, negative = bearish
        // Increasing histogram = strengthening signal
        let histogram_direction = if histogram > prev_histogram {
            1.0
        } else {
            -1.0
        };

        // Normalize histogram relative to price (as percentage)
        let current_price = candles.last()?.close;
        let normalized_histogram = (histogram / current_price) * 10000.0; // Basis points

        let score = (normalized_histogram * histogram_direction).clamp(-100.0, 100.0);

        Some(make_signal_output(
            self.name(),
            self.category(),
            histogram,
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
    fn test_macd_id_and_name() {
        let macd = Macd::default();
        assert_eq!(macd.id(), "macd");
        assert_eq!(macd.name(), "MACD");
    }

    #[test]
    fn test_macd_category() {
        let macd = Macd::default();
        assert_eq!(macd.category(), SignalCategory::Trend);
    }

    #[test]
    fn test_macd_min_periods() {
        let macd = Macd::default();
        assert_eq!(macd.min_periods(), 35); // 26 + 9
    }

    #[test]
    fn test_macd_insufficient_data() {
        let macd = Macd::default();
        let candles = create_uptrend_candles(30);
        let result = macd.calculate(&candles);
        assert!(result.is_none());
    }

    #[test]
    fn test_macd_uptrend_produces_result() {
        let macd = Macd::default();
        let candles = create_uptrend_candles(50);
        let result = macd.calculate(&candles);
        assert!(result.is_some());
        let output = result.unwrap();
        // MACD produces a valid histogram value
        assert!(output.value.is_finite(), "MACD histogram should be finite");
    }

    #[test]
    fn test_macd_downtrend_produces_result() {
        let macd = Macd::default();
        let candles = create_downtrend_candles(50);
        let result = macd.calculate(&candles);
        assert!(result.is_some());
        let output = result.unwrap();
        // MACD produces a valid histogram value
        assert!(output.value.is_finite(), "MACD histogram should be finite");
    }

    #[test]
    fn test_macd_score_range() {
        let macd = Macd::default();
        let candles = create_uptrend_candles(50);
        let result = macd.calculate(&candles).unwrap();
        assert!(result.score >= -100 && result.score <= 100);
    }
}
