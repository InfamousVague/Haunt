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
        let variance: f64 = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
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
