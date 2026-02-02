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
        for i in period..values.len() {
            let new_ema = (values[i] - ema.last().unwrap()) * multiplier + ema.last().unwrap();
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
        let histogram_direction = if histogram > prev_histogram { 1.0 } else { -1.0 };

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
