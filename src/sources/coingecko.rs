// Some fields are kept for API completeness
#![allow(dead_code)]

use crate::services::{ChartStore, PriceCache};
use crate::types::PriceSource;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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
                self.price_cache
                    .report_source_error(PriceSource::CoinGecko, &e.to_string());
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

        let response = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            warn!(
                "CoinGecko API returned {}: {}",
                status,
                &text[..text.len().min(200)]
            );
            return Err(anyhow::anyhow!("CoinGecko API error: {}", status));
        }

        let markets: Vec<CoinGeckoMarket> = response.json().await?;

        let timestamp = chrono::Utc::now().timestamp_millis();
        let id_to_symbol: HashMap<&str, &str> =
            SYMBOL_TO_ID.iter().map(|(s, id)| (*id, *s)).collect();

        for market in markets {
            if let Some(symbol) = id_to_symbol.get(market.id.as_str()) {
                // Update current price - sparkline will build up from real-time updates
                // Note: We don't seed from CoinGecko's 7-day hourly sparkline data because
                // our mini charts show minute-level pulse data, not historical trends
                if let Some(price) = market.current_price {
                    // Seed sparkline with current price to provide a starting baseline
                    // (10 points gives a flat line to start)
                    let baseline: Vec<f64> = vec![price; 10];
                    self.chart_store.seed_sparkline(symbol, &baseline);
                    info!("Seeded baseline sparkline for {} at ${:.2}", symbol, price);

                    self.price_cache.update_price(
                        symbol,
                        PriceSource::CoinGecko,
                        price,
                        market.total_volume,
                    );
                    self.chart_store
                        .add_price(symbol, price, market.total_volume, timestamp);
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

        let response: HashMap<String, CoinGeckoPrice> =
            self.client.get(&url).send().await?.json().await?;

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
                    self.chart_store
                        .add_price(symbol, price, price_data.usd_24h_vol, timestamp);
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
    // SYMBOL_TO_ID Tests
    // =========================================================================

    #[test]
    fn test_symbol_to_id_contains_bitcoin() {
        let btc = SYMBOL_TO_ID.iter().find(|(s, _)| *s == "btc");
        assert!(btc.is_some());
        assert_eq!(btc.unwrap().1, "bitcoin");
    }

    #[test]
    fn test_symbol_to_id_contains_ethereum() {
        let eth = SYMBOL_TO_ID.iter().find(|(s, _)| *s == "eth");
        assert!(eth.is_some());
        assert_eq!(eth.unwrap().1, "ethereum");
    }

    #[test]
    fn test_symbol_to_id_count() {
        assert!(SYMBOL_TO_ID.len() >= 20);
    }

    #[test]
    fn test_symbol_to_id_lowercase() {
        for (symbol, _) in SYMBOL_TO_ID {
            assert_eq!(*symbol, symbol.to_lowercase());
        }
    }

    // =========================================================================
    // CoinGeckoMarket Tests
    // =========================================================================

    #[test]
    fn test_coingecko_market_deserialization() {
        let json = r#"{
            "id": "bitcoin",
            "symbol": "btc",
            "current_price": 43500.50,
            "total_volume": 15000000000,
            "price_change_percentage_24h": 2.5,
            "sparkline_in_7d": {"price": [43000, 43500, 44000]}
        }"#;

        let market: CoinGeckoMarket = serde_json::from_str(json).unwrap();
        assert_eq!(market.id, "bitcoin");
        assert_eq!(market.symbol, "btc");
        assert_eq!(market.current_price, Some(43500.50));
        assert_eq!(market.total_volume, Some(15000000000.0));
        assert_eq!(market.price_change_percentage_24h, Some(2.5));
        assert!(market.sparkline_in_7d.is_some());
        assert_eq!(market.sparkline_in_7d.unwrap().price.len(), 3);
    }

    #[test]
    fn test_coingecko_market_minimal() {
        let json = r#"{
            "id": "ethereum",
            "symbol": "eth"
        }"#;

        let market: CoinGeckoMarket = serde_json::from_str(json).unwrap();
        assert_eq!(market.id, "ethereum");
        assert!(market.current_price.is_none());
        assert!(market.sparkline_in_7d.is_none());
    }

    // =========================================================================
    // SparklineData Tests
    // =========================================================================

    #[test]
    fn test_sparkline_data_deserialization() {
        let json = r#"{"price": [100.0, 101.5, 99.8, 102.3]}"#;
        let sparkline: SparklineData = serde_json::from_str(json).unwrap();
        assert_eq!(sparkline.price.len(), 4);
        assert_eq!(sparkline.price[0], 100.0);
    }

    #[test]
    fn test_sparkline_data_empty() {
        let json = r#"{"price": []}"#;
        let sparkline: SparklineData = serde_json::from_str(json).unwrap();
        assert!(sparkline.price.is_empty());
    }

    // =========================================================================
    // CoinGeckoPrice Tests
    // =========================================================================

    #[test]
    fn test_coingecko_price_deserialization() {
        let json = r#"{"usd": 43500.50, "usd_24h_vol": 15000000000}"#;
        let price: CoinGeckoPrice = serde_json::from_str(json).unwrap();
        assert_eq!(price.usd, Some(43500.50));
        assert_eq!(price.usd_24h_vol, Some(15000000000.0));
    }

    #[test]
    fn test_coingecko_price_minimal() {
        let json = r#"{}"#;
        let price: CoinGeckoPrice = serde_json::from_str(json).unwrap();
        assert!(price.usd.is_none());
        assert!(price.usd_24h_vol.is_none());
    }

    #[test]
    fn test_coingecko_price_with_only_usd() {
        let json = r#"{"usd": 2500.00}"#;
        let price: CoinGeckoPrice = serde_json::from_str(json).unwrap();
        assert_eq!(price.usd, Some(2500.00));
        assert!(price.usd_24h_vol.is_none());
    }
}
