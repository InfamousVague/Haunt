use serde::{Deserialize, Serialize};
use std::fmt;

/// Trade direction indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TradeDirection {
    Up,
    Down,
}

/// Price source identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PriceSource {
    // Crypto sources
    Coinbase,
    CoinGecko,
    CryptoCompare,
    CoinMarketCap,
    Binance,
    Kraken,
    KuCoin,
    Okx,
    Huobi,
    Hyperliquid,
    // Stock/ETF sources
    Finnhub,
    AlphaVantage,
    Alpaca,
    Tiingo,
}

impl PriceSource {
    /// Get the weight for this source (higher = more trusted).
    pub fn weight(&self) -> u32 {
        match self {
            // Crypto sources
            PriceSource::Coinbase => 10,
            PriceSource::Binance => 9,
            PriceSource::Kraken => 9,
            PriceSource::CoinMarketCap => 8,
            PriceSource::Okx => 8,
            PriceSource::KuCoin => 7,
            PriceSource::CoinGecko => 7,
            PriceSource::Huobi => 6,
            PriceSource::CryptoCompare => 6,
            PriceSource::Hyperliquid => 8, // Decentralized perp exchange, good liquidity
            // Stock/ETF sources
            PriceSource::Finnhub => 9,
            PriceSource::Alpaca => 9,
            PriceSource::Tiingo => 8,
            PriceSource::AlphaVantage => 7,
        }
    }

    /// Check if this source is authoritative for volume data.
    /// Only aggregator sources (CoinMarketCap, CoinGecko) provide accurate
    /// market-wide 24h volume. Individual exchanges only report their own volume.
    pub fn is_volume_authoritative(&self) -> bool {
        matches!(self, PriceSource::CoinMarketCap | PriceSource::CoinGecko)
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
            PriceSource::Kraken => write!(f, "kraken"),
            PriceSource::KuCoin => write!(f, "kucoin"),
            PriceSource::Okx => write!(f, "okx"),
            PriceSource::Huobi => write!(f, "huobi"),
            PriceSource::Hyperliquid => write!(f, "hyperliquid"),
            PriceSource::Finnhub => write!(f, "finnhub"),
            PriceSource::AlphaVantage => write!(f, "alphavantage"),
            PriceSource::Alpaca => write!(f, "alpaca"),
            PriceSource::Tiingo => write!(f, "tiingo"),
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trade_direction: Option<TradeDirection>,
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
