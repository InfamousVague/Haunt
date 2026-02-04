// Some structs/constants are kept for API completeness
#![allow(dead_code)]

use crate::services::{Cache, ChartStore, FileCache, PriceCache};
use crate::types::{
    Asset, AssetListing, FearGreedData, GlobalMetrics, PaginatedResponse, PriceSource, Quote,
};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

const CMC_API_URL: &str = "https://pro-api.coinmarketcap.com/v1";
const CMC_API_V2_URL: &str = "https://pro-api.coinmarketcap.com/v2";
const POLL_INTERVAL_SECS: u64 = 60;

/// CoinMarketCap REST client.
#[derive(Clone)]
pub struct CoinMarketCapClient {
    client: Client,
    api_key: String,
    price_cache: Arc<PriceCache>,
    chart_store: Arc<ChartStore>,
    listings_cache: Arc<Cache<Vec<AssetListing>>>,
    asset_cache: Arc<Cache<Asset>>,
    global_cache: Arc<Cache<GlobalMetrics>>,
    fear_greed_cache: Arc<Cache<FearGreedData>>,
    file_cache: Arc<FileCache>,
}

/// File cache TTL for listings (24 hours - used as ultimate fallback)
const FILE_CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Deserialize)]
struct CmcResponse<T> {
    data: T,
}

#[derive(Debug, Deserialize)]
struct CmcListingData {
    id: i64,
    name: String,
    symbol: String,
    slug: String,
    cmc_rank: Option<i32>,
    quote: Option<HashMap<String, CmcQuote>>,
}

#[derive(Debug, Clone, Deserialize)]
struct CmcQuote {
    price: Option<f64>,
    volume_24h: Option<f64>,
    volume_change_24h: Option<f64>,
    percent_change_1h: Option<f64>,
    percent_change_24h: Option<f64>,
    percent_change_7d: Option<f64>,
    percent_change_30d: Option<f64>,
    market_cap: Option<f64>,
    market_cap_dominance: Option<f64>,
    fully_diluted_market_cap: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct CmcGlobalData {
    total_market_cap: Option<HashMap<String, f64>>,
    total_volume_24h: Option<HashMap<String, f64>>,
    total_market_cap_yesterday_percentage_change: Option<f64>,
    btc_dominance: Option<f64>,
    eth_dominance: Option<f64>,
    active_cryptocurrencies: Option<i32>,
    active_exchanges: Option<i32>,
    defi_volume_24h: Option<f64>,
    defi_market_cap: Option<f64>,
    stablecoin_volume_24h: Option<f64>,
    stablecoin_market_cap: Option<f64>,
    last_updated: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CmcFearGreedResponse {
    data: CmcFearGreedData,
}

#[derive(Debug, Deserialize)]
struct CmcFearGreedData {
    value: i32,
    value_classification: String,
    update_time: String,
}

impl CoinMarketCapClient {
    /// Create a new CoinMarketCap client.
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
            listings_cache: Arc::new(Cache::new(Duration::from_secs(300))),
            asset_cache: Arc::new(Cache::new(Duration::from_secs(60))),
            global_cache: Arc::new(Cache::new(Duration::from_secs(60))),
            fear_greed_cache: Arc::new(Cache::new(Duration::from_secs(3600))),
            file_cache: Arc::new(FileCache::new()),
        }
    }

