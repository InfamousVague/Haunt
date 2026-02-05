use crate::services::{ChartStore, PriceCache};
use crate::types::PriceSource;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info};

const CRYPTOCOMPARE_API_URL: &str = "https://min-api.cryptocompare.com/data";
const POLL_INTERVAL_SECS: u64 = 30;

/// Symbols to fetch from CryptoCompare.
const SYMBOLS: &[&str] = &[
    "BTC", "ETH", "BNB", "SOL", "XRP", "DOGE", "ADA", "AVAX", "DOT", "LINK", "MATIC", "SHIB",
    "LTC", "TRX", "ATOM", "UNI", "XLM", "BCH", "NEAR", "APT",
];

#[derive(Debug, Deserialize)]
struct CryptoCompareResponse {
    #[serde(rename = "RAW")]
    raw: Option<HashMap<String, HashMap<String, CryptoComparePriceData>>>,
}

#[derive(Debug, Deserialize)]
struct CryptoComparePriceData {
    #[serde(rename = "PRICE")]
    price: Option<f64>,
    #[serde(rename = "VOLUME24HOUR")]
    volume_24h: Option<f64>,
}

/// CryptoCompare REST client.
#[derive(Clone)]
pub struct CryptoCompareClient {
    client: Client,
    api_key: String,
    price_cache: Arc<PriceCache>,
    chart_store: Arc<ChartStore>,
}

impl CryptoCompareClient {
    /// Create a new CryptoCompare client.
    pub fn new(
        api_key: String,
        price_cache: Arc<PriceCache>,
        chart_store: Arc<ChartStore>,
    ) -> Self {
        Self {
            client: Client::new(),
            api_key,
            price_cache,
            chart_store,
        }
    }

    /// Start polling for price updates.
    pub async fn start_polling(&self) {
        info!("Starting CryptoCompare price polling");

        loop {
            if let Err(e) = self.fetch_prices().await {
                error!("CryptoCompare fetch error: {}", e);
                self.price_cache
                    .report_source_error(PriceSource::CryptoCompare, &e.to_string());
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
        }
    }

    async fn fetch_prices(&self) -> anyhow::Result<()> {
        let fsyms = SYMBOLS.join(",");
        let url = format!(
            "{}/pricemultifull?fsyms={}&tsyms=USD",
            CRYPTOCOMPARE_API_URL, fsyms
        );

        // Measure request latency
        let request_start = std::time::Instant::now();
        let http_response = self
            .client
            .get(&url)
            .header("Authorization", format!("Apikey {}", self.api_key))
            .send()
            .await?;
        let latency_ms = request_start.elapsed().as_millis() as u64;

        if !http_response.status().is_success() {
            let status = http_response.status();
            self.price_cache
                .record_source_error_metrics(PriceSource::CryptoCompare, &format!("HTTP {}", status));
            return Err(anyhow::anyhow!("CryptoCompare API error: {}", status));
        }

        let response: CryptoCompareResponse = http_response.json().await?;

        let timestamp = chrono::Utc::now().timestamp_millis();

        if let Some(raw) = response.raw {
            for symbol in SYMBOLS {
                if let Some(usd_data) = raw.get(*symbol).and_then(|m| m.get("USD")) {
                    if let Some(price) = usd_data.price {
                        let symbol_lower = symbol.to_lowercase();
                        debug!("CryptoCompare price update: {} = ${}", symbol_lower, price);
                        self.price_cache.update_price_with_latency(
                            &symbol_lower,
                            PriceSource::CryptoCompare,
                            price,
                            usd_data.volume_24h,
                            latency_ms,
                        );
                        self.chart_store.add_price(
                            &symbol_lower,
                            price,
                            usd_data.volume_24h,
                            timestamp,
                        );
                    }
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
    // SYMBOLS Tests
    // =========================================================================

    #[test]
    fn test_symbols_contains_major_cryptos() {
        assert!(SYMBOLS.contains(&"BTC"));
        assert!(SYMBOLS.contains(&"ETH"));
        assert!(SYMBOLS.contains(&"SOL"));
        assert!(SYMBOLS.contains(&"DOGE"));
    }

    #[test]
    fn test_symbols_count() {
        assert!(SYMBOLS.len() >= 20);
    }

    #[test]
    fn test_symbols_uppercase() {
        for symbol in SYMBOLS {
            assert_eq!(*symbol, symbol.to_uppercase());
        }
    }

    // =========================================================================
    // CryptoComparePriceData Tests
    // =========================================================================

    #[test]
    fn test_cryptocompare_price_data_deserialization() {
        let json = r#"{"PRICE": 43500.50, "VOLUME24HOUR": 15000000000}"#;
        let data: CryptoComparePriceData = serde_json::from_str(json).unwrap();
        assert_eq!(data.price, Some(43500.50));
        assert_eq!(data.volume_24h, Some(15000000000.0));
    }

    #[test]
    fn test_cryptocompare_price_data_minimal() {
        let json = r#"{}"#;
        let data: CryptoComparePriceData = serde_json::from_str(json).unwrap();
        assert!(data.price.is_none());
        assert!(data.volume_24h.is_none());
    }

    #[test]
    fn test_cryptocompare_price_data_with_only_price() {
        let json = r#"{"PRICE": 2500.00}"#;
        let data: CryptoComparePriceData = serde_json::from_str(json).unwrap();
        assert_eq!(data.price, Some(2500.00));
        assert!(data.volume_24h.is_none());
    }

    // =========================================================================
    // CryptoCompareResponse Tests
    // =========================================================================

    #[test]
    fn test_cryptocompare_response_deserialization() {
        let json = r#"{
            "RAW": {
                "BTC": {
                    "USD": {"PRICE": 43500.50, "VOLUME24HOUR": 15000000000}
                },
                "ETH": {
                    "USD": {"PRICE": 2500.00, "VOLUME24HOUR": 8000000000}
                }
            }
        }"#;

        let response: CryptoCompareResponse = serde_json::from_str(json).unwrap();
        assert!(response.raw.is_some());
        let raw = response.raw.unwrap();
        assert!(raw.contains_key("BTC"));
        assert!(raw.contains_key("ETH"));

        let btc_usd = raw.get("BTC").unwrap().get("USD").unwrap();
        assert_eq!(btc_usd.price, Some(43500.50));
    }

    #[test]
    fn test_cryptocompare_response_empty() {
        let json = r#"{}"#;
        let response: CryptoCompareResponse = serde_json::from_str(json).unwrap();
        assert!(response.raw.is_none());
    }

    #[test]
    fn test_cryptocompare_response_null_raw() {
        let json = r#"{"RAW": null}"#;
        let response: CryptoCompareResponse = serde_json::from_str(json).unwrap();
        assert!(response.raw.is_none());
    }
}
