//! On-Balance Volume (OBV) indicator.

use crate::services::signals::{clamp_score, make_signal_output, Signal};
use crate::types::{OhlcPoint, SignalCategory, SignalOutput};

/// OBV (On-Balance Volume) indicator.
///
/// Cumulative volume indicator:
/// - If close > previous close: OBV += volume
/// - If close < previous close: OBV -= volume
///
/// Signals based on OBV trend vs price trend (divergence).
pub struct Obv {
    lookback: usize,
}

impl Default for Obv {
    fn default() -> Self {
        Self { lookback: 14 }
    }
}

impl Signal for Obv {
    fn id(&self) -> &str {
        "obv"
    }

    fn name(&self) -> &str {
        "OBV"
    }

    fn category(&self) -> SignalCategory {
        SignalCategory::Volume
    }

    fn min_periods(&self) -> usize {
        self.lookback + 1
    }

    fn calculate(&self, candles: &[OhlcPoint]) -> Option<SignalOutput> {
        if candles.len() < self.min_periods() {
            return None;
        }

        // Calculate OBV
        let mut obv = 0.0;
        let mut obv_values = vec![0.0];

        for i in 1..candles.len() {
            let volume = candles[i].volume.unwrap_or(1.0);
            if candles[i].close > candles[i - 1].close {
                obv += volume;
            } else if candles[i].close < candles[i - 1].close {
                obv -= volume;
            }
            obv_values.push(obv);
        }

        // Calculate OBV trend (slope)
        let recent_obv: Vec<f64> = obv_values.iter().rev().take(self.lookback).copied().collect();
        let obv_change = if recent_obv.len() >= 2 {
            recent_obv[0] - recent_obv[recent_obv.len() - 1]
        } else {
            0.0
        };

        // Calculate price trend
        let recent_closes: Vec<f64> = candles
            .iter()
            .rev()
            .take(self.lookback)
            .map(|c| c.close)
            .collect();
        let price_change = if recent_closes.len() >= 2 {
            recent_closes[0] - recent_closes[recent_closes.len() - 1]
        } else {
            0.0
        };

        // Normalize OBV change relative to average volume
        let avg_volume: f64 = candles
            .iter()
            .rev()
            .take(self.lookback)
            .filter_map(|c| c.volume)
            .sum::<f64>()
            / self.lookback as f64;

        let normalized_obv_change = if avg_volume > 0.0 {
            obv_change / (avg_volume * self.lookback as f64)
        } else {
            0.0
        };

        // Score based on OBV direction and divergence
        // Positive OBV with positive price = bullish confirmation
        // Positive OBV with negative price = bullish divergence (strong signal)
        // Negative OBV with negative price = bearish confirmation
        // Negative OBV with positive price = bearish divergence (strong signal)

        let score = if obv_change > 0.0 && price_change > 0.0 {
            // Bullish confirmation
            (normalized_obv_change * 100.0).clamp(20.0, 80.0)
        } else if obv_change > 0.0 && price_change <= 0.0 {
            // Bullish divergence - strong buy signal
            (normalized_obv_change * 150.0).clamp(50.0, 100.0)
        } else if obv_change < 0.0 && price_change < 0.0 {
            // Bearish confirmation
            (normalized_obv_change * 100.0).clamp(-80.0, -20.0)
        } else if obv_change < 0.0 && price_change >= 0.0 {
            // Bearish divergence - strong sell signal
            (normalized_obv_change * 150.0).clamp(-100.0, -50.0)
        } else {
            0.0
        };

        Some(make_signal_output(
            self.name(),
            self.category(),
            obv,
            clamp_score(score),
        ))
    }
}
