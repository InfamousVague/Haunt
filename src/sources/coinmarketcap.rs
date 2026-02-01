use crate::services::{Cache, ChartStore, PriceCache};
use crate::types::{Asset, AssetListing, FearGreedData, GlobalMetrics, PaginatedResponse, PriceSource, Quote, SearchResult};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info};

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
}

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
        }
    }

    /// Start polling for price updates.
    pub async fn start_polling(&self) {
        info!("Starting CoinMarketCap price polling");

        loop {
            if let Err(e) = self.fetch_listings().await {
                error!("CoinMarketCap fetch error: {}", e);
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
        }
    }

    /// Fetch paginated listings.
    pub async fn get_listings(&self, page: i32, limit: i32) -> anyhow::Result<PaginatedResponse<AssetListing>> {
        let cache_key = format!("listings_{}_{}", page, limit);

        // Check cache first - but still populate sparklines
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

        let response: CmcResponse<Vec<CmcListingData>> = self.client
            .get(&url)
            .header("X-CMC_PRO_API_KEY", &self.api_key)
            .send()
            .await?
            .json()
            .await?;

        let mut listings: Vec<AssetListing> = response.data
            .into_iter()
            .map(|d| self.convert_listing(d))
            .collect();

        // Populate sparklines from chart store
        for listing in &mut listings {
            listing.sparkline = self.chart_store.get_sparkline(&listing.symbol, 60);
        }

        self.listings_cache.set(cache_key, listings.clone());

        Ok(PaginatedResponse {
            data: listings,
            page,
            limit,
            total: 10000,
            has_more: true,
        })
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

        let resp = self.client
            .get(&url)
            .header("X-CMC_PRO_API_KEY", &self.api_key)
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() {
            tracing::warn!("CMC API error for asset {}: {} - {}", id, status, &text[..text.len().min(200)]);
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

        debug!("CMC get_asset {} response keys: {:?}", id, parsed.as_object().map(|o| o.keys().collect::<Vec<_>>()));

        let data_obj = parsed.get("data");
        if data_obj.is_none() {
            tracing::warn!("No 'data' field in CMC response for asset {}", id);
            return Ok(None);
        }

        let asset_data = data_obj.and_then(|d| d.get(&id.to_string()));
        if asset_data.is_none() {
            tracing::warn!("Asset {} not found in CMC data. Available keys: {:?}",
                id,
                data_obj.and_then(|d| d.as_object().map(|o| o.keys().collect::<Vec<_>>())));
            return Ok(None);
        }

        let asset = asset_data
            .and_then(|v| {
                match serde_json::from_value::<CmcListingData>(v.clone()) {
                    Ok(data) => Some(data),
                    Err(e) => {
                        tracing::warn!("Failed to parse asset data for {}: {}", id, e);
                        None
                    }
                }
            })
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
        let results: Vec<AssetListing> = listings.data
            .into_iter()
            .filter(|l| {
                l.name.to_lowercase().contains(&query_lower) ||
                l.symbol.to_lowercase().contains(&query_lower)
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

        let resp = self.client
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
            btc_dominance: data.get("btc_dominance").and_then(|v| v.as_f64()).unwrap_or(0.0),
            eth_dominance: data.get("eth_dominance").and_then(|v| v.as_f64()).unwrap_or(0.0),
            active_cryptocurrencies: data.get("active_cryptocurrencies").and_then(|v| v.as_i64()).map(|v| v as i32).unwrap_or(0),
            active_exchanges: data.get("active_exchanges").and_then(|v| v.as_i64()).map(|v| v as i32).unwrap_or(0),
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
            last_updated: data.get("last_updated").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
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

        let resp = self.client
            .get(url)
            .send()
            .await?;

        let text = resp.text().await?;
        let parsed: serde_json::Value = serde_json::from_str(&text)?;

        // alternative.me response format: { "data": [{ "value": "25", "value_classification": "Extreme Fear", "timestamp": "..." }] }
        let data_arr = parsed.get("data").and_then(|d| d.as_array());

        let data = if let Some(arr) = data_arr {
            if let Some(item) = arr.first() {
                FearGreedData {
                    value: item.get("value")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<i32>().ok())
                        .unwrap_or(50),
                    classification: item.get("value_classification")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Neutral")
                        .to_string(),
                    timestamp: item.get("timestamp")
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

        self.fear_greed_cache.set("fear_greed".to_string(), data.clone());

        Ok(data)
    }

    async fn fetch_listings(&self) -> anyhow::Result<()> {
        let url = format!(
            "{}/cryptocurrency/listings/latest?start=1&limit=100&convert=USD",
            CMC_API_URL
        );

        let response: CmcResponse<Vec<CmcListingData>> = self.client
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
                    self.chart_store.add_price(&symbol, price, quote.volume_24h, timestamp);
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

        AssetListing {
            id: data.id,
            rank: data.cmc_rank.unwrap_or(0),
            name: data.name,
            symbol: data.symbol,
            image: format!("https://s2.coinmarketcap.com/static/img/coins/64x64/{}.png", data.id),
            price: safe_f64(quote.as_ref().and_then(|q| q.price)),
            change_1h: safe_f64(quote.as_ref().and_then(|q| q.percent_change_1h)),
            change_24h: safe_f64(quote.as_ref().and_then(|q| q.percent_change_24h)),
            change_7d: safe_f64(quote.as_ref().and_then(|q| q.percent_change_7d)),
            market_cap: safe_f64(quote.as_ref().and_then(|q| q.market_cap)),
            volume_24h: safe_f64(quote.as_ref().and_then(|q| q.volume_24h)),
            circulating_supply: 0.0, // CMC listings endpoint doesn't include this
            max_supply: None,
            sparkline: vec![], // Populated after by get_listings
        }
    }

    fn convert_to_asset(&self, data: CmcListingData) -> Asset {
        let quote = data.quote.and_then(|m| m.get("USD").cloned()).map(|q| Quote {
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
            logo: Some(format!("https://s2.coinmarketcap.com/static/img/coins/64x64/{}.png", data.id)),
            description: None,
            category: None,
            date_added: None,
            tags: None,
            urls: None,
            quote,
        }
    }
}
