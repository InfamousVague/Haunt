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
    "BTC", "ETH", "BNB", "SOL", "XRP", "DOGE", "ADA", "AVAX", "DOT", "LINK",
    "MATIC", "SHIB", "LTC", "TRX", "ATOM", "UNI", "XLM", "BCH", "NEAR", "APT",
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
                self.price_cache.report_source_error(PriceSource::CryptoCompare, &e.to_string());
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
        }
    }

    async fn fetch_prices(&self) -> anyhow::Result<()> {
        let fsyms = SYMBOLS.join(",");
        let url = format!(
            "{}/pricemultifull?fsyms={}&tsyms=USD",
            CRYPTOCOMPARE_API_URL,
            fsyms
        );

        let response: CryptoCompareResponse = self.client
            .get(&url)
            .header("Authorization", format!("Apikey {}", self.api_key))
            .send()
            .await?
            .json()
            .await?;

        let timestamp = chrono::Utc::now().timestamp_millis();

        if let Some(raw) = response.raw {
            for symbol in SYMBOLS {
                if let Some(usd_data) = raw.get(*symbol).and_then(|m| m.get("USD")) {
                    if let Some(price) = usd_data.price {
                        let symbol_lower = symbol.to_lowercase();
                        debug!("CryptoCompare price update: {} = ${}", symbol_lower, price);
                        self.price_cache.update_price(
                            &symbol_lower,
                            PriceSource::CryptoCompare,
                            price,
                            usd_data.volume_24h,
                        );
                        self.chart_store.add_price(&symbol_lower, price, usd_data.volume_24h, timestamp);
                    }
                }
            }
        }

        Ok(())
    }
}
