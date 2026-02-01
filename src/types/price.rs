use serde::{Deserialize, Serialize};
use std::fmt;

/// Price source identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PriceSource {
    Coinbase,
    CoinGecko,
    CryptoCompare,
    CoinMarketCap,
    Binance,
}

impl PriceSource {
    /// Get the weight for this source (higher = more trusted).
    pub fn weight(&self) -> u32 {
        match self {
            PriceSource::Coinbase => 10,
            PriceSource::CoinMarketCap => 8,
            PriceSource::CoinGecko => 7,
            PriceSource::CryptoCompare => 6,
            PriceSource::Binance => 9,
        }
    }
}

impl fmt::Display for PriceSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PriceSource::Coinbase => write!(f, "coinbase"),
            PriceSource::CoinGecko => write!(f, "coingecko"),
            PriceSource::CryptoCompare => write!(f, "cryptocompare"),
            PriceSource::CoinMarketCap => write!(f, "coinmarketcap"),
            PriceSource::Binance => write!(f, "binance"),
        }
    }
}

/// A single price point from a source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcePrice {
    pub source: PriceSource,
    pub price: f64,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_24h: Option<f64>,
}

/// Aggregated price from multiple sources.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregatedPrice {
    pub id: String,
    pub symbol: String,
    pub price: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_24h: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_24h: Option<f64>,
    pub source: PriceSource,
    pub sources: Vec<PriceSource>,
    pub timestamp: i64,
}

/// Configuration for price aggregation.
#[derive(Debug, Clone)]
pub struct AggregationConfig {
    /// Minimum price change percentage to emit an update.
    pub change_threshold: f64,
    /// Minimum time between updates for the same symbol (ms).
    pub throttle_ms: u64,
    /// Time after which a source price is considered stale (ms).
    pub stale_threshold_ms: u64,
}

impl Default for AggregationConfig {
    fn default() -> Self {
        Self {
            change_threshold: 0.01, // 0.01%
            throttle_ms: 100,
            stale_threshold_ms: 120_000, // 2 minutes
        }
    }
}
