//! Money Flow Index (MFI) indicator.

use crate::services::signals::{clamp_score, make_signal_output, Signal};
use crate::types::{OhlcPoint, SignalCategory, SignalOutput};

/// MFI (Money Flow Index) indicator.
///
/// Volume-weighted RSI. Measures buying and selling pressure:
/// MFI = 100 - (100 / (1 + Money Flow Ratio))
///
/// Signals:
/// - Below 20: Oversold (bullish)
/// - Above 80: Overbought (bearish)
pub struct Mfi {
    period: usize,
}

impl Default for Mfi {
    fn default() -> Self {
        Self { period: 14 }
    }
}

impl Mfi {
    /// Calculate typical price.
    fn typical_price(candle: &OhlcPoint) -> f64 {
        (candle.high + candle.low + candle.close) / 3.0
    }
}

impl Signal for Mfi {
    fn id(&self) -> &str {
        "mfi"
    }

    fn name(&self) -> &str {
        "MFI (14)"
    }

    fn category(&self) -> SignalCategory {
        SignalCategory::Momentum
    }

    fn min_periods(&self) -> usize {
        self.period + 1
    }

    fn calculate(&self, candles: &[OhlcPoint]) -> Option<SignalOutput> {
        if candles.len() < self.min_periods() {
            return None;
        }

        let mut positive_flow = 0.0;
        let mut negative_flow = 0.0;

        // Calculate money flow for each period
        for i in 1..candles.len() {
            let current_tp = Self::typical_price(&candles[i]);
            let prev_tp = Self::typical_price(&candles[i - 1]);

            // Use volume, default to 1 if not available
            let volume = candles[i].volume.unwrap_or(1.0);
            let money_flow = current_tp * volume;

            if current_tp > prev_tp {
                positive_flow += money_flow;
            } else if current_tp < prev_tp {
                negative_flow += money_flow;
            }
        }

        // Calculate MFI
        let mfi = if negative_flow == 0.0 {
            100.0
        } else if positive_flow == 0.0 {
            0.0
        } else {
            let money_flow_ratio = positive_flow / negative_flow;
            100.0 - (100.0 / (1.0 + money_flow_ratio))
        };

        // Score based on MFI value (same as RSI)
        // Below 20 = oversold = bullish
        // Above 80 = overbought = bearish
        let score = if mfi <= 20.0 {
            // Oversold zone
            ((20.0 - mfi) / 20.0 * 100.0).min(100.0)
        } else if mfi >= 80.0 {
            // Overbought zone
            -((mfi - 80.0) / 20.0 * 100.0).max(-100.0)
        } else {
            // Neutral zone
            (50.0 - mfi) / 30.0 * 50.0
        };

        Some(make_signal_output(
            self.name(),
            self.category(),
            mfi,
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
    fn test_mfi_id_and_name() {
        let mfi = Mfi::default();
        assert_eq!(mfi.id(), "mfi");
        assert_eq!(mfi.name(), "MFI (14)");
    }

    #[test]
    fn test_mfi_category() {
        let mfi = Mfi::default();
        assert_eq!(mfi.category(), SignalCategory::Momentum);
    }

    #[test]
    fn test_mfi_min_periods() {
        let mfi = Mfi::default();
        assert_eq!(mfi.min_periods(), 15);
    }

    #[test]
    fn test_mfi_insufficient_data() {
        let mfi = Mfi::default();
        let candles = create_uptrend_candles(10);
        let result = mfi.calculate(&candles);
        assert!(result.is_none());
    }

    #[test]
    fn test_mfi_value_range() {
        let mfi = Mfi::default();
        let candles = create_uptrend_candles(30);
        let result = mfi.calculate(&candles);
        assert!(result.is_some());
        let output = result.unwrap();
        assert!(
            output.value >= 0.0 && output.value <= 100.0,
            "MFI should be 0-100, got {}",
            output.value
        );
    }

    #[test]
    fn test_mfi_score_range() {
        let mfi = Mfi::default();
        let candles = create_uptrend_candles(30);
        let result = mfi.calculate(&candles).unwrap();
        assert!(result.score >= -100 && result.score <= 100);
    }
}
