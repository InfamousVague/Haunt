//! Trading signals service module.
//!
//! Provides technical indicator calculations, composite scoring,
//! and historical accuracy tracking for trading signals.

pub mod accuracy;
pub mod indicators;
pub mod predictions;
pub mod store;

pub use accuracy::AccuracyStore;
pub use predictions::PredictionStore;
pub use store::SignalStore;

use crate::types::{OhlcPoint, SignalCategory, SignalDirection, SignalOutput};

/// Trait for implementing technical indicators.
pub trait Signal: Send + Sync {
    /// Unique identifier for this indicator.
    fn id(&self) -> &str;

    /// Human-readable name.
    fn name(&self) -> &str;

    /// Category this indicator belongs to.
    fn category(&self) -> SignalCategory;

    /// Minimum number of candle periods required for calculation.
    fn min_periods(&self) -> usize;

    /// Calculate the signal from OHLC candle data.
    /// Returns None if insufficient data or calculation fails.
    fn calculate(&self, candles: &[OhlcPoint]) -> Option<SignalOutput>;
}

/// Helper to create a SignalOutput.
pub fn make_signal_output(
    name: &str,
    category: SignalCategory,
    value: f64,
    score: i8,
) -> SignalOutput {
    SignalOutput {
        name: name.to_string(),
        category,
        value,
        score,
        direction: SignalDirection::from_score(score),
        accuracy: None,
        sample_size: None,
        timestamp: chrono::Utc::now().timestamp_millis(),
    }
}

/// Clamp a value to i8 range.
pub fn clamp_score(value: f64) -> i8 {
    value.clamp(-100.0, 100.0) as i8
}
