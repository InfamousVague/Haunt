//! Bollinger Bands indicator.

use crate::services::signals::{clamp_score, make_signal_output, Signal};
use crate::types::{OhlcPoint, SignalCategory, SignalOutput};

/// Bollinger Bands indicator.
///
/// Consists of:
/// - Middle band: SMA(20)
/// - Upper band: SMA + 2 * StdDev
/// - Lower band: SMA - 2 * StdDev
///
/// Signals:
/// - Price near lower band = oversold (bullish)
/// - Price near upper band = overbought (bearish)
/// - Band squeeze = low volatility, potential breakout
pub struct BollingerBands {
    period: usize,
    std_dev_multiplier: f64,
}

impl Default for BollingerBands {
    fn default() -> Self {
        Self {
            period: 20,
            std_dev_multiplier: 2.0,
        }
    }
}

impl BollingerBands {
    /// Calculate standard deviation.
    fn std_dev(values: &[f64], mean: f64) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        let variance: f64 =
            values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
        variance.sqrt()
    }
}

impl Signal for BollingerBands {
    fn id(&self) -> &str {
        "bollinger"
    }

    fn name(&self) -> &str {
        "Bollinger Bands"
    }

    fn category(&self) -> SignalCategory {
        SignalCategory::Volatility
    }

    fn min_periods(&self) -> usize {
        self.period
    }

    fn calculate(&self, candles: &[OhlcPoint]) -> Option<SignalOutput> {
        if candles.len() < self.period {
            return None;
        }

        // Get closes for the period
        let closes: Vec<f64> = candles
            .iter()
            .rev()
            .take(self.period)
            .map(|c| c.close)
            .collect();

        // Calculate middle band (SMA)
        let middle = closes.iter().sum::<f64>() / self.period as f64;

        // Calculate standard deviation
        let std_dev = Self::std_dev(&closes, middle);

        // Calculate bands
        let upper = middle + self.std_dev_multiplier * std_dev;
        let lower = middle - self.std_dev_multiplier * std_dev;
        let band_width = upper - lower;

        let current_price = candles.last()?.close;

        // Calculate %B: where price is relative to the bands
        // %B = (Price - Lower) / (Upper - Lower)
        // %B > 1: above upper band (overbought)
        // %B < 0: below lower band (oversold)
        // %B = 0.5: at middle band
        let percent_b = if band_width > 0.0 {
            (current_price - lower) / band_width
        } else {
            0.5
        };

        // Score based on %B position
        // Below lower band (oversold) = bullish
        // Above upper band (overbought) = bearish
        let score = if percent_b <= 0.0 {
            // Below lower band - strong buy
            100.0
        } else if percent_b >= 1.0 {
            // Above upper band - strong sell
            -100.0
        } else {
            // Linear scale within bands
            (0.5 - percent_b) * 200.0
        };

        Some(make_signal_output(
            self.name(),
            self.category(),
            percent_b * 100.0, // Return %B as percentage
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

    fn create_sideways_candles(count: usize) -> Vec<OhlcPoint> {
        (0..count)
            .map(|i| {
                let variation = (i % 10) as f64 - 5.0;
                let base = 100.0 + variation;
                OhlcPoint {
                    time: 1000000 + i as i64 * 60000,
                    open: base,
                    high: base + 2.0,
                    low: base - 2.0,
                    close: base + (if i % 2 == 0 { 1.0 } else { -1.0 }),
                    volume: Some(1000.0),
                }
            })
            .collect()
    }

    #[test]
    fn test_bollinger_id_and_name() {
        let bb = BollingerBands::default();
        assert_eq!(bb.id(), "bollinger");
        assert_eq!(bb.name(), "Bollinger Bands");
    }

    #[test]
    fn test_bollinger_category() {
        let bb = BollingerBands::default();
        assert_eq!(bb.category(), SignalCategory::Volatility);
    }

    #[test]
    fn test_bollinger_min_periods() {
        let bb = BollingerBands::default();
        assert_eq!(bb.min_periods(), 20);
    }

    #[test]
    fn test_bollinger_insufficient_data() {
        let bb = BollingerBands::default();
        let candles = create_uptrend_candles(15);
        let result = bb.calculate(&candles);
        assert!(result.is_none());
    }

    #[test]
    fn test_bollinger_sideways_produces_result() {
        let bb = BollingerBands::default();
        let candles = create_sideways_candles(30);
        let result = bb.calculate(&candles);
        assert!(result.is_some());
        let output = result.unwrap();
        // Bollinger %B should be a valid percentage value (0-100 typically)
        assert!(
            output.value.is_finite(),
            "Bollinger %B should be finite, got {}",
            output.value
        );
    }

    #[test]
    fn test_bollinger_uptrend_upper_band() {
        let bb = BollingerBands::default();
        let candles = create_uptrend_candles(30);
        let result = bb.calculate(&candles);
        assert!(result.is_some());
        let output = result.unwrap();
        // In uptrend, price tends toward upper band (%B > 50)
        assert!(
            output.value > 50.0,
            "Bollinger %B in uptrend should be > 50, got {}",
            output.value
        );
    }

    #[test]
    fn test_bollinger_score_range() {
        let bb = BollingerBands::default();
        let candles = create_uptrend_candles(30);
        let result = bb.calculate(&candles).unwrap();
        assert!(result.score >= -100 && result.score <= 100);
    }
}
