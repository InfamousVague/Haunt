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
                self.price_cache.report_source_error(PriceSource::Kraken, &e.to_string());
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
        }
    }

    async fn fetch_prices(&self) -> anyhow::Result<()> {
        let pairs: Vec<&str> = SYMBOL_PAIRS.iter().map(|(_, p)| *p).collect();
        let pairs_str = pairs.join(",");

        let url = format!("{}/Ticker?pair={}", KRAKEN_API_URL, pairs_str);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            warn!("Kraken API returned {}: {}", status, &text[..text.len().min(200)]);
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
        let pair_to_symbol: HashMap<&str, &str> = SYMBOL_PAIRS.iter()
            .map(|(s, p)| (*p, *s))
            .collect();

        for (pair, ticker) in result {
            // Kraken sometimes returns slightly different pair names
            let symbol = pair_to_symbol.get(pair.as_str())
                .or_else(|| {
                    // Try without trailing zeros or with alternate format
                    SYMBOL_PAIRS.iter()
                        .find(|(_, p)| pair.starts_with(*p) || p.starts_with(&pair))
                        .map(|(s, _)| s)
                })
                .copied();

            if let Some(symbol) = symbol {
                let price: f64 = ticker.c.first()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(0.0);

                let volume_24h: f64 = ticker.v.get(1)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.0);

                if price > 0.0 {
                    debug!("Kraken price update: {} = ${}", symbol, price);
                    self.price_cache.update_price(
                        symbol,
                        PriceSource::Kraken,
                        price,
                        Some(volume_24h),
                    );
                    self.chart_store.add_price(symbol, price, Some(volume_24h), timestamp);
                }
            }
        }

        Ok(())
    }
}
