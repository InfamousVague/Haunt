use crate::services::{ChartStore, PriceCache};
use crate::types::PriceSource;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

const HUOBI_API_URL: &str = "https://api.huobi.pro";
const POLL_INTERVAL_SECS: u64 = 45;

/// Symbol mapping for Huobi (symbol -> Huobi trading pair).
pub const SYMBOL_PAIRS: &[(&str, &str)] = &[
    ("btc", "btcusdt"),
    ("eth", "ethusdt"),
    ("sol", "solusdt"),
    ("xrp", "xrpusdt"),
    ("doge", "dogeusdt"),
    ("ada", "adausdt"),
    ("avax", "avaxusdt"),
    ("dot", "dotusdt"),
    ("link", "linkusdt"),
    ("matic", "maticusdt"),
    ("shib", "shibusdt"),
    ("ltc", "ltcusdt"),
    ("trx", "trxusdt"),
    ("atom", "atomusdt"),
    ("uni", "uniusdt"),
    ("xlm", "xlmusdt"),
    ("bch", "bchusdt"),
    ("near", "nearusdt"),
    ("apt", "aptusdt"),
];

/// Huobi tickers response.
#[derive(Debug, Deserialize)]
struct HuobiResponse {
    status: String,
    data: Option<Vec<HuobiTicker>>,
}

#[derive(Debug, Deserialize)]
struct HuobiTicker {
    symbol: String,
    close: f64,
    vol: f64,
    amount: f64,
}

/// Huobi REST client.
#[derive(Clone)]
pub struct HuobiClient {
    client: Client,
    api_key: Option<String>,
    price_cache: Arc<PriceCache>,
    chart_store: Arc<ChartStore>,
}

impl HuobiClient {
    /// Create a new Huobi client.
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
        info!("Starting Huobi price polling");

        loop {
            if let Err(e) = self.fetch_prices().await {
                error!("Huobi fetch error: {}", e);
                self.price_cache.report_source_error(PriceSource::Huobi, &e.to_string());
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
        }
    }

    async fn fetch_prices(&self) -> anyhow::Result<()> {
        // Fetch all tickers in one request
        let url = format!("{}/market/tickers", HUOBI_API_URL);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            warn!("Huobi API returned {}: {}", status, &text[..text.len().min(200)]);
            return Err(anyhow::anyhow!("Huobi API error: {}", status));
        }

        let data: HuobiResponse = response.json().await?;

        if data.status != "ok" {
            warn!("Huobi API error: {}", data.status);
            return Err(anyhow::anyhow!("Huobi API error: {}", data.status));
        }

        let tickers = match data.data {
            Some(t) => t,
            None => return Ok(()),
        };

        let timestamp = chrono::Utc::now().timestamp_millis();

        // Build pair lookup
        let pair_to_symbol: HashMap<&str, &str> = SYMBOL_PAIRS.iter()
            .map(|(s, p)| (*p, *s))
            .collect();

        for ticker in tickers {
            if let Some(symbol) = pair_to_symbol.get(ticker.symbol.as_str()) {
                let price = ticker.close;
                // Huobi vol is in base currency, amount is in quote currency
                let volume_24h = ticker.amount;

                if price > 0.0 {
                    debug!("Huobi price update: {} = ${}", symbol, price);
                    self.price_cache.update_price(
                        symbol,
                        PriceSource::Huobi,
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
