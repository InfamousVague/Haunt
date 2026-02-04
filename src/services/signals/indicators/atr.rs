//! Average True Range (ATR) indicator.

use crate::services::signals::{clamp_score, make_signal_output, Signal};
use crate::types::{OhlcPoint, SignalCategory, SignalOutput};

/// ATR (Average True Range) indicator.
///
/// Measures market volatility by calculating the average of true ranges:
/// TR = max(High-Low, |High-PrevClose|, |Low-PrevClose|)
///
/// Higher ATR = higher volatility
/// Lower ATR = lower volatility
///
/// For scoring, we compare current ATR to recent average.
pub struct Atr {
    period: usize,
}

impl Default for Atr {
    fn default() -> Self {
        Self { period: 14 }
    }
}

impl Atr {
    /// Calculate True Range.
    fn true_range(current: &OhlcPoint, previous: &OhlcPoint) -> f64 {
        let hl = current.high - current.low;
        let hc = (current.high - previous.close).abs();
        let lc = (current.low - previous.close).abs();
        hl.max(hc).max(lc)
    }
}

impl Signal for Atr {
    fn id(&self) -> &str {
        "atr"
    }

    fn name(&self) -> &str {
        "ATR (14)"
    }

    fn category(&self) -> SignalCategory {
        SignalCategory::Volatility
    }

    fn min_periods(&self) -> usize {
        self.period + 1
    }

    fn calculate(&self, candles: &[OhlcPoint]) -> Option<SignalOutput> {
        if candles.len() < self.min_periods() {
            return None;
        }

        // Calculate true ranges
        let mut true_ranges = Vec::new();
        for i in 1..candles.len() {
            true_ranges.push(Self::true_range(&candles[i], &candles[i - 1]));
        }

        // Calculate ATR using Wilder's smoothing
        let initial_atr: f64 =
            true_ranges.iter().take(self.period).sum::<f64>() / self.period as f64;

        let mut atr = initial_atr;
        for tr in true_ranges.iter().skip(self.period) {
            atr = (atr * (self.period - 1) as f64 + tr) / self.period as f64;
        }

        // Calculate ATR as percentage of current price
        let current_price = candles.last()?.close;
        let atr_pct = (atr / current_price) * 100.0;

        // Calculate longer-term average ATR for comparison
        let lookback = self.period * 2;
        let avg_tr: f64 = if true_ranges.len() >= lookback {
            true_ranges.iter().rev().take(lookback).sum::<f64>() / lookback as f64
        } else {
            atr
        };
        let avg_atr_pct = (avg_tr / current_price) * 100.0;

        // Score based on relative volatility
        // High volatility (ATR > average) = neutral/bearish (uncertainty)
        // Low volatility (ATR < average) = neutral/bullish (consolidation)
        // For volatility, we report the level rather than direction
        let relative_vol = if avg_atr_pct > 0.0 {
            (atr_pct / avg_atr_pct - 1.0) * 100.0
        } else {
            0.0
        };

        // Score: high volatility slightly negative, low volatility slightly positive
        // This is a nuanced signal - extreme values are notable
        let score = -relative_vol.clamp(-50.0, 50.0);

        Some(make_signal_output(
            self.name(),
            self.category(),
            atr_pct, // Return ATR as percentage of price
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
    fn test_atr_id_and_name() {
        let atr = Atr::default();
        assert_eq!(atr.id(), "atr");
        assert_eq!(atr.name(), "ATR (14)");
    }

    #[test]
    fn test_atr_category() {
        let atr = Atr::default();
        assert_eq!(atr.category(), SignalCategory::Volatility);
    }

    #[test]
    fn test_atr_min_periods() {
        let atr = Atr::default();
        assert_eq!(atr.min_periods(), 15);
    }

    #[test]
    fn test_atr_insufficient_data() {
        let atr = Atr::default();
        let candles = create_uptrend_candles(10);
        let result = atr.calculate(&candles);
        assert!(result.is_none());
    }

    #[test]
    fn test_atr_positive_value() {
        let atr = Atr::default();
        let candles = create_uptrend_candles(30);
        let result = atr.calculate(&candles);
        assert!(result.is_some());
        let output = result.unwrap();
        assert!(
            output.value > 0.0,
            "ATR should be positive, got {}",
            output.value
        );
    }

    #[test]
    fn test_atr_score_range() {
        let atr = Atr::default();
        let candles = create_uptrend_candles(30);
        let result = atr.calculate(&candles).unwrap();
        assert!(result.score >= -100 && result.score <= 100);
    }
}
