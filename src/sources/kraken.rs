use crate::services::{ChartStore, PriceCache};
use crate::types::PriceSource;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

const KRAKEN_API_URL: &str = "https://api.kraken.com/0/public";
const POLL_INTERVAL_SECS: u64 = 45;

/// Symbol mapping for Kraken (symbol -> Kraken trading pair).
pub const SYMBOL_PAIRS: &[(&str, &str)] = &[
    ("btc", "XXBTZUSD"),
    ("eth", "XETHZUSD"),
    ("sol", "SOLUSD"),
    ("xrp", "XXRPZUSD"),
    ("doge", "XDGUSD"),
    ("ada", "ADAUSD"),
    ("avax", "AVAXUSD"),
    ("dot", "DOTUSD"),
    ("link", "LINKUSD"),
    ("matic", "MATICUSD"),
    ("ltc", "XLTCZUSD"),
    ("atom", "ATOMUSD"),
    ("uni", "UNIUSD"),
    ("xlm", "XXLMZUSD"),
    ("bch", "BCHUSD"),
    ("near", "NEARUSD"),
    ("apt", "APTUSD"),
];

/// Kraken ticker response.
#[derive(Debug, Deserialize)]
struct KrakenResponse {
    error: Vec<String>,
    result: Option<HashMap<String, KrakenTicker>>,
}

#[derive(Debug, Deserialize)]
struct KrakenTicker {
    /// Last trade closed [price, lot volume]
    c: Vec<String>,
    /// Volume [today, last 24 hours]
    v: Vec<String>,
}

/// Kraken REST client.
#[derive(Clone)]
#[allow(dead_code)]
pub struct KrakenClient {
    client: Client,
    api_key: Option<String>,
    price_cache: Arc<PriceCache>,
    chart_store: Arc<ChartStore>,
}

impl KrakenClient {
    /// Create a new Kraken client.
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
        info!("Starting Kraken price polling");

        loop {
            if let Err(e) = self.fetch_prices().await {
                error!("Kraken fetch error: {}", e);
                self.price_cache
                    .report_source_error(PriceSource::Kraken, &e.to_string());
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
        }
    }

    async fn fetch_prices(&self) -> anyhow::Result<()> {
        let pairs: Vec<&str> = SYMBOL_PAIRS.iter().map(|(_, p)| *p).collect();
        let pairs_str = pairs.join(",");

        let url = format!("{}/Ticker?pair={}", KRAKEN_API_URL, pairs_str);

        // Measure request latency
        let request_start = std::time::Instant::now();
        let response = self.client.get(&url).send().await?;
        let latency_ms = request_start.elapsed().as_millis() as u64;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            warn!(
                "Kraken API returned {}: {}",
                status,
                &text[..text.len().min(200)]
            );
            self.price_cache
                .record_source_error_metrics(PriceSource::Kraken, &format!("HTTP {}", status));
            return Err(anyhow::anyhow!("Kraken API error: {}", status));
        }

        let data: KrakenResponse = response.json().await?;

        if !data.error.is_empty() {
            warn!("Kraken API errors: {:?}", data.error);
        }

        let result = match data.result {
            Some(r) => r,
            None => return Ok(()),
        };

        let timestamp = chrono::Utc::now().timestamp_millis();

        // Build pair lookup
        let pair_to_symbol: HashMap<&str, &str> =
            SYMBOL_PAIRS.iter().map(|(s, p)| (*p, *s)).collect();

