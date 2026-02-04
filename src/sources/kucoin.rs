use crate::services::{ChartStore, PriceCache};
use crate::types::PriceSource;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

const KUCOIN_API_URL: &str = "https://api.kucoin.com/api/v1";
const POLL_INTERVAL_SECS: u64 = 30;

/// Symbol mapping for KuCoin (symbol -> KuCoin trading pair).
pub const SYMBOL_PAIRS: &[(&str, &str)] = &[
    ("btc", "BTC-USDT"),
    ("eth", "ETH-USDT"),
    ("sol", "SOL-USDT"),
    ("xrp", "XRP-USDT"),
    ("doge", "DOGE-USDT"),
    ("ada", "ADA-USDT"),
    ("avax", "AVAX-USDT"),
    ("dot", "DOT-USDT"),
    ("link", "LINK-USDT"),
    ("matic", "MATIC-USDT"),
    ("shib", "SHIB-USDT"),
    ("ltc", "LTC-USDT"),
    ("trx", "TRX-USDT"),
    ("atom", "ATOM-USDT"),
    ("uni", "UNI-USDT"),
    ("xlm", "XLM-USDT"),
    ("bch", "BCH-USDT"),
    ("near", "NEAR-USDT"),
    ("apt", "APT-USDT"),
];

/// KuCoin all tickers response.
#[derive(Debug, Deserialize)]
struct KuCoinResponse {
    code: String,
    data: Option<KuCoinData>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct KuCoinData {
    time: Option<u64>,
    ticker: Vec<KuCoinTicker>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct KuCoinTicker {
    symbol: String,
    last: Option<String>,
    vol_value: Option<String>,
    change_rate: Option<String>,
}

/// KuCoin REST client.
#[derive(Clone)]
pub struct KuCoinClient {
    client: Client,
    api_key: Option<String>,
    price_cache: Arc<PriceCache>,
    chart_store: Arc<ChartStore>,
}

impl KuCoinClient {
    /// Create a new KuCoin client.
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
        info!("Starting KuCoin price polling");

        loop {
            if let Err(e) = self.fetch_prices().await {
                error!("KuCoin fetch error: {}", e);
                self.price_cache
                    .report_source_error(PriceSource::KuCoin, &e.to_string());
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
        }
    }

    async fn fetch_prices(&self) -> anyhow::Result<()> {
        // Fetch all tickers in one request
        let url = format!("{}/market/allTickers", KUCOIN_API_URL);

        let mut request = self.client.get(&url);
        if let Some(ref key) = self.api_key {
            request = request.header("KC-API-KEY", key);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            warn!(
                "KuCoin API returned {}: {}",
                status,
                &text[..text.len().min(200)]
            );
            return Err(anyhow::anyhow!("KuCoin API error: {}", status));
        }

        let data: KuCoinResponse = response.json().await?;

        if data.code != "200000" {
            warn!("KuCoin API error code: {}", data.code);
            return Err(anyhow::anyhow!("KuCoin API error: {}", data.code));
        }

        let tickers = match data.data {
            Some(d) => d.ticker,
            None => return Ok(()),
        };

        let timestamp = chrono::Utc::now().timestamp_millis();

        // Build pair lookup
        let pair_to_symbol: HashMap<&str, &str> =
            SYMBOL_PAIRS.iter().map(|(s, p)| (*p, *s)).collect();

        for ticker in tickers {
            if let Some(symbol) = pair_to_symbol.get(ticker.symbol.as_str()) {
                let price: f64 = ticker
                    .last
                    .as_ref()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(0.0);

                let volume_24h: f64 = ticker
                    .vol_value
                    .as_ref()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.0);

                if price > 0.0 {
                    debug!("KuCoin price update: {} = ${}", symbol, price);
                    self.price_cache.update_price(
                        symbol,
                        PriceSource::KuCoin,
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
        assert_eq!(btc.unwrap().1, "BTC-USDT");
    }

    #[test]
    fn test_symbol_pairs_count() {
        assert!(SYMBOL_PAIRS.len() >= 19);
    }

    #[test]
    fn test_symbol_pairs_all_usdt() {
        for (_, pair) in SYMBOL_PAIRS {
            assert!(pair.ends_with("-USDT"));
        }
    }

    // =========================================================================
    // KuCoinTicker Tests
    // =========================================================================

    #[test]
    fn test_kucoin_ticker_deserialization() {
        let json = r#"{
            "symbol": "BTC-USDT",
            "last": "43500.50",
            "volValue": "2175000000",
            "changeRate": "0.025"
        }"#;

        let ticker: KuCoinTicker = serde_json::from_str(json).unwrap();
        assert_eq!(ticker.symbol, "BTC-USDT");
        assert_eq!(ticker.last, Some("43500.50".to_string()));
        assert_eq!(ticker.vol_value, Some("2175000000".to_string()));
        assert_eq!(ticker.change_rate, Some("0.025".to_string()));
    }

    #[test]
    fn test_kucoin_ticker_minimal() {
        let json = r#"{"symbol": "ETH-USDT"}"#;
        let ticker: KuCoinTicker = serde_json::from_str(json).unwrap();
        assert_eq!(ticker.symbol, "ETH-USDT");
        assert!(ticker.last.is_none());
        assert!(ticker.vol_value.is_none());
    }

    // =========================================================================
    // KuCoinData Tests
    // =========================================================================

    #[test]
    fn test_kucoin_data_deserialization() {
        let json = r#"{
            "time": 1700000000000,
            "ticker": [
                {"symbol": "BTC-USDT", "last": "43500.50", "volValue": "2175000000", "changeRate": "0.025"}
            ]
        }"#;

        let data: KuCoinData = serde_json::from_str(json).unwrap();
        assert_eq!(data.time, Some(1700000000000));
        assert_eq!(data.ticker.len(), 1);
    }

    // =========================================================================
    // KuCoinResponse Tests
    // =========================================================================

    #[test]
    fn test_kucoin_response_success() {
        let json = r#"{
            "code": "200000",
            "data": {
                "time": 1700000000000,
                "ticker": [
                    {"symbol": "BTC-USDT", "last": "43500.50", "volValue": "2175000000", "changeRate": "0.025"}
                ]
            }
        }"#;

        let response: KuCoinResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.code, "200000");
        assert!(response.data.is_some());
    }

    #[test]
    fn test_kucoin_response_error() {
        let json = r#"{
            "code": "400001",
            "data": null
        }"#;

        let response: KuCoinResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.code, "400001");
        assert!(response.data.is_none());
    }
}
