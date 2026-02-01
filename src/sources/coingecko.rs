use crate::services::{ChartStore, PriceCache};
use crate::types::PriceSource;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, error, info, warn};

const COINGECKO_API_URL: &str = "https://api.coingecko.com/api/v3";
const COINGECKO_PRO_API_URL: &str = "https://pro-api.coingecko.com/api/v3";
const POLL_INTERVAL_SECS: u64 = 120; // Rate limit friendly (free tier: 10-30 calls/min)

/// Symbol to CoinGecko ID mapping.
pub const SYMBOL_TO_ID: &[(&str, &str)] = &[
    ("btc", "bitcoin"),
    ("eth", "ethereum"),
    ("bnb", "binancecoin"),
    ("sol", "solana"),
    ("xrp", "ripple"),
    ("doge", "dogecoin"),
    ("ada", "cardano"),
    ("avax", "avalanche-2"),
    ("dot", "polkadot"),
    ("link", "chainlink"),
    ("matic", "matic-network"),
    ("shib", "shiba-inu"),
    ("ltc", "litecoin"),
    ("trx", "tron"),
    ("atom", "cosmos"),
    ("uni", "uniswap"),
    ("xlm", "stellar"),
    ("bch", "bitcoin-cash"),
    ("near", "near"),
    ("apt", "aptos"),
];

/// CoinGecko market data with sparkline.
#[derive(Debug, Deserialize)]
struct CoinGeckoMarket {
    id: String,
    symbol: String,
    current_price: Option<f64>,
    total_volume: Option<f64>,
    price_change_percentage_24h: Option<f64>,
    sparkline_in_7d: Option<SparklineData>,
}

#[derive(Debug, Deserialize)]
struct SparklineData {
    price: Vec<f64>,
}

#[derive(Debug, Deserialize)]
struct CoinGeckoPrice {
    usd: Option<f64>,
    usd_24h_vol: Option<f64>,
}

/// CoinGecko REST client.
#[derive(Clone)]
pub struct CoinGeckoClient {
    client: Client,
    api_key: Option<String>,
    price_cache: Arc<PriceCache>,
    chart_store: Arc<ChartStore>,
    seeded: Arc<AtomicBool>,
}

impl CoinGeckoClient {
    /// Create a new CoinGecko client.
    pub fn new(
        api_key: Option<String>,
        price_cache: Arc<PriceCache>,
        chart_store: Arc<ChartStore>,
    ) -> Self {
        // Create client with proper User-Agent
        let client = Client::builder()
            .user_agent("Haunt/1.0 (Cryptocurrency Price Aggregator)")
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            api_key,
            price_cache,
            chart_store,
            seeded: Arc::new(AtomicBool::new(false)),
        }
    }

    fn base_url(&self) -> &str {
        if self.api_key.is_some() {
            COINGECKO_PRO_API_URL
        } else {
            COINGECKO_API_URL
        }
    }

    /// Start polling for price updates.
    pub async fn start_polling(&self) {
        info!("Starting CoinGecko price polling");

        // First fetch with sparkline data to seed charts
        if let Err(e) = self.fetch_markets_with_sparkline().await {
            warn!("Failed to seed sparkline data from CoinGecko: {}", e);
        }

        loop {
            // Use simpler price endpoint for ongoing updates
            if let Err(e) = self.fetch_prices().await {
                error!("CoinGecko fetch error: {}", e);
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
        }
    }

    /// Fetch markets with sparkline data to seed historical charts.
    async fn fetch_markets_with_sparkline(&self) -> anyhow::Result<()> {
        let ids: Vec<&str> = SYMBOL_TO_ID.iter().map(|(_, id)| *id).collect();
        let ids_str = ids.join(",");

        let mut url = format!(
            "{}/coins/markets?vs_currency=usd&ids={}&sparkline=true&price_change_percentage=24h",
            self.base_url(),
            ids_str
        );

        if let Some(ref key) = self.api_key {
            url.push_str(&format!("&x_cg_pro_api_key={}", key));
        }

        info!("Fetching CoinGecko market data with sparklines...");

        let response = self.client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            warn!("CoinGecko API returned {}: {}", status, &text[..text.len().min(200)]);
            return Err(anyhow::anyhow!("CoinGecko API error: {}", status));
        }

        let markets: Vec<CoinGeckoMarket> = response.json().await?;

        let timestamp = chrono::Utc::now().timestamp_millis();
        let id_to_symbol: HashMap<&str, &str> = SYMBOL_TO_ID.iter()
            .map(|(s, id)| (*id, *s))
            .collect();

        for market in markets {
            if let Some(symbol) = id_to_symbol.get(market.id.as_str()) {
                // Seed sparkline data (take last 60 points for 1-hour view)
                if let Some(ref sparkline) = market.sparkline_in_7d {
                    let prices: Vec<f64> = sparkline.price.iter()
                        .rev()
                        .take(60)
                        .rev()
                        .copied()
                        .collect();

                    if !prices.is_empty() {
                        self.chart_store.seed_sparkline(symbol, &prices);
                        info!("Seeded {} sparkline points for {}", prices.len(), symbol);
                    }
                }

                // Update current price
                if let Some(price) = market.current_price {
                    self.price_cache.update_price(
                        symbol,
                        PriceSource::CoinGecko,
                        price,
                        market.total_volume,
                    );
                    self.chart_store.add_price(symbol, price, market.total_volume, timestamp);
                }
            }
        }

        self.seeded.store(true, Ordering::SeqCst);
        info!("CoinGecko sparkline seeding complete");

        Ok(())
    }

    async fn fetch_prices(&self) -> anyhow::Result<()> {
        let ids: Vec<&str> = SYMBOL_TO_ID.iter().map(|(_, id)| *id).collect();
        let ids_str = ids.join(",");

        let mut url = format!(
            "{}/simple/price?ids={}&vs_currencies=usd&include_24hr_vol=true",
            self.base_url(),
            ids_str
        );

        if let Some(ref key) = self.api_key {
            url.push_str(&format!("&x_cg_pro_api_key={}", key));
        }

        let response: HashMap<String, CoinGeckoPrice> = self.client
            .get(&url)
            .send()
            .await?
            .json()
            .await?;

        let timestamp = chrono::Utc::now().timestamp_millis();

        for (symbol, id) in SYMBOL_TO_ID {
            if let Some(price_data) = response.get(*id) {
                if let Some(price) = price_data.usd {
                    debug!("CoinGecko price update: {} = ${}", symbol, price);
                    self.price_cache.update_price(
                        symbol,
                        PriceSource::CoinGecko,
                        price,
                        price_data.usd_24h_vol,
                    );
                    self.chart_store.add_price(symbol, price, price_data.usd_24h_vol, timestamp);
                }
            }
        }

        Ok(())
    }
}
