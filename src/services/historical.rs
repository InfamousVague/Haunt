//! Historical data fetching and seeding service.
//!
//! Fetches historical OHLC data from public APIs (CoinGecko) and stores
//! it in Redis for persistent chart data.

use crate::error::{AppError, Result};
use crate::services::ChartStore;
use crate::sources::{AlphaVantageClient, FinnhubClient};
use crate::sources::finnhub::{STOCK_SYMBOLS, ETF_SYMBOLS};
use dashmap::DashMap;
use redis::{aio::ConnectionManager, AsyncCommands};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Redis key prefixes for historical data
const REDIS_OHLC_PREFIX: &str = "haunt:ohlc:";
const REDIS_SEED_STATUS_PREFIX: &str = "haunt:seed:";

/// Minimum number of data points required for adequate chart data
const MIN_POINTS_1H: usize = 30;
const MIN_POINTS_1D: usize = 48;
const MIN_POINTS_1W: usize = 84;
const MIN_POINTS_1M: usize = 120;

/// OHLC data point for Redis storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OhlcDataPoint {
    pub time: i64,      // Unix timestamp in seconds
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// CoinGecko OHLC response format: [[timestamp, open, high, low, close], ...]
type CoinGeckoOhlc = Vec<[f64; 5]>;

/// CoinGecko market chart response
#[derive(Debug, Deserialize)]
struct CoinGeckoMarketChart {
    prices: Vec<[f64; 2]>,        // [[timestamp, price], ...]
    market_caps: Vec<[f64; 2]>,
    total_volumes: Vec<[f64; 2]>,
}

/// CryptoCompare histohour response
#[derive(Debug, Deserialize)]
struct CryptoCompareResponse {
    #[serde(rename = "Response")]
    response: String,
    #[serde(rename = "Data")]
    data: Option<CryptoCompareData>,
}

#[derive(Debug, Deserialize)]
struct CryptoCompareData {
    #[serde(rename = "Data")]
    data: Vec<CryptoCompareOhlc>,
}

#[derive(Debug, Deserialize)]
struct CryptoCompareOhlc {
    time: i64,
    high: f64,
    low: f64,
    open: f64,
    close: f64,
    #[serde(rename = "volumefrom")]
    volume_from: f64,
    #[serde(rename = "volumeto")]
    volume_to: f64,
}