    /// Start polling for price updates.
    pub async fn start_polling(&self) {
        info!("Starting CoinMarketCap price polling");

        loop {
            if let Err(e) = self.fetch_listings().await {
                error!("CoinMarketCap fetch error: {}", e);
                self.price_cache
                    .report_source_error(PriceSource::CoinMarketCap, &e.to_string());
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
        }
    }

    /// Fetch paginated listings.
    pub async fn get_listings(
        &self,
        page: i32,
        limit: i32,
    ) -> anyhow::Result<PaginatedResponse<AssetListing>> {
        let cache_key = format!("listings_{}_{}", page, limit);
        let file_cache_key = format!("cmc_listings_{}_{}", page, limit);

        // Check memory cache first - but still populate sparklines
        if let Some(mut cached) = self.listings_cache.get(&cache_key) {
            // Populate sparklines from chart store (these update in real-time)
            for listing in &mut cached {
                listing.sparkline = self.chart_store.get_sparkline(&listing.symbol, 60);
            }
            return Ok(PaginatedResponse {
                data: cached,
                page,
                limit,
                total: 10000, // CMC has many assets
                has_more: true,
            });
        }

        let start = (page - 1) * limit + 1;
        let url = format!(
            "{}/cryptocurrency/listings/latest?start={}&limit={}&convert=USD",
            CMC_API_URL, start, limit
        );

        // Try to fetch from API
        let api_result = async {
            let response: CmcResponse<Vec<CmcListingData>> = self
                .client
                .get(&url)
                .header("X-CMC_PRO_API_KEY", &self.api_key)
                .send()
                .await?
                .json()
                .await?;

            let listings: Vec<AssetListing> = response
                .data
                .into_iter()
                .map(|d| self.convert_listing(d))
                .collect();

            Ok::<Vec<AssetListing>, anyhow::Error>(listings)
        }
        .await;

        match api_result {
            Ok(mut listings) => {
                // Populate sparklines from chart store
                for listing in &mut listings {
                    listing.sparkline = self.chart_store.get_sparkline(&listing.symbol, 60);
                }

                // Save to both memory and file cache
                self.listings_cache.set(cache_key, listings.clone());
                self.file_cache.set(&file_cache_key, &listings);

                Ok(PaginatedResponse {
                    data: listings,
                    page,
                    limit,
                    total: 10000,
                    has_more: true,
                })
            }
            Err(e) => {
                warn!("CMC API failed, trying file cache fallback: {}", e);

                // Try file cache (fresh first, then stale)
                let file_cached: Option<Vec<AssetListing>> = self
                    .file_cache
                    .get(&file_cache_key, FILE_CACHE_TTL)
                    .or_else(|| {
                        warn!("Using stale file cache for listings");
                        self.file_cache.get_stale(&file_cache_key)
                    });

                if let Some(mut cached) = file_cached {
                    // Update sparklines from chart store
                    for listing in &mut cached {
                        listing.sparkline = self.chart_store.get_sparkline(&listing.symbol, 60);
                    }
                    info!("Serving {} listings from file cache fallback", cached.len());
                    return Ok(PaginatedResponse {
                        data: cached,
                        page,
                        limit,
                        total: 10000,
                        has_more: true,
                    });
                }

                // No cache available, propagate error
                Err(e)
            }
        }
    }

    /// Get a single asset by ID.
    pub async fn get_asset(&self, id: i64) -> anyhow::Result<Option<Asset>> {
        let cache_key = format!("asset_{}", id);

        if let Some(cached) = self.asset_cache.get(&cache_key) {
            return Ok(Some(cached));
        }

        let url = format!(
            "{}/cryptocurrency/quotes/latest?id={}&convert=USD",
            CMC_API_URL, id
        );

        let resp = self
            .client
            .get(&url)
            .header("X-CMC_PRO_API_KEY", &self.api_key)
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() {
            tracing::warn!(
                "CMC API error for asset {}: {} - {}",
                id,
                status,
                &text[..text.len().min(200)]
            );
            return Ok(None);
        }

        // Parse as v1 format (data is a map with string keys)
        let parsed: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("Failed to parse CMC response for asset {}: {}", id, e);
                return Ok(None);
            }
        };

        debug!(
            "CMC get_asset {} response keys: {:?}",
            id,
            parsed.as_object().map(|o| o.keys().collect::<Vec<_>>())
        );

