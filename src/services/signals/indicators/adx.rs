//! Average Directional Index (ADX) indicator.

use crate::services::signals::{clamp_score, make_signal_output, Signal};
use crate::types::{OhlcPoint, SignalCategory, SignalOutput};

/// ADX (Average Directional Index) indicator.
///
/// Measures trend strength (not direction):
/// - Below 20: Weak trend / ranging market
/// - 20-40: Trending
/// - Above 40: Strong trend
///
/// Combined with +DI and -DI for direction.
pub struct Adx {
    period: usize,
}

impl Default for Adx {
    fn default() -> Self {
        Self { period: 14 }
    }
}

impl Adx {
    /// Calculate True Range.
    fn true_range(current: &OhlcPoint, previous: &OhlcPoint) -> f64 {
        let hl = current.high - current.low;
        let hc = (current.high - previous.close).abs();
        let lc = (current.low - previous.close).abs();
        hl.max(hc).max(lc)
    }

    /// Calculate smoothed moving average (Wilder's smoothing).
    fn wilders_smooth(values: &[f64], period: usize) -> Vec<f64> {
        if values.len() < period {
            return Vec::new();
        }

        let mut result = Vec::with_capacity(values.len());
        let initial: f64 = values.iter().take(period).sum::<f64>() / period as f64;
        result.push(initial);

        for value in values.iter().skip(period) {
            let smoothed = (result.last().unwrap() * (period - 1) as f64 + value) / period as f64;
            result.push(smoothed);
        }

        result
    }
}

impl Signal for Adx {
    fn id(&self) -> &str {
        "adx"
    }

    fn name(&self) -> &str {
        "ADX (14)"
    }

    fn category(&self) -> SignalCategory {
        SignalCategory::Trend
    }

    fn min_periods(&self) -> usize {
        self.period * 2 + 1
    }

    fn calculate(&self, candles: &[OhlcPoint]) -> Option<SignalOutput> {
        if candles.len() < self.min_periods() {
            return None;
        }

        let mut plus_dm = Vec::new();
        let mut minus_dm = Vec::new();
        let mut tr = Vec::new();

        // Calculate DM and TR
        for i in 1..candles.len() {
            let current = &candles[i];
            let previous = &candles[i - 1];

            // +DM and -DM
            let up_move = current.high - previous.high;
            let down_move = previous.low - current.low;

            if up_move > down_move && up_move > 0.0 {
                plus_dm.push(up_move);
            } else {
                plus_dm.push(0.0);
            }

            if down_move > up_move && down_move > 0.0 {
                minus_dm.push(down_move);
            } else {
                minus_dm.push(0.0);
            }

            tr.push(Self::true_range(current, previous));
        }

        // Smooth the values
        let smoothed_plus_dm = Self::wilders_smooth(&plus_dm, self.period);
        let smoothed_minus_dm = Self::wilders_smooth(&minus_dm, self.period);
        let smoothed_tr = Self::wilders_smooth(&tr, self.period);

        if smoothed_tr.is_empty() {
            return None;
        }

        // Calculate +DI and -DI
        let mut dx_values = Vec::new();
        for i in 0..smoothed_tr.len() {
            let atr = smoothed_tr[i];
            if atr == 0.0 {
                dx_values.push(0.0);
                continue;
            }

            let plus_di = (smoothed_plus_dm[i] / atr) * 100.0;
            let minus_di = (smoothed_minus_dm[i] / atr) * 100.0;

            // Calculate DX
            let di_sum = plus_di + minus_di;
            let dx = if di_sum > 0.0 {
                ((plus_di - minus_di).abs() / di_sum) * 100.0
            } else {
                0.0
            };
            dx_values.push(dx);
        }

        // Calculate ADX (smoothed DX)
        let adx_values = Self::wilders_smooth(&dx_values, self.period);
        let adx = *adx_values.last().unwrap_or(&0.0);

        // Get current DI values for direction
        let last_atr = *smoothed_tr.last().unwrap_or(&1.0);
        let plus_di = if last_atr > 0.0 {
            (*smoothed_plus_dm.last().unwrap_or(&0.0) / last_atr) * 100.0
        } else {
            0.0
        };
        let minus_di = if last_atr > 0.0 {
            (*smoothed_minus_dm.last().unwrap_or(&0.0) / last_atr) * 100.0
        } else {
            0.0
        };

        // Score based on ADX strength and DI direction
        // ADX > 25 indicates a trend
        // +DI > -DI = bullish trend
        // -DI > +DI = bearish trend
        let trend_strength = (adx / 50.0).min(1.0); // 0 to 1
        let direction = if plus_di > minus_di { 1.0 } else { -1.0 };

        let score = if adx < 20.0 {
            // Weak trend - neutral
            0.0
        } else {
            // Trend present - score based on direction and strength
            direction * trend_strength * 100.0
        };

        Some(make_signal_output(
            self.name(),
            self.category(),
            adx,
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
    fn test_adx_id_and_name() {
        let adx = Adx::default();
        assert_eq!(adx.id(), "adx");
        assert_eq!(adx.name(), "ADX (14)");
    }

    #[test]
    fn test_adx_category() {
        let adx = Adx::default();
        assert_eq!(adx.category(), SignalCategory::Trend);
    }

    #[test]
    fn test_adx_min_periods() {
        let adx = Adx::default();
        assert_eq!(adx.min_periods(), 29); // period * 2 + 1
    }

    #[test]
    fn test_adx_insufficient_data() {
        let adx = Adx::default();
        let candles = create_uptrend_candles(20);
        let result = adx.calculate(&candles);
        assert!(result.is_none());
    }

    #[test]
    fn test_adx_positive_value() {
        let adx = Adx::default();
        let candles = create_uptrend_candles(50);
        let result = adx.calculate(&candles);
        assert!(result.is_some());
        let output = result.unwrap();
        assert!(
            output.value >= 0.0,
            "ADX should be positive, got {}",
            output.value
        );
    }

    #[test]
    fn test_adx_score_range() {
        let adx = Adx::default();
        let candles = create_uptrend_candles(50);
        let result = adx.calculate(&candles).unwrap();
        assert!(result.score >= -100 && result.score <= 100);
    }
}