/// Symbol to CoinGecko ID mapping (common cryptocurrencies)
fn get_coingecko_id(symbol: &str) -> Option<&'static str> {
    match symbol.to_lowercase().as_str() {
        "btc" => Some("bitcoin"),
        "eth" => Some("ethereum"),
        "bnb" => Some("binancecoin"),
        "xrp" => Some("ripple"),
        "ada" => Some("cardano"),
        "doge" => Some("dogecoin"),
        "sol" => Some("solana"),
        "dot" => Some("polkadot"),
        "matic" => Some("matic-network"),
        "ltc" => Some("litecoin"),
        "shib" => Some("shiba-inu"),
        "trx" => Some("tron"),
        "avax" => Some("avalanche-2"),
        "link" => Some("chainlink"),
        "atom" => Some("cosmos"),
        "uni" => Some("uniswap"),
        "xlm" => Some("stellar"),
        "etc" => Some("ethereum-classic"),
        "bch" => Some("bitcoin-cash"),
        "fil" => Some("filecoin"),
        "apt" => Some("aptos"),
        "arb" => Some("arbitrum"),
        "near" => Some("near"),
        "op" => Some("optimism"),
        "aave" => Some("aave"),
        "mkr" => Some("maker"),
        "crv" => Some("curve-dao-token"),
        "ldo" => Some("lido-dao"),
        "rpl" => Some("rocket-pool"),
        "snx" => Some("havven"),
        "comp" => Some("compound-governance-token"),
        "grt" => Some("the-graph"),
        "imx" => Some("immutable-x"),
        "inj" => Some("injective-protocol"),
        "ftm" => Some("fantom"),
        "algo" => Some("algorand"),
        "xtz" => Some("tezos"),
        "eos" => Some("eos"),
        "flow" => Some("flow"),
        "mina" => Some("mina-protocol"),
        "sand" => Some("the-sandbox"),
        "mana" => Some("decentraland"),
        "axs" => Some("axie-infinity"),
        "ape" => Some("apecoin"),
        "gala" => Some("gala"),
        "ilv" => Some("illuvium"),
        "ens" => Some("ethereum-name-service"),
        "blur" => Some("blur"),
        "pepe" => Some("pepe"),
        "floki" => Some("floki"),
        "wif" => Some("dogwifcoin"),
        "bonk" => Some("bonk"),
        // Privacy coins
        "xmr" => Some("monero"),
        "zec" => Some("zcash"),
        "dash" => Some("dash"),
        // Additional popular coins
        "hbar" => Some("hedera-hashgraph"),
        "vet" => Some("vechain"),
        "icp" => Some("internet-computer"),
        "qnt" => Some("quant-network"),
        "theta" => Some("theta-token"),
        "fet" => Some("fetch-ai"),
        "rndr" => Some("render-token"),
        "stx" => Some("blockstack"),
        "sei" => Some("sei-network"),
        "sui" => Some("sui"),
        "ton" => Some("the-open-network"),
        "kas" => Some("kaspa"),
        "rune" => Some("thorchain"),
        "egld" => Some("elrond-erd-2"),
        "kava" => Some("kava"),
        "neo" => Some("neo"),
        "iota" => Some("iota"),
        "xdc" => Some("xdce-crowd-sale"),
        "okb" => Some("okb"),
        "tusd" => Some("true-usd"),
        "usdc" => Some("usd-coin"),
        "usdt" => Some("tether"),
        "dai" => Some("dai"),
        "busd" => Some("binance-usd"),
        "leo" => Some("leo-token"),
        "cro" => Some("crypto-com-chain"),
        _ => None,
    }
}

/// Seeding status for a symbol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedStatus {
    NotSeeded,
    Seeding,
    Seeded,
    Failed,
}

/// Detailed seeding progress information
#[derive(Debug, Clone)]
pub struct SeedProgress {
    pub status: SeedStatus,
    pub progress: u8,         // 0-100
    pub points: u64,          // Total points fetched
    pub message: Option<String>,
}

/// Check if a symbol is a stock or ETF.
fn is_stock_or_etf(symbol: &str) -> bool {
    let upper = symbol.to_uppercase();
    STOCK_SYMBOLS.contains(&upper.as_str()) || ETF_SYMBOLS.contains(&upper.as_str())
}

/// Historical data service for fetching and storing chart data.
pub struct HistoricalDataService {
    http_client: Client,
    redis: RwLock<Option<ConnectionManager>>,
    seed_status: DashMap<String, SeedStatus>,
    seed_progress: DashMap<String, SeedProgress>,
    chart_store: Arc<ChartStore>,
    coingecko_api_key: Option<String>,
    cryptocompare_api_key: Option<String>,
    alphavantage_client: Option<Arc<AlphaVantageClient>>,
}

