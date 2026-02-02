//! Technical indicator implementations.

pub mod adx;
pub mod atr;
pub mod bollinger;
pub mod cci;
pub mod ema;
pub mod macd;
pub mod mfi;
pub mod obv;
pub mod rsi;
pub mod sma;
pub mod stochastic;
pub mod vwap;

pub use adx::Adx;
pub use atr::Atr;
pub use bollinger::BollingerBands;
pub use cci::Cci;
pub use ema::Ema;
pub use macd::Macd;
pub use mfi::Mfi;
pub use obv::Obv;
pub use rsi::Rsi;
pub use sma::Sma;
pub use stochastic::Stochastic;
pub use vwap::Vwap;

use super::Signal;

/// Get all available indicators.
pub fn all_indicators() -> Vec<Box<dyn Signal>> {
    vec![
        // Trend indicators
        Box::new(Sma::new(20)),
        Box::new(Sma::new(50)),
        Box::new(Ema::new(12)),
        Box::new(Ema::new(26)),
        Box::new(Macd::default()),
        Box::new(Adx::default()),
        // Momentum indicators
        Box::new(Rsi::default()),
        Box::new(Stochastic::default()),
        Box::new(Cci::default()),
        Box::new(Mfi::default()),
        // Volatility indicators
        Box::new(BollingerBands::default()),
        Box::new(Atr::default()),
        // Volume indicators
        Box::new(Obv::default()),
        Box::new(Vwap::default()),
    ]
}
