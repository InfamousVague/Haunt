//! Stochastic Oscillator indicator.

use crate::services::signals::{clamp_score, make_signal_output, Signal};
use crate::types::{OhlcPoint, SignalCategory, SignalOutput};

/// Stochastic Oscillator.
///
/// Compares closing price to price range over a period:
/// %K = (Current Close - Lowest Low) / (Highest High - Lowest Low) * 100
///
/// Signals:
/// - Below 20: Oversold (bullish)
/// - Above 80: Overbought (bearish)
pub struct Stochastic {
    k_period: usize,
    d_period: usize,
}

impl Default for Stochastic {
    fn default() -> Self {
        Self {
            k_period: 14,
            d_period: 3,
        }
    }
}

impl Signal for Stochastic {
    fn id(&self) -> &str {
        "stochastic"
    }

    fn name(&self) -> &str {
        "Stochastic"
    }

    fn category(&self) -> SignalCategory {
        SignalCategory::Momentum
    }

    fn min_periods(&self) -> usize {
        self.k_period + self.d_period
    }

    fn calculate(&self, candles: &[OhlcPoint]) -> Option<SignalOutput> {
        if candles.len() < self.min_periods() {
            return None;
        }

        // Calculate %K values for all periods we need
        let mut k_values = Vec::new();

        for i in (self.k_period - 1)..candles.len() {
            let window = &candles[(i + 1 - self.k_period)..=i];

            let lowest_low = window.iter().map(|c| c.low).fold(f64::INFINITY, f64::min);
            let highest_high = window
                .iter()
                .map(|c| c.high)
                .fold(f64::NEG_INFINITY, f64::max);

            let current_close = candles[i].close;

            let k = if highest_high != lowest_low {
                ((current_close - lowest_low) / (highest_high - lowest_low)) * 100.0
            } else {
                50.0
            };

            k_values.push(k);
        }

        if k_values.len() < self.d_period {
            return None;
        }

        // Current %K
        let k = *k_values.last().unwrap();

        // %D is SMA of %K (calculated for future use)
        let _d: f64 = k_values.iter().rev().take(self.d_period).sum::<f64>() / self.d_period as f64;

        // Score based on %K position
        // Below 20 = oversold = bullish
        // Above 80 = overbought = bearish
        let score = if k <= 20.0 {
            // Oversold zone
            ((20.0 - k) / 20.0 * 100.0).min(100.0)
        } else if k >= 80.0 {
            // Overbought zone
            -((k - 80.0) / 20.0 * 100.0).max(-100.0)
        } else {
            // Neutral zone
            (50.0 - k) / 30.0 * 50.0
        };

        Some(make_signal_output(
            self.name(),
            self.category(),
            k, // Return %K value
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

    #[test]
    fn test_stochastic_id_and_name() {
        let stoch = Stochastic::default();
        assert_eq!(stoch.id(), "stochastic");
        assert_eq!(stoch.name(), "Stochastic");
    }

    #[test]
    fn test_stochastic_category() {
        let stoch = Stochastic::default();
        assert_eq!(stoch.category(), SignalCategory::Momentum);
    }

    #[test]
    fn test_stochastic_min_periods() {
        let stoch = Stochastic::default();
        assert_eq!(stoch.min_periods(), 17); // k_period + d_period
    }

    #[test]
    fn test_stochastic_insufficient_data() {
        let stoch = Stochastic::default();
        let candles = create_uptrend_candles(10);
        let result = stoch.calculate(&candles);
        assert!(result.is_none());
    }

    #[test]
    fn test_stochastic_uptrend_high_k() {
        let stoch = Stochastic::default();
        let candles = create_uptrend_candles(30);
        let result = stoch.calculate(&candles);
        assert!(result.is_some());
        let output = result.unwrap();
        // In uptrend, %K should be high
        assert!(
            output.value > 50.0,
            "Stochastic %K in uptrend should be > 50, got {}",
            output.value
        );
    }

    #[test]
    fn test_stochastic_value_range() {
        let stoch = Stochastic::default();
        let candles = create_uptrend_candles(30);
        let result = stoch.calculate(&candles).unwrap();
        // Stochastic should be 0-100
        assert!(result.value >= 0.0 && result.value <= 100.0);
    }

    #[test]
    fn test_stochastic_score_range() {
        let stoch = Stochastic::default();
        let candles = create_uptrend_candles(30);
        let result = stoch.calculate(&candles).unwrap();
        assert!(result.score >= -100 && result.score <= 100);
    }
}
