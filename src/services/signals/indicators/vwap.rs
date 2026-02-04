//! Volume Weighted Average Price (VWAP) indicator.

use crate::services::signals::{clamp_score, make_signal_output, Signal};
use crate::types::{OhlcPoint, SignalCategory, SignalOutput};

/// VWAP (Volume Weighted Average Price) indicator.
///
/// Average price weighted by volume:
/// VWAP = Cumulative(TP * Volume) / Cumulative(Volume)
///
/// Signals:
/// - Price above VWAP = bullish (institutional buying)
/// - Price below VWAP = bearish (institutional selling)
pub struct Vwap {
    period: usize,
}

impl Default for Vwap {
    fn default() -> Self {
        Self { period: 20 }
    }
}

impl Vwap {
    /// Calculate typical price.
    fn typical_price(candle: &OhlcPoint) -> f64 {
        (candle.high + candle.low + candle.close) / 3.0
    }
}

impl Signal for Vwap {
    fn id(&self) -> &str {
        "vwap"
    }

    fn name(&self) -> &str {
        "VWAP"
    }

    fn category(&self) -> SignalCategory {
        SignalCategory::Volume
    }

    fn min_periods(&self) -> usize {
        self.period
    }

    fn calculate(&self, candles: &[OhlcPoint]) -> Option<SignalOutput> {
        if candles.len() < self.period {
            return None;
        }

        // Calculate VWAP for the period
        let recent_candles: Vec<&OhlcPoint> = candles.iter().rev().take(self.period).collect();

        let mut cum_tp_vol = 0.0;
        let mut cum_vol = 0.0;

        for candle in &recent_candles {
            let tp = Self::typical_price(candle);
            let vol = candle.volume.unwrap_or(1.0);
            cum_tp_vol += tp * vol;
            cum_vol += vol;
        }

        let vwap = if cum_vol > 0.0 {
            cum_tp_vol / cum_vol
        } else {
            candles.last()?.close
        };

        let current_price = candles.last()?.close;

        // Calculate percentage difference from VWAP
        let pct_diff = ((current_price - vwap) / vwap) * 100.0;

        // Score: price above VWAP is bullish, below is bearish
        // Scale so that 3% deviation = full signal
        let score = (pct_diff * 33.0).clamp(-100.0, 100.0);

        Some(make_signal_output(
            self.name(),
            self.category(),
            vwap,
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
                    volume: Some(1000.0 + (i % 5) as f64 * 100.0),
                }
            })
            .collect()
    }

    #[test]
    fn test_vwap_id_and_name() {
        let vwap = Vwap::default();
        assert_eq!(vwap.id(), "vwap");
        assert_eq!(vwap.name(), "VWAP");
    }

    #[test]
    fn test_vwap_category() {
        let vwap = Vwap::default();
        assert_eq!(vwap.category(), SignalCategory::Volume);
    }

    #[test]
    fn test_vwap_min_periods() {
        let vwap = Vwap::default();
        assert_eq!(vwap.min_periods(), 20);
    }

    #[test]
    fn test_vwap_insufficient_data() {
        let vwap = Vwap::default();
        let candles = create_uptrend_candles(15);
        let result = vwap.calculate(&candles);
        assert!(result.is_none());
    }

    #[test]
    fn test_vwap_produces_result() {
        let vwap = Vwap::default();
        let candles = create_uptrend_candles(30);
        let result = vwap.calculate(&candles);
        assert!(result.is_some());
        let output = result.unwrap();
        assert!(
            output.value > 0.0,
            "VWAP should be positive, got {}",
            output.value
        );
    }

    #[test]
    fn test_vwap_score_range() {
        let vwap = Vwap::default();
        let candles = create_uptrend_candles(30);
        let result = vwap.calculate(&candles).unwrap();
        assert!(result.score >= -100 && result.score <= 100);
    }
}