impl HistoricalDataService {
    /// Create a new historical data service.
    pub fn new(
        chart_store: Arc<ChartStore>,
        coingecko_api_key: Option<String>,
        cryptocompare_api_key: Option<String>,
        alphavantage_client: Option<Arc<AlphaVantageClient>>,
    ) -> Arc<Self> {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Haunt/1.0 (cryptocurrency price aggregator; https://github.com/haunt)")
            .build()
            .expect("Failed to create HTTP client");

        Arc::new(Self {
            http_client,
            redis: RwLock::new(None),
            seed_status: DashMap::new(),
            seed_progress: DashMap::new(),
            chart_store,
            coingecko_api_key,
            cryptocompare_api_key,
            alphavantage_client,
        })
    }

    /// Connect to Redis.
    pub async fn connect_redis(&self, redis_url: &str) {
        match redis::Client::open(redis_url) {
            Ok(client) => {
                match ConnectionManager::new(client).await {
                    Ok(conn) => {
                        info!("HistoricalDataService connected to Redis");
                        *self.redis.write().await = Some(conn);
                    }
                    Err(e) => {
                        warn!("Failed to connect HistoricalDataService to Redis: {}", e);
                    }
                }
            }
            Err(e) => {
                warn!("Invalid Redis URL for HistoricalDataService: {}", e);
            }
        }
    }

    /// Load historical data from Redis for common symbols on startup.
    /// This ensures charts have data immediately after server restart.
    pub async fn load_common_symbols(&self) {
        let common_symbols = vec![
            "btc", "eth", "bnb", "xrp", "ada", "doge", "sol", "dot", "matic", "ltc",
            "shib", "trx", "avax", "link", "atom", "uni", "xlm", "etc", "bch", "fil",
            "apt", "arb", "near", "op", "aave", "mkr", "crv", "ldo", "snx", "comp",
        ];

        let mut loaded_count = 0;
        for symbol in common_symbols {
            if self.load_from_redis(symbol).await {
                self.seed_status.insert(symbol.to_string(), SeedStatus::Seeded);
                loaded_count += 1;
            }
        }

        if loaded_count > 0 {
            info!("Loaded historical data for {} common symbols from Redis", loaded_count);
        }
    }

    /// Check if a symbol has adequate chart data for a given range.
    pub fn has_adequate_data(&self, symbol: &str, range: &str) -> bool {
        let sparkline = self.chart_store.get_sparkline(symbol, 500);
        let min_points = match range {
            "1h" => MIN_POINTS_1H,
            "4h" => MIN_POINTS_1H * 2,
            "1d" => MIN_POINTS_1D,
            "1w" => MIN_POINTS_1W,
            "1m" => MIN_POINTS_1M,
            _ => MIN_POINTS_1D,
        };
        sparkline.len() >= min_points
    }

    /// Get the seeding status for a symbol.
    pub fn get_seed_status(&self, symbol: &str) -> SeedStatus {
        self.seed_status
            .get(&symbol.to_lowercase())
            .map(|s| *s)
            .unwrap_or(SeedStatus::NotSeeded)
    }

    /// Get the detailed seeding progress for a symbol.
    pub fn get_seed_progress(&self, symbol: &str) -> Option<SeedProgress> {
        self.seed_progress
            .get(&symbol.to_lowercase())
            .map(|p| p.clone())
    }

    /// Update the seeding progress for a symbol.
    fn update_progress(&self, symbol: &str, progress: u8, points: u64, message: Option<String>) {
        let status = self.get_seed_status(symbol);
        self.seed_progress.insert(
            symbol.to_lowercase(),
            SeedProgress {
                status,
                progress,
                points,
                message,
            },
        );
    }

    /// Check if historical data should be seeded for a symbol.
    /// Returns true if seeding should be triggered.
    pub async fn should_seed(&self, symbol: &str, range: &str) -> bool {
        let symbol_lower = symbol.to_lowercase();

        // Check if already seeding or seeded
        let status = self.get_seed_status(&symbol_lower);
        if status == SeedStatus::Seeding || status == SeedStatus::Seeded {
            return false;
        }

        // Check if we have adequate in-memory data
        if self.has_adequate_data(symbol, range) {
            return false;
        }

        // Check if we have data in Redis
        if self.load_from_redis(&symbol_lower).await {
            // Data loaded from Redis, check again
            if self.has_adequate_data(symbol, range) {
                self.seed_status.insert(symbol_lower.clone(), SeedStatus::Seeded);
                return false;
            }
        }

        true
    }

    /// Load historical OHLC data from Redis into the chart store.
    pub async fn load_from_redis(&self, symbol: &str) -> bool {
        let conn_guard = self.redis.read().await;
        let Some(ref conn) = *conn_guard else {
            return false;
        };

        let mut conn = conn.clone();
        let key = format!("{}{}:1h", REDIS_OHLC_PREFIX, symbol.to_lowercase());

        // Get all OHLC data from sorted set
        let result: std::result::Result<Vec<(String, f64)>, redis::RedisError> = redis::cmd("ZRANGEBYSCORE")
            .arg(&key)
            .arg("-inf")
            .arg("+inf")
            .arg("WITHSCORES")
            .query_async(&mut conn)
            .await;

        match result {
            Ok(data) if !data.is_empty() => {
                debug!("Loading {} historical points for {} from Redis", data.len(), symbol);

                for (json_str, _score) in &data {
                    if let Ok(point) = serde_json::from_str::<OhlcDataPoint>(json_str) {
                        // Convert to milliseconds for chart store
                        let timestamp_ms = point.time * 1000;
                        self.chart_store.add_price(
                            symbol,
                            point.close,
                            Some(point.volume),
                            timestamp_ms,
                        );
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// Save OHLC data to Redis.
    async fn save_to_redis(&self, symbol: &str, data: &[OhlcDataPoint]) -> Result<()> {
        let conn_guard = self.redis.read().await;
        let Some(ref conn) = *conn_guard else {
            return Ok(()); // No Redis, skip saving
        };

        let mut conn = conn.clone();
        let key = format!("{}{}:1h", REDIS_OHLC_PREFIX, symbol.to_lowercase());

        // Use pipeline for batch insert
        let mut pipe = redis::pipe();

        for point in data {
            let json = serde_json::to_string(point)
                .map_err(|e| AppError::Internal(format!("JSON serialize error: {}", e)))?;

            // Use timestamp as score for sorted set
            pipe.zadd(&key, &json, point.time as f64);
        }

        // Set TTL of 90 days
        pipe.expire(&key, 90 * 24 * 60 * 60);

        pipe.query_async::<_, ()>(&mut conn)
            .await
            .map_err(|e| AppError::Internal(format!("Redis pipeline error: {}", e)))?;

        info!("Saved {} historical OHLC points for {} to Redis", data.len(), symbol);
        Ok(())
    }

    /// Seed historical data for a symbol from multiple sources.
    /// For crypto: CoinGecko + CryptoCompare
    /// For stocks/ETFs: Alpha Vantage
    /// This is meant to be called as a background task.
    pub async fn seed_historical_data(self: Arc<Self>, symbol: String) {
        let symbol_lower = symbol.to_lowercase();
        let symbol_upper = symbol.to_uppercase();

        // Mark as seeding and initialize progress
        self.seed_status.insert(symbol_lower.clone(), SeedStatus::Seeding);
        self.update_progress(&symbol_lower, 0, 0, Some("Starting data fetch...".to_string()));

        info!("Starting historical data seed for {}", symbol);

        // Check if this is a stock/ETF - use different data source
        if is_stock_or_etf(&symbol_upper) {
            self.seed_stock_historical_data(&symbol_lower, &symbol_upper).await;
            return;
        }

        let mut all_points: Vec<OhlcDataPoint> = Vec::new();
        let mut coingecko_succeeded = false;

        // === Source 1: CoinGecko ===
        if let Some(coingecko_id) = get_coingecko_id(&symbol_lower) {
            self.update_progress(&symbol_lower, 10, 0, Some("Fetching from CoinGecko...".to_string()));

            for (i, days) in [1, 7, 30, 90].iter().enumerate() {
                match self.fetch_coingecko_market_chart(coingecko_id, *days).await {
                    Ok(points) => {
                        info!("[CoinGecko] Fetched {} points for {} ({} days)", points.len(), symbol, days);
                        all_points.extend(points);
                        coingecko_succeeded = true;
                        // Progress: 10-50% during CoinGecko fetches
                        let progress = 10 + ((i + 1) * 10) as u8;
                        self.update_progress(&symbol_lower, progress, all_points.len() as u64, None);
                    }
                    Err(e) => {
                        warn!("[CoinGecko] Failed to fetch {} day data for {}: {}", days, symbol, e);
                    }
                }
                // Small delay to avoid rate limiting
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        } else {
            debug!("No CoinGecko ID mapping for symbol: {}, trying CryptoCompare only", symbol);
        }

        // === Source 2: CryptoCompare (supplement or fallback) ===
        self.update_progress(&symbol_lower, 60, all_points.len() as u64, Some("Fetching from CryptoCompare...".to_string()));

        // Fetch hourly data (up to 2000 hours = ~83 days)
        match self.fetch_cryptocompare_histohour(&symbol_lower, 2000).await {
            Ok(points) => {
                info!("[CryptoCompare] Fetched {} hourly points for {}", points.len(), symbol);
                all_points.extend(points);
                self.update_progress(&symbol_lower, 75, all_points.len() as u64, None);
            }
            Err(e) => {
                if !coingecko_succeeded {
                    warn!("[CryptoCompare] Failed to fetch hourly data for {}: {}", symbol, e);
                } else {
                    debug!("[CryptoCompare] Hourly fetch failed (using CoinGecko data): {}", e);
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;

        // Fetch daily data for longer history (up to 365 days)
        match self.fetch_cryptocompare_histoday(&symbol_lower, 365).await {
            Ok(points) => {
                info!("[CryptoCompare] Fetched {} daily points for {}", points.len(), symbol);
                // Convert daily points to hourly timestamps (use closing time)
                // These fill in gaps for longer time ranges
                all_points.extend(points);
                self.update_progress(&symbol_lower, 85, all_points.len() as u64, Some("Processing data...".to_string()));
            }
            Err(e) => {
                debug!("[CryptoCompare] Daily fetch failed: {}", e);
            }
        }

        if all_points.is_empty() {
            warn!("No historical data fetched for {} from any source", symbol);
            self.seed_status.insert(symbol_lower.clone(), SeedStatus::Failed);
            self.update_progress(&symbol_lower, 0, 0, Some("Failed to fetch data".to_string()));
            return;
        }

        // Sort by timestamp and deduplicate
        // When there are duplicate timestamps, prefer the point with more volume data
        all_points.sort_by_key(|p| p.time);

        // Deduplicate with preference for non-zero volume
        let mut deduped: Vec<OhlcDataPoint> = Vec::new();
        for point in all_points {
            if let Some(last) = deduped.last_mut() {
                if last.time == point.time {
                    // Same timestamp - merge, preferring non-zero values
                    if last.volume == 0.0 && point.volume > 0.0 {
                        last.volume = point.volume;
                    }
                    // Average the OHLC values for better accuracy
                    last.open = (last.open + point.open) / 2.0;
                    last.high = last.high.max(point.high);
                    last.low = last.low.min(point.low);
                    last.close = (last.close + point.close) / 2.0;
                    continue;
                }
            }
            deduped.push(point);
        }

        info!("Aggregated {} unique historical data points for {} from multiple sources", deduped.len(), symbol);

        // Add to chart store
        for point in &deduped {
            let timestamp_ms = point.time * 1000;
            self.chart_store.add_price(
                &symbol_lower,
                point.close,
                Some(point.volume),
                timestamp_ms,
            );
        }

        // Save to Redis for persistence
        if let Err(e) = self.save_to_redis(&symbol_lower, &deduped).await {
            error!("Failed to save historical data to Redis: {}", e);
        }

        self.seed_status.insert(symbol_lower.clone(), SeedStatus::Seeded);
        self.update_progress(&symbol_lower, 100, deduped.len() as u64, Some("Complete".to_string()));
        info!("Completed historical data seed for {}", symbol);
    }

    /// Fetch market chart data from CoinGecko and convert to OHLC points.
    async fn fetch_coingecko_market_chart(&self, coin_id: &str, days: u32) -> Result<Vec<OhlcDataPoint>> {
        // CoinGecko free API - market_chart endpoint gives price/volume history
        let url = format!(
            "https://api.coingecko.com/api/v3/coins/{}/market_chart?vs_currency=usd&days={}",
            coin_id, days
        );

        debug!("Fetching CoinGecko market chart: {} days for {}", days, coin_id);

        let mut request = self.http_client.get(&url);

        // Add API key if available (for higher rate limits)
        if let Some(ref key) = self.coingecko_api_key {
            request = request.header("x-cg-demo-api-key", key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| AppError::ExternalApi(format!("CoinGecko request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalApi(format!(
                "CoinGecko API error {}: {}", status, body
            )));
        }

        let chart: CoinGeckoMarketChart = response
            .json()
            .await
            .map_err(|e| AppError::ExternalApi(format!("Failed to parse CoinGecko response: {}", e)))?;

        // Convert price/volume data to OHLC-like points
        // Group by hour for hourly resolution
        let mut hourly_data: std::collections::BTreeMap<i64, OhlcDataPoint> = std::collections::BTreeMap::new();

        for (i, price_point) in chart.prices.iter().enumerate() {
            let timestamp_ms = price_point[0] as i64;
            let price = price_point[1];

            // Get corresponding volume if available
            let volume = chart.total_volumes
                .get(i)
                .map(|v| v[1])
                .unwrap_or(0.0);

            // Round to hour
            let hour_ts = (timestamp_ms / 1000 / 3600) * 3600;

            hourly_data
                .entry(hour_ts)
                .and_modify(|ohlc| {
                    ohlc.high = ohlc.high.max(price);
                    ohlc.low = ohlc.low.min(price);
                    ohlc.close = price;
                    ohlc.volume += volume;
                })
                .or_insert(OhlcDataPoint {
                    time: hour_ts,
                    open: price,
                    high: price,
                    low: price,
                    close: price,
                    volume,
                });
        }

        Ok(hourly_data.into_values().collect())
    }

    /// Fetch OHLC data directly from CoinGecko (limited days available).
    #[allow(dead_code)]
    async fn fetch_coingecko_ohlc(&self, coin_id: &str, days: u32) -> Result<Vec<OhlcDataPoint>> {
        // CoinGecko OHLC endpoint - limited to 1/7/14/30/90/180/365 days
        let valid_days = match days {
            d if d <= 1 => 1,
            d if d <= 7 => 7,
            d if d <= 14 => 14,
            d if d <= 30 => 30,
            d if d <= 90 => 90,
            d if d <= 180 => 180,
            _ => 365,
        };

        let url = format!(
            "https://api.coingecko.com/api/v3/coins/{}/ohlc?vs_currency=usd&days={}",
            coin_id, valid_days
        );

        debug!("Fetching CoinGecko OHLC: {} days for {}", valid_days, coin_id);

        let mut request = self.http_client.get(&url);

        if let Some(ref key) = self.coingecko_api_key {
            request = request.header("x-cg-demo-api-key", key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| AppError::ExternalApi(format!("CoinGecko request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(AppError::ExternalApi(format!("CoinGecko API error: {}", status)));
        }

        let ohlc: CoinGeckoOhlc = response
            .json()
            .await
            .map_err(|e| AppError::ExternalApi(format!("Failed to parse OHLC: {}", e)))?;

        let points: Vec<OhlcDataPoint> = ohlc
            .into_iter()
            .map(|candle| OhlcDataPoint {
                time: (candle[0] / 1000.0) as i64,
                open: candle[1],
                high: candle[2],
                low: candle[3],
                close: candle[4],
                volume: 0.0, // OHLC endpoint doesn't include volume
            })
            .collect();

        Ok(points)
    }

    /// Fetch hourly OHLC data from CryptoCompare.
    /// CryptoCompare provides up to 2000 hourly data points per request.
    async fn fetch_cryptocompare_histohour(&self, symbol: &str, limit: u32) -> Result<Vec<OhlcDataPoint>> {
        // CryptoCompare uses uppercase symbols directly
        let fsym = symbol.to_uppercase();
        let url = format!(
            "https://min-api.cryptocompare.com/data/v2/histohour?fsym={}&tsym=USD&limit={}",
            fsym, limit.min(2000)
        );

        debug!("Fetching CryptoCompare histohour: {} hours for {}", limit, symbol);

        let mut request = self.http_client.get(&url);

        // Add API key if available (for higher rate limits)
        if let Some(ref key) = self.cryptocompare_api_key {
            request = request.header("authorization", format!("Apikey {}", key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| AppError::ExternalApi(format!("CryptoCompare request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::ExternalApi(format!(
                "CryptoCompare API error {}: {}", status, body
            )));
        }

        let resp: CryptoCompareResponse = response
            .json()
            .await
            .map_err(|e| AppError::ExternalApi(format!("Failed to parse CryptoCompare response: {}", e)))?;

        if resp.response != "Success" {
            return Err(AppError::ExternalApi(format!(
                "CryptoCompare API returned error: {}", resp.response
            )));
        }

        let data = resp.data
            .ok_or_else(|| AppError::ExternalApi("CryptoCompare returned no data".to_string()))?;

        let points: Vec<OhlcDataPoint> = data.data
            .into_iter()
            .filter(|ohlc| ohlc.close > 0.0) // Filter out zero-price entries
            .map(|ohlc| OhlcDataPoint {
                time: ohlc.time,
                open: ohlc.open,
                high: ohlc.high,
                low: ohlc.low,
                close: ohlc.close,
                volume: ohlc.volume_to, // volume_to is in USD
            })
            .collect();

        Ok(points)
    }

    /// Fetch daily OHLC data from CryptoCompare for longer time ranges.
    async fn fetch_cryptocompare_histoday(&self, symbol: &str, limit: u32) -> Result<Vec<OhlcDataPoint>> {
        let fsym = symbol.to_uppercase();
        let url = format!(
            "https://min-api.cryptocompare.com/data/v2/histoday?fsym={}&tsym=USD&limit={}",
            fsym, limit.min(2000)
        );

        debug!("Fetching CryptoCompare histoday: {} days for {}", limit, symbol);

        let mut request = self.http_client.get(&url);

        if let Some(ref key) = self.cryptocompare_api_key {
            request = request.header("authorization", format!("Apikey {}", key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| AppError::ExternalApi(format!("CryptoCompare request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(AppError::ExternalApi(format!("CryptoCompare API error: {}", status)));
        }

        let resp: CryptoCompareResponse = response
            .json()
            .await
            .map_err(|e| AppError::ExternalApi(format!("Failed to parse CryptoCompare response: {}", e)))?;

        if resp.response != "Success" {
            return Err(AppError::ExternalApi("CryptoCompare API returned error".to_string()));
        }

        let data = resp.data
            .ok_or_else(|| AppError::ExternalApi("CryptoCompare returned no data".to_string()))?;

        let points: Vec<OhlcDataPoint> = data.data
            .into_iter()
            .filter(|ohlc| ohlc.close > 0.0)
            .map(|ohlc| OhlcDataPoint {
                time: ohlc.time,
                open: ohlc.open,
                high: ohlc.high,
                low: ohlc.low,
                close: ohlc.close,
                volume: ohlc.volume_to,
            })
            .collect();

        Ok(points)
    }

    /// Seed data for multiple symbols concurrently with rate limiting.
    pub async fn seed_multiple(self: Arc<Self>, symbols: Vec<String>) {
        info!("Starting batch historical data seed for {} symbols", symbols.len());

        for (i, symbol) in symbols.into_iter().enumerate() {
            // Rate limit: CoinGecko free tier allows ~10-30 calls/minute
            if i > 0 && i % 5 == 0 {
                tokio::time::sleep(Duration::from_secs(15)).await;
            }

            let service = self.clone();
            tokio::spawn(async move {
                service.seed_historical_data(symbol).await;
            });

            // Small delay between spawns
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// Seed historical data for stocks/ETFs using Alpha Vantage.
    async fn seed_stock_historical_data(&self, symbol_lower: &str, symbol_upper: &str) {
        let Some(ref av_client) = self.alphavantage_client else {
            warn!("Alpha Vantage client not configured, cannot seed stock data for {}", symbol_upper);
            self.seed_status.insert(symbol_lower.to_string(), SeedStatus::Failed);
            self.update_progress(symbol_lower, 0, 0, Some("Alpha Vantage not configured".to_string()));
            return;
        };

        self.update_progress(symbol_lower, 10, 0, Some("Fetching from Alpha Vantage...".to_string()));

        // Fetch daily time series (compact = last 100 trading days)
        match av_client.get_daily_time_series(symbol_upper, "compact").await {
            Ok(points) => {
                info!("[AlphaVantage] Fetched {} daily points for {}", points.len(), symbol_upper);

                if points.is_empty() {
                    warn!("No historical data from Alpha Vantage for {}", symbol_upper);
                    self.seed_status.insert(symbol_lower.to_string(), SeedStatus::Failed);
                    self.update_progress(symbol_lower, 0, 0, Some("No data available".to_string()));
                    return;
                }

                self.update_progress(symbol_lower, 50, points.len() as u64, Some("Processing data...".to_string()));

                // Convert to OhlcDataPoint and add to chart store
                let ohlc_points: Vec<OhlcDataPoint> = points
                    .into_iter()
                    .map(|p| OhlcDataPoint {
                        time: p.time / 1000, // Convert from ms to seconds
                        open: p.open,
                        high: p.high,
                        low: p.low,
                        close: p.close,
                        volume: p.volume,
                    })
                    .collect();

                // Add to chart store
                for point in &ohlc_points {
                    let timestamp_ms = point.time * 1000;
                    self.chart_store.add_price(
                        symbol_lower,
                        point.close,
                        Some(point.volume),
                        timestamp_ms,
                    );
                }

                // Save to Redis
                if let Err(e) = self.save_to_redis(symbol_lower, &ohlc_points).await {
                    error!("Failed to save stock historical data to Redis: {}", e);
                }

                self.seed_status.insert(symbol_lower.to_string(), SeedStatus::Seeded);
                self.update_progress(symbol_lower, 100, ohlc_points.len() as u64, Some("Complete".to_string()));
                info!("Completed historical data seed for {} ({} points)", symbol_upper, ohlc_points.len());
            }
            Err(e) => {
                error!("[AlphaVantage] Failed to fetch data for {}: {}", symbol_upper, e);
                self.seed_status.insert(symbol_lower.to_string(), SeedStatus::Failed);
                self.update_progress(symbol_lower, 0, 0, Some(format!("Failed: {}", e)));
            }
        }
    }

    /// Load common stock symbols from Redis on startup.
    pub async fn load_stock_symbols(&self) {
        let mut loaded_count = 0;

        for symbol in STOCK_SYMBOLS.iter().chain(ETF_SYMBOLS.iter()) {
            let symbol_lower = symbol.to_lowercase();
            if self.load_from_redis(&symbol_lower).await {
                self.seed_status.insert(symbol_lower, SeedStatus::Seeded);
                loaded_count += 1;
            }
        }

        if loaded_count > 0 {
            info!("Loaded historical data for {} stock/ETF symbols from Redis", loaded_count);
        }
    }
}
