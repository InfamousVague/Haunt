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

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // TradeDirection Tests
    // =========================================================================

    #[test]
    fn test_trade_direction_serialization() {
        let up = TradeDirection::Up;
        let down = TradeDirection::Down;

        let up_json = serde_json::to_string(&up).unwrap();
        let down_json = serde_json::to_string(&down).unwrap();

        assert_eq!(up_json, "\"up\"");
        assert_eq!(down_json, "\"down\"");

        let parsed_up: TradeDirection = serde_json::from_str(&up_json).unwrap();
        let parsed_down: TradeDirection = serde_json::from_str(&down_json).unwrap();

        assert_eq!(parsed_up, TradeDirection::Up);
        assert_eq!(parsed_down, TradeDirection::Down);
    }

    // =========================================================================
    // PriceSource Tests
    // =========================================================================

    #[test]
    fn test_price_source_weight_positive() {
        let sources = [
            PriceSource::Coinbase,
            PriceSource::Binance,
            PriceSource::Kraken,
            PriceSource::CoinMarketCap,
            PriceSource::CoinGecko,
            PriceSource::CryptoCompare,
            PriceSource::Okx,
            PriceSource::KuCoin,
            PriceSource::Huobi,
            PriceSource::Hyperliquid,
            PriceSource::Finnhub,
            PriceSource::Alpaca,
            PriceSource::Tiingo,
            PriceSource::AlphaVantage,
        ];

        for source in sources {
            assert!(
                source.weight() > 0,
                "{:?} should have positive weight",
                source
            );
        }
    }

    #[test]
    fn test_price_source_weight_range() {
        let sources = [
            PriceSource::Coinbase,
            PriceSource::Binance,
            PriceSource::Kraken,
            PriceSource::CoinMarketCap,
            PriceSource::CoinGecko,
        ];

        for source in sources {
            let weight = source.weight();
            assert!(
                (1..=10).contains(&weight),
                "{:?} weight {} out of range",
                source,
                weight
            );
        }
    }

    #[test]
    fn test_price_source_volume_authoritative() {
        // Only aggregators should be authoritative for volume
        assert!(PriceSource::CoinMarketCap.is_volume_authoritative());
        assert!(PriceSource::CoinGecko.is_volume_authoritative());

        // Exchanges should not be
        assert!(!PriceSource::Coinbase.is_volume_authoritative());
        assert!(!PriceSource::Binance.is_volume_authoritative());
        assert!(!PriceSource::Kraken.is_volume_authoritative());
    }

    #[test]
    fn test_price_source_display() {
        assert_eq!(format!("{}", PriceSource::Coinbase), "coinbase");
        assert_eq!(format!("{}", PriceSource::Binance), "binance");
        assert_eq!(format!("{}", PriceSource::CoinMarketCap), "coinmarketcap");
        assert_eq!(format!("{}", PriceSource::CoinGecko), "coingecko");
        assert_eq!(format!("{}", PriceSource::Hyperliquid), "hyperliquid");
    }

    #[test]
    fn test_price_source_serialization() {
        let source = PriceSource::Coinbase;
        let json = serde_json::to_string(&source).unwrap();
        assert_eq!(json, "\"coinbase\"");

        let parsed: PriceSource = serde_json::from_str("\"binance\"").unwrap();
        assert_eq!(parsed, PriceSource::Binance);
    }

    // =========================================================================
    // SourcePrice Tests
    // =========================================================================

    #[test]
    fn test_source_price_creation() {
        let price = SourcePrice {
            source: PriceSource::Coinbase,
            price: 50000.0,
            timestamp: 1704067200000,
            volume_24h: Some(1000000000.0),
        };

        assert_eq!(price.source, PriceSource::Coinbase);
        assert_eq!(price.price, 50000.0);
        assert_eq!(price.volume_24h, Some(1000000000.0));
    }

    #[test]
    fn test_source_price_optional_volume() {
        let price = SourcePrice {
            source: PriceSource::Binance,
            price: 50000.0,
            timestamp: 1704067200000,
            volume_24h: None,
        };

        assert!(price.volume_24h.is_none());
    }

    // =========================================================================
    // AggregatedPrice Tests
    // =========================================================================

    #[test]
    fn test_aggregated_price_creation() {
        let price = AggregatedPrice {
            id: "btc".to_string(),
            symbol: "BTC".to_string(),
            price: 50000.0,
            previous_price: Some(49000.0),
            change_24h: Some(2.04),
            volume_24h: Some(1000000000.0),
            trade_direction: Some(TradeDirection::Up),
            source: PriceSource::Coinbase,
            sources: vec![PriceSource::Coinbase, PriceSource::Binance],
            timestamp: 1704067200000,
        };

        assert_eq!(price.symbol, "BTC");
        assert_eq!(price.price, 50000.0);
        assert_eq!(price.sources.len(), 2);
    }

    // =========================================================================
    // AggregationConfig Tests
    // =========================================================================

    #[test]
    fn test_aggregation_config_default() {
        let config = AggregationConfig::default();

        assert_eq!(config.change_threshold, 0.01);
        assert_eq!(config.throttle_ms, 100);
        assert_eq!(config.stale_threshold_ms, 120_000);
    }
}