        for (pair, ticker) in result {
            // Kraken sometimes returns slightly different pair names
            let symbol = pair_to_symbol
                .get(pair.as_str())
                .or_else(|| {
                    // Try without trailing zeros or with alternate format
                    SYMBOL_PAIRS
                        .iter()
                        .find(|(_, p)| pair.starts_with(*p) || p.starts_with(&pair))
                        .map(|(s, _)| s)
                })
                .copied();

            if let Some(symbol) = symbol {
                let price: f64 = ticker.c.first().and_then(|p| p.parse().ok()).unwrap_or(0.0);

                let volume_24h: f64 = ticker.v.get(1).and_then(|v| v.parse().ok()).unwrap_or(0.0);

                if price > 0.0 {
                    debug!("Kraken price update: {} = ${}", symbol, price);
                    self.price_cache.update_price_with_latency(
                        symbol,
                        PriceSource::Kraken,
                        price,
                        Some(volume_24h),
                        latency_ms,
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
        assert_eq!(btc.unwrap().1, "XXBTZUSD");
    }

    #[test]
    fn test_symbol_pairs_contains_eth() {
        let eth = SYMBOL_PAIRS.iter().find(|(s, _)| *s == "eth");
        assert!(eth.is_some());
        assert_eq!(eth.unwrap().1, "XETHZUSD");
    }

    #[test]
    fn test_symbol_pairs_count() {
        assert!(SYMBOL_PAIRS.len() >= 17);
    }

    #[test]
    fn test_symbol_pairs_lowercase_symbols() {
        for (symbol, _) in SYMBOL_PAIRS {
            assert_eq!(*symbol, symbol.to_lowercase());
        }
    }

    #[test]
    fn test_symbol_pairs_all_usd() {
        for (_, pair) in SYMBOL_PAIRS {
            assert!(pair.ends_with("USD"));
        }
    }

    // =========================================================================
    // KrakenTicker Tests
    // =========================================================================

    #[test]
    fn test_kraken_ticker_deserialization() {
        let json = r#"{
            "c": ["43500.50", "0.5"],
            "v": ["1000", "50000"]
        }"#;

        let ticker: KrakenTicker = serde_json::from_str(json).unwrap();
        assert_eq!(ticker.c.len(), 2);
        assert_eq!(ticker.c[0], "43500.50");
        assert_eq!(ticker.v.len(), 2);
        assert_eq!(ticker.v[1], "50000");
    }

    #[test]
    fn test_kraken_ticker_parse_price() {
        let json = r#"{"c": ["2500.00", "1.0"], "v": ["100", "1000"]}"#;
        let ticker: KrakenTicker = serde_json::from_str(json).unwrap();
        let price: f64 = ticker.c.first().unwrap().parse().unwrap();
        assert_eq!(price, 2500.0);
    }

    #[test]
    fn test_kraken_ticker_parse_volume() {
        let json = r#"{"c": ["100.00", "1.0"], "v": ["1000", "50000"]}"#;
        let ticker: KrakenTicker = serde_json::from_str(json).unwrap();
        let volume: f64 = ticker.v.get(1).unwrap().parse().unwrap();
        assert_eq!(volume, 50000.0);
    }

    // =========================================================================
    // KrakenResponse Tests
    // =========================================================================

    #[test]
    fn test_kraken_response_success() {
        let json = r#"{
            "error": [],
            "result": {
                "XXBTZUSD": {"c": ["43500.50", "0.5"], "v": ["1000", "50000"]}
            }
        }"#;

        let response: KrakenResponse = serde_json::from_str(json).unwrap();
        assert!(response.error.is_empty());
        assert!(response.result.is_some());
        let result = response.result.unwrap();
        assert!(result.contains_key("XXBTZUSD"));
    }

    #[test]
    fn test_kraken_response_with_error() {
        let json = r#"{
            "error": ["EGeneral:Invalid arguments"],
            "result": null
        }"#;

        let response: KrakenResponse = serde_json::from_str(json).unwrap();
        assert!(!response.error.is_empty());
        assert_eq!(response.error[0], "EGeneral:Invalid arguments");
        assert!(response.result.is_none());
    }

    #[test]
    fn test_kraken_response_empty_result() {
        let json = r#"{"error": [], "result": {}}"#;
        let response: KrakenResponse = serde_json::from_str(json).unwrap();
        assert!(response.result.is_some());
        assert!(response.result.unwrap().is_empty());
    }
}