        let data_obj = parsed.get("data");
        if data_obj.is_none() {
            tracing::warn!("No 'data' field in CMC response for asset {}", id);
            return Ok(None);
        }

        let asset_data = data_obj.and_then(|d| d.get(id.to_string()));
        if asset_data.is_none() {
            tracing::warn!(
                "Asset {} not found in CMC data. Available keys: {:?}",
                id,
                data_obj.and_then(|d| d.as_object().map(|o| o.keys().collect::<Vec<_>>()))
            );
            return Ok(None);
        }

        let asset = asset_data
            .and_then(
                |v| match serde_json::from_value::<CmcListingData>(v.clone()) {
                    Ok(data) => Some(data),
                    Err(e) => {
                        tracing::warn!("Failed to parse asset data for {}: {}", id, e);
                        None
                    }
                },
            )
            .map(|d| self.convert_to_asset(d));

        if let Some(ref a) = asset {
            self.asset_cache.set(cache_key, a.clone());
        }

        Ok(asset)
    }

    /// Search for assets.
    pub async fn search(&self, query: &str, limit: i32) -> anyhow::Result<Vec<AssetListing>> {
        // CMC doesn't have a direct search endpoint, so we fetch listings and filter
        let listings = self.get_listings(1, 100).await?;

        let query_lower = query.to_lowercase();
        let results: Vec<AssetListing> = listings
            .data
            .into_iter()
            .filter(|l| {
                l.name.to_lowercase().contains(&query_lower)
                    || l.symbol.to_lowercase().contains(&query_lower)
            })
            .take(limit as usize)
            .collect();

        Ok(results)
    }

    /// Get global market metrics.
    pub async fn get_global_metrics(&self) -> anyhow::Result<GlobalMetrics> {
        if let Some(cached) = self.global_cache.get("global") {
            return Ok(cached);
        }

        let url = format!("{}/global-metrics/quotes/latest", CMC_API_URL);

        let resp = self
            .client
            .get(&url)
            .header("X-CMC_PRO_API_KEY", &self.api_key)
            .send()
            .await?;

        let text = resp.text().await?;
        let parsed: serde_json::Value = serde_json::from_str(&text)?;

        let data = parsed.get("data").unwrap_or(&serde_json::Value::Null);

        // CMC returns quotes.USD for market cap/volume
        let quote = data.get("quote").and_then(|q| q.get("USD"));

        let metrics = GlobalMetrics {
            total_market_cap: quote
                .and_then(|q| q.get("total_market_cap"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            total_volume_24h: quote
                .and_then(|q| q.get("total_volume_24h"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            btc_dominance: data
                .get("btc_dominance")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            eth_dominance: data
                .get("eth_dominance")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            active_cryptocurrencies: data
                .get("active_cryptocurrencies")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32)
                .unwrap_or(0),
            active_exchanges: data
                .get("active_exchanges")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32)
                .unwrap_or(0),
            market_cap_change_24h: quote
                .and_then(|q| q.get("total_market_cap_yesterday_percentage_change"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            volume_change_24h: quote
                .and_then(|q| q.get("total_volume_24h_yesterday_percentage_change"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            defi_volume_24h: data.get("defi_volume_24h").and_then(|v| v.as_f64()),
            defi_market_cap: data.get("defi_market_cap").and_then(|v| v.as_f64()),
            stablecoin_volume_24h: data.get("stablecoin_volume_24h").and_then(|v| v.as_f64()),
            stablecoin_market_cap: data.get("stablecoin_market_cap").and_then(|v| v.as_f64()),
            last_updated: data
                .get("last_updated")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
        };

        self.global_cache.set("global".to_string(), metrics.clone());

        Ok(metrics)
    }

    /// Get Fear & Greed Index from alternative.me API (free and reliable).
    pub async fn get_fear_greed(&self) -> anyhow::Result<FearGreedData> {
        if let Some(cached) = self.fear_greed_cache.get("fear_greed") {
            return Ok(cached);
        }

        // Use alternative.me Fear & Greed API (free, no API key required)
        let url = "https://api.alternative.me/fng/?limit=1";

        let resp = self.client.get(url).send().await?;

        let text = resp.text().await?;
        let parsed: serde_json::Value = serde_json::from_str(&text)?;

        // alternative.me response format: { "data": [{ "value": "25", "value_classification": "Extreme Fear", "timestamp": "..." }] }
        let data_arr = parsed.get("data").and_then(|d| d.as_array());

        let data = if let Some(arr) = data_arr {
            if let Some(item) = arr.first() {
                FearGreedData {
                    value: item
                        .get("value")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<i32>().ok())
                        .unwrap_or(50),
                    classification: item
                        .get("value_classification")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Neutral")
                        .to_string(),
                    timestamp: item
                        .get("timestamp")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    previous_close: None,
                    previous_week: None,
                    previous_month: None,
                }
            } else {
                FearGreedData::default()
            }
        } else {
            FearGreedData::default()
        };

        self.fear_greed_cache
            .set("fear_greed".to_string(), data.clone());

        Ok(data)
    }

    async fn fetch_listings(&self) -> anyhow::Result<()> {
        let url = format!(
            "{}/cryptocurrency/listings/latest?start=1&limit=100&convert=USD",
            CMC_API_URL
        );

        let response: CmcResponse<Vec<CmcListingData>> = self
            .client
            .get(&url)
            .header("X-CMC_PRO_API_KEY", &self.api_key)
            .send()
            .await?
            .json()
            .await?;

        let timestamp = chrono::Utc::now().timestamp_millis();

        for listing in response.data {
            if let Some(quote) = listing.quote.and_then(|m| m.get("USD").cloned()) {
                if let Some(price) = quote.price {
                    let symbol = listing.symbol.to_lowercase();
                    debug!("CMC price update: {} = ${}", symbol, price);
                    self.price_cache.update_price(
                        &symbol,
                        PriceSource::CoinMarketCap,
                        price,
                        quote.volume_24h,
                    );
                    self.chart_store
                        .add_price(&symbol, price, quote.volume_24h, timestamp);
                }
            }
        }

        Ok(())
    }

    fn convert_listing(&self, data: CmcListingData) -> AssetListing {
        let quote = data.quote.and_then(|m| m.get("USD").cloned());

        // Helper to ensure values are finite (not NaN or Infinity)
        fn safe_f64(val: Option<f64>) -> f64 {
            val.filter(|v| v.is_finite()).unwrap_or(0.0)
        }

        // Get trade direction from price cache
        let trade_direction = self.price_cache.get_trade_direction(&data.symbol);

        AssetListing {
            id: data.id,
            rank: data.cmc_rank.unwrap_or(0),
            name: data.name,
            symbol: data.symbol,
            image: format!(
                "https://s2.coinmarketcap.com/static/img/coins/64x64/{}.png",
                data.id
            ),
            price: safe_f64(quote.as_ref().and_then(|q| q.price)),
            change_1h: safe_f64(quote.as_ref().and_then(|q| q.percent_change_1h)),
            change_24h: safe_f64(quote.as_ref().and_then(|q| q.percent_change_24h)),
            change_7d: safe_f64(quote.as_ref().and_then(|q| q.percent_change_7d)),
            market_cap: safe_f64(quote.as_ref().and_then(|q| q.market_cap)),
            volume_24h: safe_f64(quote.as_ref().and_then(|q| q.volume_24h)),
            circulating_supply: 0.0, // CMC listings endpoint doesn't include this
            max_supply: None,
            sparkline: vec![], // Populated after by get_listings
            trade_direction,
            asset_type: "crypto".to_string(),
            exchange: None,
            sector: None,
        }
    }

    fn convert_to_asset(&self, data: CmcListingData) -> Asset {
        let quote = data
            .quote
            .and_then(|m| m.get("USD").cloned())
            .map(|q| Quote {
                price: q.price.unwrap_or(0.0),
                volume_24h: q.volume_24h,
                volume_change_24h: q.volume_change_24h,
                market_cap: q.market_cap,
                market_cap_dominance: q.market_cap_dominance,
                percent_change_1h: q.percent_change_1h,
                percent_change_24h: q.percent_change_24h,
                percent_change_7d: q.percent_change_7d,
                percent_change_30d: q.percent_change_30d,
                fully_diluted_market_cap: q.fully_diluted_market_cap,
                circulating_supply: None,
                total_supply: None,
                max_supply: None,
                last_updated: None,
            });

        Asset {
            id: data.id,
            name: data.name,
            symbol: data.symbol,
            slug: data.slug,
            rank: data.cmc_rank,
            logo: Some(format!(
                "https://s2.coinmarketcap.com/static/img/coins/64x64/{}.png",
                data.id
            )),
            description: None,
            category: None,
            date_added: None,
            tags: None,
            urls: None,
            quote,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // CmcResponse Tests
    // =========================================================================

    #[test]
    fn test_cmc_response_deserialization() {
        let json = r#"{
            "data": [
                {
                    "id": 1,
                    "name": "Bitcoin",
                    "symbol": "BTC",
                    "slug": "bitcoin",
                    "cmc_rank": 1
                }
            ]
        }"#;
        let response: CmcResponse<Vec<CmcListingData>> = serde_json::from_str(json).unwrap();
        assert_eq!(response.data.len(), 1);
        assert_eq!(response.data[0].symbol, "BTC");
    }

    // =========================================================================
    // CmcListingData Tests
    // =========================================================================

    #[test]
    fn test_cmc_listing_data_deserialization() {
        let json = r#"{
            "id": 1,
            "name": "Bitcoin",
            "symbol": "BTC",
            "slug": "bitcoin",
            "cmc_rank": 1,
            "quote": {
                "USD": {
                    "price": 43500.50,
                    "volume_24h": 25000000000.0,
                    "percent_change_24h": 2.5,
                    "market_cap": 850000000000.0
                }
            }
        }"#;
        let listing: CmcListingData = serde_json::from_str(json).unwrap();
        assert_eq!(listing.id, 1);
        assert_eq!(listing.name, "Bitcoin");
        assert_eq!(listing.symbol, "BTC");
        assert_eq!(listing.cmc_rank, Some(1));
        assert!(listing.quote.is_some());
    }

    #[test]
    fn test_cmc_listing_data_minimal() {
        let json = r#"{
            "id": 1027,
            "name": "Ethereum",
            "symbol": "ETH",
            "slug": "ethereum"
        }"#;
        let listing: CmcListingData = serde_json::from_str(json).unwrap();
        assert_eq!(listing.id, 1027);
        assert!(listing.cmc_rank.is_none());
        assert!(listing.quote.is_none());
    }

    // =========================================================================
    // CmcQuote Tests
    // =========================================================================

    #[test]
    fn test_cmc_quote_deserialization() {
        let json = r#"{
            "price": 43500.50,
            "volume_24h": 25000000000.0,
            "volume_change_24h": 5.2,
            "percent_change_1h": 0.5,
            "percent_change_24h": 2.5,
            "percent_change_7d": -1.2,
            "percent_change_30d": 15.0,
            "market_cap": 850000000000.0,
            "market_cap_dominance": 52.5,
            "fully_diluted_market_cap": 900000000000.0
        }"#;
        let quote: CmcQuote = serde_json::from_str(json).unwrap();
        assert_eq!(quote.price, Some(43500.50));
        assert_eq!(quote.volume_24h, Some(25000000000.0));
        assert_eq!(quote.percent_change_24h, Some(2.5));
        assert_eq!(quote.market_cap_dominance, Some(52.5));
    }

    #[test]
    fn test_cmc_quote_null_values() {
        let json = r#"{
            "price": 100.0,
            "volume_24h": null,
            "percent_change_24h": null
        }"#;
        let quote: CmcQuote = serde_json::from_str(json).unwrap();
        assert_eq!(quote.price, Some(100.0));
        assert!(quote.volume_24h.is_none());
    }

    #[test]
    fn test_cmc_quote_clone() {
        let quote = CmcQuote {
            price: Some(100.0),
            volume_24h: Some(1000000.0),
            volume_change_24h: None,
            percent_change_1h: None,
            percent_change_24h: Some(2.5),
            percent_change_7d: None,
            percent_change_30d: None,
            market_cap: Some(1000000000.0),
            market_cap_dominance: None,
            fully_diluted_market_cap: None,
        };
        let cloned = quote.clone();
        assert_eq!(cloned.price, quote.price);
    }

    // =========================================================================
    // CmcGlobalData Tests
    // =========================================================================

    #[test]
    fn test_cmc_global_data_deserialization() {
        let json = r#"{
            "btc_dominance": 52.5,
            "eth_dominance": 18.0,
            "active_cryptocurrencies": 10000,
            "active_exchanges": 500
        }"#;
        let data: CmcGlobalData = serde_json::from_str(json).unwrap();
        assert_eq!(data.btc_dominance, Some(52.5));
        assert_eq!(data.eth_dominance, Some(18.0));
        assert_eq!(data.active_cryptocurrencies, Some(10000));
    }

    // =========================================================================
    // CmcFearGreedData Tests
    // =========================================================================

    #[test]
    fn test_cmc_fear_greed_data_deserialization() {
        let json = r#"{
            "value": 25,
            "value_classification": "Extreme Fear",
            "update_time": "2024-01-15T00:00:00Z"
        }"#;
        let data: CmcFearGreedData = serde_json::from_str(json).unwrap();
        assert_eq!(data.value, 25);
        assert_eq!(data.value_classification, "Extreme Fear");
    }

    #[test]
    fn test_cmc_fear_greed_classifications() {
        let classifications = vec![
            (10, "Extreme Fear"),
            (30, "Fear"),
            (50, "Neutral"),
            (70, "Greed"),
            (90, "Extreme Greed"),
        ];
        for (value, classification) in classifications {
            let json = format!(
                r#"{{"value": {}, "value_classification": "{}", "update_time": "2024-01-15"}}"#,
                value, classification
            );
            let data: CmcFearGreedData = serde_json::from_str(&json).unwrap();
            assert_eq!(data.value, value);
            assert_eq!(data.value_classification, classification);
        }
    }

    // =========================================================================
    // CmcFearGreedResponse Tests
    // =========================================================================

    #[test]
    fn test_cmc_fear_greed_response_deserialization() {
        let json = r#"{
            "data": {
                "value": 45,
                "value_classification": "Fear",
                "update_time": "2024-01-15T12:00:00Z"
            }
        }"#;
        let response: CmcFearGreedResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.data.value, 45);
    }

    // =========================================================================
    // Constants Tests
    // =========================================================================

    #[test]
    fn test_cmc_api_url() {
        assert!(CMC_API_URL.contains("coinmarketcap.com"));
        assert!(CMC_API_URL.contains("/v1"));
    }

    #[test]
    fn test_cmc_api_v2_url() {
        assert!(CMC_API_V2_URL.contains("coinmarketcap.com"));
        assert!(CMC_API_V2_URL.contains("/v2"));
    }

    #[test]
    fn test_poll_interval() {
        // Verify poll interval is at least 1 minute (60 seconds)
        let expected_min = 60_u64;
        assert!(POLL_INTERVAL_SECS >= expected_min);
    }

    #[test]
    fn test_file_cache_ttl() {
        assert_eq!(FILE_CACHE_TTL, Duration::from_secs(24 * 60 * 60));
    }
}
