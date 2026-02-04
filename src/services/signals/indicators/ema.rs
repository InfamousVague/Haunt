//! Exponential Moving Average (EMA) indicator.

use crate::services::signals::{clamp_score, make_signal_output, Signal};
use crate::types::{OhlcPoint, SignalCategory, SignalOutput};

/// EMA (Exponential Moving Average) indicator.
///
/// Like SMA but gives more weight to recent prices.
/// Signal based on price position relative to EMA:
/// - Price above EMA = bullish
/// - Price below EMA = bearish
pub struct Ema {
    period: usize,
}

impl Ema {
    pub fn new(period: usize) -> Self {
        Self { period }
    }

    /// Calculate EMA value.
    fn calculate_ema(candles: &[OhlcPoint], period: usize) -> Option<f64> {
        if candles.len() < period {
            return None;
        }

        let multiplier = 2.0 / (period as f64 + 1.0);

        // First EMA is SMA
        let sma: f64 = candles.iter().take(period).map(|c| c.close).sum::<f64>() / period as f64;

        // Calculate EMA for remaining data
        let mut ema = sma;
        for candle in candles.iter().skip(period) {
            ema = (candle.close - ema) * multiplier + ema;
        }

        Some(ema)
    }
}

impl Signal for Ema {
    fn id(&self) -> &str {
        match self.period {
            12 => "ema12",
            26 => "ema26",
            _ => "ema",
        }
    }

    fn name(&self) -> &str {
        match self.period {
            12 => "EMA (12)",
            26 => "EMA (26)",
            _ => "EMA",
        }
    }

    fn category(&self) -> SignalCategory {
        SignalCategory::Trend
    }

    fn min_periods(&self) -> usize {
        self.period
    }

    fn calculate(&self, candles: &[OhlcPoint]) -> Option<SignalOutput> {
        let ema = Self::calculate_ema(candles, self.period)?;
        let current_price = candles.last()?.close;

        // Calculate percentage difference from EMA
        let pct_diff = ((current_price - ema) / ema) * 100.0;

        // Score: price above EMA is bullish, below is bearish
        // Scale so that 5% deviation = full signal
        let score = (pct_diff * 20.0).clamp(-100.0, 100.0);

        Some(make_signal_output(
            self.name(),
            self.category(),
            ema,
            clamp_score(score),
        ))
    }
}

impl Default for Ema {
    fn default() -> Self {
        Self { period: 12 }
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
    fn test_ema_id_and_name() {
        let ema = Ema::new(12);
        assert_eq!(ema.id(), "ema12");
        assert_eq!(ema.name(), "EMA (12)");
    }

    #[test]
    fn test_ema_category() {
        let ema = Ema::default();
        assert_eq!(ema.category(), SignalCategory::Trend);
    }

    #[test]
    fn test_ema_min_periods() {
        let ema = Ema::new(12);
        assert_eq!(ema.min_periods(), 12);
    }

    #[test]
    fn test_ema_insufficient_data() {
        let ema = Ema::default();
        let candles = create_uptrend_candles(10);
        let result = ema.calculate(&candles);
        assert!(result.is_none());
    }

    #[test]
    fn test_ema_uptrend_positive_score() {
        let ema = Ema::default();
        let candles = create_uptrend_candles(30);
        let result = ema.calculate(&candles);
        assert!(result.is_some());
        let output = result.unwrap();
        assert!(
            output.score > 0,
            "EMA score in uptrend should be positive, got {}",
            output.score
        );
    }

    #[test]
    fn test_ema_downtrend_negative_score() {
        let ema = Ema::default();
        let candles = create_downtrend_candles(30);
        let result = ema.calculate(&candles);
        assert!(result.is_some());
        let output = result.unwrap();
        assert!(
            output.score < 0,
            "EMA score in downtrend should be negative, got {}",
            output.score
        );
    }

    #[test]
    fn test_ema_score_range() {
        let ema = Ema::default();
        let candles = create_uptrend_candles(30);
        let result = ema.calculate(&candles).unwrap();
        assert!(result.score >= -100 && result.score <= 100);
    }
}
