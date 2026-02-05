use crate::services::{ChartStore, PriceCache};
use crate::types::PriceSource;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

const OKX_API_URL: &str = "https://www.okx.com/api/v5";
const POLL_INTERVAL_SECS: u64 = 30;

/// Symbol mapping for OKX (symbol -> OKX instrument ID).
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

/// OKX tickers response.
#[derive(Debug, Deserialize)]
struct OkxResponse {
    code: String,
    msg: String,
    data: Vec<OkxTicker>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OkxTicker {
    inst_id: String,
    last: String,
    vol_ccy24h: String,
}

/// OKX REST client.
#[derive(Clone)]
pub struct OkxClient {
    client: Client,
    api_key: Option<String>,
    price_cache: Arc<PriceCache>,
    chart_store: Arc<ChartStore>,
}

impl OkxClient {
    /// Create a new OKX client.
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
        info!("Starting OKX price polling");

        loop {
            if let Err(e) = self.fetch_prices().await {
                error!("OKX fetch error: {}", e);
                self.price_cache
                    .report_source_error(PriceSource::Okx, &e.to_string());
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
        }
    }

    async fn fetch_prices(&self) -> anyhow::Result<()> {
        // OKX requires fetching tickers by instrument type
        let url = format!("{}/market/tickers?instType=SPOT", OKX_API_URL);

        let mut request = self.client.get(&url);
        if let Some(ref key) = self.api_key {
            request = request.header("OK-ACCESS-KEY", key);
        }

        // Measure request latency
        let request_start = std::time::Instant::now();
        let response = request.send().await?;
        let latency_ms = request_start.elapsed().as_millis() as u64;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            warn!(
                "OKX API returned {}: {}",
                status,
                &text[..text.len().min(200)]
            );
            self.price_cache
                .record_source_error_metrics(PriceSource::Okx, &format!("HTTP {}", status));
            return Err(anyhow::anyhow!("OKX API error: {}", status));
        }

        let data: OkxResponse = response.json().await?;

        if data.code != "0" {
            warn!("OKX API error: {} - {}", data.code, data.msg);
            self.price_cache
                .record_source_error_metrics(PriceSource::Okx, &format!("API error: {}", data.msg));
            return Err(anyhow::anyhow!("OKX API error: {}", data.msg));
        }

        let timestamp = chrono::Utc::now().timestamp_millis();

        // Build pair lookup
        let pair_to_symbol: HashMap<&str, &str> =
            SYMBOL_PAIRS.iter().map(|(s, p)| (*p, *s)).collect();

        for ticker in data.data {
            if let Some(symbol) = pair_to_symbol.get(ticker.inst_id.as_str()) {
                let price: f64 = ticker.last.parse().unwrap_or(0.0);
                let volume_24h: f64 = ticker.vol_ccy24h.parse().unwrap_or(0.0);

                if price > 0.0 {
                    debug!("OKX price update: {} = ${}", symbol, price);
                    self.price_cache.update_price_with_latency(
                        symbol,
                        PriceSource::Okx,
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
    // OkxTicker Tests
    // =========================================================================

    #[test]
    fn test_okx_ticker_deserialization() {
        let json = r#"{
            "instId": "BTC-USDT",
            "last": "43500.50",
            "volCcy24h": "15000000000"
        }"#;

        let ticker: OkxTicker = serde_json::from_str(json).unwrap();
        assert_eq!(ticker.inst_id, "BTC-USDT");
        assert_eq!(ticker.last, "43500.50");
        assert_eq!(ticker.vol_ccy24h, "15000000000");
    }

    #[test]
    fn test_okx_ticker_parse_values() {
        let json = r#"{"instId": "ETH-USDT", "last": "2500.00", "volCcy24h": "8000000000"}"#;
        let ticker: OkxTicker = serde_json::from_str(json).unwrap();
        let price: f64 = ticker.last.parse().unwrap();
        let volume: f64 = ticker.vol_ccy24h.parse().unwrap();
        assert_eq!(price, 2500.0);
        assert_eq!(volume, 8000000000.0);
    }

    // =========================================================================
    // OkxResponse Tests
    // =========================================================================

    #[test]
    fn test_okx_response_success() {
        let json = r#"{
            "code": "0",
            "msg": "",
            "data": [
                {"instId": "BTC-USDT", "last": "43500.50", "volCcy24h": "15000000000"}
            ]
        }"#;

        let response: OkxResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.code, "0");
        assert!(response.msg.is_empty());
        assert_eq!(response.data.len(), 1);
    }

    #[test]
    fn test_okx_response_error() {
        let json = r#"{
            "code": "50001",
            "msg": "Invalid request",
            "data": []
        }"#;

        let response: OkxResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.code, "50001");
        assert_eq!(response.msg, "Invalid request");
        assert!(response.data.is_empty());
    }

    #[test]
    fn test_okx_response_multiple_tickers() {
        let json = r#"{
            "code": "0",
            "msg": "",
            "data": [
                {"instId": "BTC-USDT", "last": "43500.50", "volCcy24h": "15000000000"},
                {"instId": "ETH-USDT", "last": "2500.00", "volCcy24h": "8000000000"}
            ]
        }"#;

        let response: OkxResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.data.len(), 2);
    }
}
