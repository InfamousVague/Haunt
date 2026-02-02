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
                self.price_cache.report_source_error(PriceSource::Okx, &e.to_string());
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

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            warn!("OKX API returned {}: {}", status, &text[..text.len().min(200)]);
            return Err(anyhow::anyhow!("OKX API error: {}", status));
        }

        let data: OkxResponse = response.json().await?;

        if data.code != "0" {
            warn!("OKX API error: {} - {}", data.code, data.msg);
            return Err(anyhow::anyhow!("OKX API error: {}", data.msg));
        }

        let timestamp = chrono::Utc::now().timestamp_millis();

        // Build pair lookup
        let pair_to_symbol: HashMap<&str, &str> = SYMBOL_PAIRS.iter()
            .map(|(s, p)| (*p, *s))
            .collect();

        for ticker in data.data {
            if let Some(symbol) = pair_to_symbol.get(ticker.inst_id.as_str()) {
                let price: f64 = ticker.last.parse().unwrap_or(0.0);
                let volume_24h: f64 = ticker.vol_ccy24h.parse().unwrap_or(0.0);

                if price > 0.0 {
                    debug!("OKX price update: {} = ${}", symbol, price);
                    self.price_cache.update_price(
                        symbol,
                        PriceSource::Okx,
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
