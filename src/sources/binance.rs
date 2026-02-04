use crate::services::{ChartStore, PriceCache};
use crate::types::PriceSource;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

const BINANCE_API_URL: &str = "https://api.binance.com/api/v3";
const POLL_INTERVAL_SECS: u64 = 30;

/// Symbol mapping for Binance (symbol -> Binance trading pair).
pub const SYMBOL_PAIRS: &[(&str, &str)] = &[
    ("btc", "BTCUSDT"),
    ("eth", "ETHUSDT"),
    ("bnb", "BNBUSDT"),
    ("sol", "SOLUSDT"),
    ("xrp", "XRPUSDT"),
    ("doge", "DOGEUSDT"),
    ("ada", "ADAUSDT"),
    ("avax", "AVAXUSDT"),
    ("dot", "DOTUSDT"),
    ("link", "LINKUSDT"),
    ("matic", "MATICUSDT"),
    ("shib", "SHIBUSDT"),
    ("ltc", "LTCUSDT"),
    ("trx", "TRXUSDT"),
    ("atom", "ATOMUSDT"),
    ("uni", "UNIUSDT"),
    ("xlm", "XLMUSDT"),
    ("bch", "BCHUSDT"),
    ("near", "NEARUSDT"),
    ("apt", "APTUSDT"),
];

/// Binance 24hr ticker response.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct BinanceTicker {
    symbol: String,
    last_price: String,
    price_change_percent: String,
    volume: String,
    quote_volume: String,
}

/// Binance REST client.
#[derive(Clone)]
pub struct BinanceClient {
    client: Client,
    api_key: Option<String>,
    price_cache: Arc<PriceCache>,
    chart_store: Arc<ChartStore>,
}

impl BinanceClient {
    /// Create a new Binance client.
    pub fn new(
        api_key: Option<String>,
        price_cache: Arc<PriceCache>,
        chart_store: Arc<ChartStore>,
    ) -> Self {
        let client = Client::builder()
            .user_agent("Haunt/1.0")
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            api_key,
            price_cache,
            chart_store,
        }
    }

    /// Start polling for price updates.
    pub async fn start_polling(&self) {
        info!("Starting Binance price polling");

        loop {
            if let Err(e) = self.fetch_prices().await {
                error!("Binance fetch error: {}", e);
                self.price_cache
                    .report_source_error(PriceSource::Binance, &e.to_string());
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
        }
    }

    async fn fetch_prices(&self) -> anyhow::Result<()> {
        // Fetch all tickers in one request
        let url = format!("{}/ticker/24hr", BINANCE_API_URL);

        let mut request = self.client.get(&url);
        if let Some(ref key) = self.api_key {
            request = request.header("X-MBX-APIKEY", key);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            warn!(
                "Binance API returned {}: {}",
                status,
                &text[..text.len().min(200)]
            );
            return Err(anyhow::anyhow!("Binance API error: {}", status));
        }

        let tickers: Vec<BinanceTicker> = response.json().await?;
        let timestamp = chrono::Utc::now().timestamp_millis();

        // Build symbol lookup
        let pair_to_symbol: HashMap<&str, &str> =
            SYMBOL_PAIRS.iter().map(|(s, p)| (*p, *s)).collect();

        for ticker in tickers {
            if let Some(symbol) = pair_to_symbol.get(ticker.symbol.as_str()) {
                let price: f64 = ticker.last_price.parse().unwrap_or(0.0);
                let volume_24h: f64 = ticker.quote_volume.parse().unwrap_or(0.0);

                if price > 0.0 {
                    debug!("Binance price update: {} = ${}", symbol, price);
                    self.price_cache.update_price(
                        symbol,
                        PriceSource::Binance,
                        price,
                        Some(volume_24h),
                    );
                    self.chart_store
                        .add_price(symbol, price, Some(volume_24h), timestamp);
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // SYMBOL_PAIRS Tests
    // =========================================================================

    #[test]
    fn test_symbol_pairs_contains_btc() {
        let btc = SYMBOL_PAIRS.iter().find(|(s, _)| *s == "btc");
        assert!(btc.is_some());
        assert_eq!(btc.unwrap().1, "BTCUSDT");
    }

    #[test]
    fn test_symbol_pairs_contains_eth() {
        let eth = SYMBOL_PAIRS.iter().find(|(s, _)| *s == "eth");
        assert!(eth.is_some());
        assert_eq!(eth.unwrap().1, "ETHUSDT");
    }

    #[test]
    fn test_symbol_pairs_count() {
        assert!(SYMBOL_PAIRS.len() >= 20);
    }

    #[test]
    fn test_symbol_pairs_lowercase_symbols() {
        for (symbol, _) in SYMBOL_PAIRS {
            assert_eq!(*symbol, symbol.to_lowercase());
        }
    }

    #[test]
    fn test_symbol_pairs_uppercase_trading_pairs() {
        for (_, pair) in SYMBOL_PAIRS {
            assert_eq!(*pair, pair.to_uppercase());
        }
    }

    #[test]
    fn test_symbol_pairs_all_usdt() {
        for (_, pair) in SYMBOL_PAIRS {
            assert!(pair.ends_with("USDT"));
        }
    }

    // =========================================================================
    // BinanceTicker Tests
    // =========================================================================

    #[test]
    fn test_binance_ticker_deserialization() {
        let json = r#"{
            "symbol": "BTCUSDT",
            "lastPrice": "43500.50",
            "priceChangePercent": "2.5",
            "volume": "50000",
            "quoteVolume": "2175000000"
        }"#;

        let ticker: BinanceTicker = serde_json::from_str(json).unwrap();
        assert_eq!(ticker.symbol, "BTCUSDT");
        assert_eq!(ticker.last_price, "43500.50");
        assert_eq!(ticker.price_change_percent, "2.5");
        assert_eq!(ticker.volume, "50000");
        assert_eq!(ticker.quote_volume, "2175000000");
    }

    #[test]
    fn test_binance_ticker_parse_price() {
        let json = r#"{
            "symbol": "ETHUSDT",
            "lastPrice": "2500.00",
            "priceChangePercent": "-1.2",
            "volume": "100000",
            "quoteVolume": "250000000"
        }"#;

        let ticker: BinanceTicker = serde_json::from_str(json).unwrap();
        let price: f64 = ticker.last_price.parse().unwrap();
        assert_eq!(price, 2500.0);
    }

    #[test]
    fn test_binance_ticker_parse_volume() {
        let json = r#"{
            "symbol": "SOLUSDT",
            "lastPrice": "100.00",
            "priceChangePercent": "5.0",
            "volume": "500000",
            "quoteVolume": "50000000"
        }"#;

        let ticker: BinanceTicker = serde_json::from_str(json).unwrap();
        let volume: f64 = ticker.quote_volume.parse().unwrap();
        assert_eq!(volume, 50000000.0);
    }

    #[test]
    fn test_binance_ticker_debug() {
        let json = r#"{
            "symbol": "DOGEUSDT",
            "lastPrice": "0.08",
            "priceChangePercent": "10.0",
            "volume": "10000000000",
            "quoteVolume": "800000000"
        }"#;

        let ticker: BinanceTicker = serde_json::from_str(json).unwrap();
        let debug_str = format!("{:?}", ticker);
        assert!(debug_str.contains("BinanceTicker"));
        assert!(debug_str.contains("DOGEUSDT"));
    }
}
