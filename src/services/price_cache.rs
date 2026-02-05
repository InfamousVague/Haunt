use crate::types::{AggregatedPrice, AggregationConfig, PriceSource, SourcePrice, TradeDirection};
use dashmap::DashMap;
use redis::aio::ConnectionManager;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info, warn};

const REDIS_PRICE_PREFIX: &str = "haunt:price:";
const REDIS_UPDATE_COUNT_KEY: &str = "haunt:stats:update_count";
const REDIS_SOURCE_COUNTS_KEY: &str = "haunt:stats:source_counts";

/// Cached volume with source tracking.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct CachedVolume {
    value: f64,
    source: PriceSource,
}

/// Cached price data for a symbol.
#[derive(Debug, Clone)]
struct SymbolPrice {
    /// Prices from each source.
    sources: Vec<SourcePrice>,
    /// Last price per source (for change detection).
    last_source_prices: HashMap<PriceSource, f64>,
    /// Last aggregated price.
    last_aggregated: Option<f64>,
    /// Last update time for throttling.
    last_update_time: Instant,
    /// Cached volume from authoritative source.
    cached_volume: Option<CachedVolume>,
    /// Last trade direction (up/down based on price movement).
    trade_direction: Option<TradeDirection>,
}

impl Default for SymbolPrice {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            last_source_prices: HashMap::new(),
            last_aggregated: None,
            last_update_time: Instant::now(),
            cached_volume: None,
            trade_direction: None,
        }
    }
}

/// Status tracking for a source.
#[derive(Debug, Default)]
struct SourceStatus {
    /// Whether the source is currently online.
    online: AtomicBool,
    /// Last successful update timestamp (unix ms).
    last_success: AtomicU64,
}

/// TPS window for calculating transactions per second (last 60 seconds).
const TPS_WINDOW_SECS: u64 = 60;

/// Multi-source price aggregation cache.
pub struct PriceCache {
    /// Price data keyed by symbol.
    prices: DashMap<String, SymbolPrice>,
    /// Aggregation configuration.
    config: AggregationConfig,
    /// Broadcast channel for price updates.
    tx: broadcast::Sender<AggregatedPrice>,
    /// Redis connection for persistence.
    redis: Arc<RwLock<Option<ConnectionManager>>>,
    /// Total update count across all sources.
    total_updates: AtomicU64,
    /// Update counts per source.
    source_updates: DashMap<PriceSource, AtomicU64>,
    /// Update counts per symbol per source (for per-asset source breakdown).
    symbol_source_updates: DashMap<String, DashMap<PriceSource, AtomicU64>>,
    /// Status tracking per source.
    source_status: DashMap<PriceSource, SourceStatus>,
    /// Last error per source (separate DashMap for simpler borrowing).
    source_errors: DashMap<PriceSource, String>,
    /// Start time for uptime calculation.
    start_time: Instant,
    /// Recent update timestamps for TPS calculation (last 60 seconds).
    recent_updates: Mutex<VecDeque<Instant>>,
}

impl PriceCache {
    /// Create a new price cache.
    pub fn new(config: AggregationConfig) -> (Arc<Self>, broadcast::Receiver<AggregatedPrice>) {
        let (tx, rx) = broadcast::channel(4096);
        let cache = Arc::new(Self {
            prices: DashMap::new(),
            config,
            tx,
            redis: Arc::new(RwLock::new(None)),
            total_updates: AtomicU64::new(0),
            source_updates: DashMap::new(),
            symbol_source_updates: DashMap::new(),
            source_status: DashMap::new(),
            source_errors: DashMap::new(),
            start_time: Instant::now(),
            recent_updates: Mutex::new(VecDeque::with_capacity(10000)),
        });
        (cache, rx)
    }

    /// Connect to Redis for persistence.
    pub async fn connect_redis(&self, redis_url: &str) {
        match redis::Client::open(redis_url) {
            Ok(client) => match ConnectionManager::new(client).await {
                Ok(conn) => {
                    info!("PriceCache connected to Redis at {}", redis_url);
                    *self.redis.write().await = Some(conn);
                }
                Err(e) => {
                    warn!("Failed to connect PriceCache to Redis: {}", e);
                }
            },
            Err(e) => {
                warn!("Invalid Redis URL for PriceCache: {}", e);
            }
        }
    }

    /// Load prices from Redis.
    pub async fn load_from_redis(&self, symbols: &[&str]) {
        let conn_guard = self.redis.read().await;
        let Some(ref conn) = *conn_guard else { return };

        let mut conn = conn.clone();
        let mut loaded = 0;

        for symbol in symbols {
            let key = format!("{}{}", REDIS_PRICE_PREFIX, symbol.to_lowercase());
            let result: Result<Option<String>, _> =
                redis::cmd("GET").arg(&key).query_async(&mut conn).await;

            if let Ok(Some(data)) = result {
                if let Ok(price_data) = serde_json::from_str::<SymbolPriceData>(&data) {
                    let mut entry = self.prices.entry(symbol.to_lowercase()).or_default();
                    entry.last_aggregated = Some(price_data.price);
                    for (source, price) in price_data.sources {
                        entry.last_source_prices.insert(source, price);
                    }
                    loaded += 1;
                }
            }
        }

        if loaded > 0 {
            info!("Loaded {} prices from Redis", loaded);
        }
    }

    /// Subscribe to price updates.
    pub fn subscribe(&self) -> broadcast::Receiver<AggregatedPrice> {
        self.tx.subscribe()
    }

    /// Update a price from a source.
    pub fn update_price(
        &self,
        symbol: &str,
        source: PriceSource,
        price: f64,
        volume_24h: Option<f64>,
    ) {
        let now = Instant::now();
        let timestamp = chrono::Utc::now().timestamp_millis();
        let symbol_lower = symbol.to_lowercase();

        let mut entry = self.prices.entry(symbol_lower.clone()).or_default();
        let symbol_price = entry.value_mut();

        // Check if this source's price actually changed
        let last_source_price = symbol_price.last_source_prices.get(&source).copied();
        let source_price_changed = match last_source_price {
            Some(last) => (price - last).abs() > 0.0001, // Any meaningful change
            None => true,                                // New source
        };

        // Update source price tracking
        symbol_price.last_source_prices.insert(source, price);

        // Only accept volume from authoritative sources (CoinMarketCap, CoinGecko)
        // Individual exchanges only report their own volume, not market-wide 24h volume
        if let Some(vol) = volume_24h {
            if source.is_volume_authoritative() {
                symbol_price.cached_volume = Some(CachedVolume { value: vol, source });
            }
        }

        // Update or add source price
        let source_price = SourcePrice {
            source,
            price,
            timestamp,
            volume_24h,
        };

        if let Some(existing) = symbol_price.sources.iter_mut().find(|s| s.source == source) {
            *existing = source_price;
        } else {
            symbol_price.sources.push(source_price);
        }

        // Remove stale sources
        let stale_threshold = timestamp - self.config.stale_threshold_ms as i64;
        symbol_price
            .sources
            .retain(|s| s.timestamp > stale_threshold);

        // If this source's price didn't change, skip broadcasting
        if !source_price_changed {
            return;
        }

        // Check throttle (per-symbol, not per-source)
        let elapsed_ms = now
            .duration_since(symbol_price.last_update_time)
            .as_millis() as u64;
        if elapsed_ms < self.config.throttle_ms {
            return;
        }

        // Calculate weighted average price
        let aggregated = self.aggregate(&symbol_price.sources);

        // Get primary source (highest weight)
        let primary_source = symbol_price
            .sources
            .iter()
            .max_by_key(|s| s.source.weight())
            .map(|s| s.source)
            .unwrap_or(source);

        let sources: Vec<PriceSource> = symbol_price.sources.iter().map(|s| s.source).collect();
        let previous_price = symbol_price.last_aggregated;

        // Use cached authoritative volume instead of the current update's volume
        let authoritative_volume = symbol_price.cached_volume.as_ref().map(|v| v.value);

        // Calculate trade direction based on price movement
        let trade_direction = match previous_price {
            Some(prev) if aggregated > prev => Some(TradeDirection::Up),
            Some(prev) if aggregated < prev => Some(TradeDirection::Down),
            _ => symbol_price.trade_direction, // Keep previous direction if price unchanged
        };
        symbol_price.trade_direction = trade_direction;

        let update = AggregatedPrice {
            id: symbol_lower.clone(),
            symbol: symbol_lower.clone(),
            price: aggregated,
            previous_price,
            change_24h: None,
            volume_24h: authoritative_volume,
            trade_direction,
            source: primary_source,
            sources,
            timestamp,
        };

        symbol_price.last_aggregated = Some(aggregated);
        symbol_price.last_update_time = now;

        // Clone data for async Redis save
        let source_prices = symbol_price.last_source_prices.clone();
        drop(entry);

        // Increment update counters
        self.total_updates.fetch_add(1, Ordering::Relaxed);
        self.source_updates
            .entry(source)
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);

        // Increment per-symbol source counter
        self.symbol_source_updates
            .entry(symbol_lower.clone())
            .or_default()
            .entry(source)
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);

        // Track timestamp for TPS calculation
        if let Ok(mut recent) = self.recent_updates.lock() {
            recent.push_back(now);

            // Remove old entries outside the TPS window
            let cutoff = now - std::time::Duration::from_secs(TPS_WINDOW_SECS);
            while let Some(front) = recent.front() {
                if *front < cutoff {
                    recent.pop_front();
                } else {
                    break;
                }
            }
        }

        // Mark source as online (successful update)
        self.mark_source_online(source, timestamp as u64);

        // Broadcast update
        debug!(
            "Broadcasting {} update from {:?}: ${:.2}",
            symbol_lower, source, aggregated
        );
        let _ = self.tx.send(update);

        // Save to Redis in background
        let symbol_for_redis = symbol_lower;
        let self_clone = self.clone_for_redis();
        tokio::spawn(async move {
            self_clone
                .save_to_redis(&symbol_for_redis, aggregated, &source_prices)
                .await;
        });
    }

    fn clone_for_redis(&self) -> PriceCacheRedisRef {
        PriceCacheRedisRef {
            redis: self.redis.clone(),
        }
    }

    /// Calculate weighted average price from sources.
    fn aggregate(&self, sources: &[SourcePrice]) -> f64 {
        if sources.is_empty() {
            return 0.0;
        }

        let mut total_weight = 0u32;
        let mut weighted_sum = 0.0;

        for source in sources {
            let weight = source.source.weight();
            total_weight += weight;
            weighted_sum += source.price * weight as f64;
        }

        if total_weight == 0 {
            sources[0].price
        } else {
            weighted_sum / total_weight as f64
        }
    }

    /// Get the current aggregated price for a symbol.
    pub fn get_price(&self, symbol: &str) -> Option<f64> {
        let entry = self.prices.get(&symbol.to_lowercase())?;
        entry.last_aggregated
    }

    /// Get all current prices.
    pub fn get_all_prices(&self) -> Vec<(String, f64)> {
        self.prices
            .iter()
            .filter_map(|entry| {
                entry
                    .last_aggregated
                    .map(|price| (entry.key().clone(), price))
            })
            .collect()
    }

    /// Get sources for a symbol.
    pub fn get_sources(&self, symbol: &str) -> Vec<PriceSource> {
        self.prices
            .get(&symbol.to_lowercase())
            .map(|entry| entry.sources.iter().map(|s| s.source).collect())
            .unwrap_or_default()
    }

    /// Get the trade direction for a symbol.
    pub fn get_trade_direction(&self, symbol: &str) -> Option<TradeDirection> {
        self.prices
            .get(&symbol.to_lowercase())
            .and_then(|entry| entry.trade_direction)
    }

    /// Mark a source as online after a successful update.
    fn mark_source_online(&self, source: PriceSource, timestamp: u64) {
        let status = self.source_status.entry(source).or_default();
        let was_offline = !status.online.swap(true, Ordering::Relaxed);
        status.last_success.store(timestamp, Ordering::Relaxed);

        if was_offline {
            info!("Source {:?} is now ONLINE", source);
            // Clear the error when coming back online
            self.source_errors.remove(&source);
        }
    }

    /// Report an error from a source, marking it offline.
    pub fn report_source_error(&self, source: PriceSource, error: &str) {
        let status = self.source_status.entry(source).or_default();
        let was_online = status.online.swap(false, Ordering::Relaxed);

        if was_online {
            warn!("Source {:?} is now OFFLINE: {}", source, error);
        }

        // Store the error message
        self.source_errors.insert(source, error.to_string());
    }

    /// Check if a source is online.
    pub fn is_source_online(&self, source: PriceSource) -> bool {
        self.source_status
            .get(&source)
            .map(|s| s.online.load(Ordering::Relaxed))
            .unwrap_or(true) // Assume online if not tracked yet
    }

    /// Get the last error for a source.
    pub fn get_source_error(&self, source: PriceSource) -> Option<String> {
        self.source_errors.get(&source).map(|e| e.clone())
    }

    /// Get exchange statistics based on updates tracked through our system.
    pub fn get_exchange_stats(&self) -> Vec<ExchangeStats> {
        let total_updates = self.total_updates.load(Ordering::Relaxed);

        let mut result: Vec<ExchangeStats> = self
            .source_updates
            .iter()
            .map(|entry| {
                let source = *entry.key();
                let update_count = entry.value().load(Ordering::Relaxed);
                let online = self.is_source_online(source);
                let last_error = if !online {
                    self.get_source_error(source)
                } else {
                    None
                };
                let last_update = self.get_source_last_update(source);
                ExchangeStats {
                    source,
                    update_count,
                    update_percent: if total_updates > 0 {
                        (update_count as f64 / total_updates as f64) * 100.0
                    } else {
                        0.0
                    },
                    online,
                    last_error,
                    last_update,
                }
            })
            .collect();

        // Sort by update count descending
        result.sort_by(|a, b| b.update_count.cmp(&a.update_count));

        result
    }

    /// Get total update count.
    pub fn get_total_updates(&self) -> u64 {
        self.total_updates.load(Ordering::Relaxed)
    }

    /// Get transactions per second (average over last 60 seconds).
    pub fn get_tps(&self) -> f64 {
        let Ok(recent) = self.recent_updates.lock() else {
            return 0.0;
        };
        let count = recent.len();
        if count == 0 {
            return 0.0;
        }

        // Calculate actual time window from first to last update
        if let (Some(first), Some(last)) = (recent.front(), recent.back()) {
            let duration: std::time::Duration = last.duration_since(*first);
            let secs = duration.as_secs_f64();
            if secs > 0.0 {
                return count as f64 / secs;
            }
        }

        count as f64 / TPS_WINDOW_SECS as f64
    }

    /// Get uptime in seconds.
    pub fn get_uptime_secs(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// Get number of active symbols being tracked.
    pub fn get_active_symbols(&self) -> usize {
        self.prices.len()
    }

    /// Get number of online sources.
    pub fn get_online_sources(&self) -> usize {
        self.source_status
            .iter()
            .filter(|entry| entry.value().online.load(Ordering::Relaxed))
            .count()
    }

    /// Get last update timestamp for a source (unix ms).
    pub fn get_source_last_update(&self, source: PriceSource) -> Option<u64> {
        self.source_status
            .get(&source)
            .map(|s| s.last_success.load(Ordering::Relaxed))
            .filter(|&ts| ts > 0)
    }

    /// Get source statistics for a specific symbol.
    /// Returns update counts per source for the given symbol.
    pub fn get_symbol_source_stats(&self, symbol: &str) -> Vec<SymbolSourceStat> {
        let symbol_lower = symbol.to_lowercase();

        let Some(symbol_sources) = self.symbol_source_updates.get(&symbol_lower) else {
            return vec![];
        };

        // Calculate total updates for this symbol
        let total_updates: u64 = symbol_sources
            .iter()
            .map(|entry| entry.value().load(Ordering::Relaxed))
            .sum();

        let mut result: Vec<SymbolSourceStat> = symbol_sources
            .iter()
            .map(|entry| {
                let source = *entry.key();
                let update_count = entry.value().load(Ordering::Relaxed);
                let online = self.is_source_online(source);
                SymbolSourceStat {
                    source,
                    update_count,
                    update_percent: if total_updates > 0 {
                        (update_count as f64 / total_updates as f64) * 100.0
                    } else {
                        0.0
                    },
                    online,
                }
            })
            .collect();

        // Sort by update count descending
        result.sort_by(|a, b| b.update_count.cmp(&a.update_count));

        result
    }

    /// Get confidence metrics for a specific symbol.
    pub fn get_symbol_confidence(&self, symbol: &str) -> SymbolConfidence {
        let symbol_lower = symbol.to_lowercase();
        let now = chrono::Utc::now().timestamp();

        // Get source stats
        let source_stats = self.get_symbol_source_stats(&symbol_lower);
        let source_count = source_stats.len();
        let online_sources = source_stats.iter().filter(|s| s.online).count();
        let total_updates: u64 = source_stats.iter().map(|s| s.update_count).sum();

        // Get price data
        let (current_price, price_spread_percent, seconds_since_update) = self
            .prices
            .get(&symbol_lower)
            .map(|entry| {
                let prices: Vec<f64> = entry.sources.iter().map(|s| s.price).collect();
                let spread = if prices.len() >= 2 {
                    let min = prices.iter().cloned().fold(f64::INFINITY, f64::min);
                    let max = prices.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                    let avg = prices.iter().sum::<f64>() / prices.len() as f64;
                    if avg > 0.0 {
                        Some(((max - min) / avg) * 100.0)
                    } else {
                        None
                    }
                } else {
                    None
                };

                let latest_timestamp = entry.sources.iter().map(|s| s.timestamp).max().unwrap_or(0);
                let secs_since = if latest_timestamp > 0 {
                    Some(now - latest_timestamp)
                } else {
                    None
                };

                (entry.last_aggregated, spread, secs_since)
            })
            .unwrap_or((None, None, None));

        // Calculate confidence factors

        // Source diversity (0-30): More sources = higher confidence
        // 1 source = 10, 2 sources = 18, 3 sources = 24, 4+ sources = 30
        let source_diversity = match online_sources {
            0 => 0,
            1 => 10,
            2 => 18,
            3 => 24,
            _ => 30,
        };

        // Update frequency (0-25): More updates = higher confidence
        // Based on total updates for this symbol
        let update_frequency = match total_updates {
            0 => 0,
            1..=10 => 5,
            11..=100 => 10,
            101..=1000 => 15,
            1001..=10000 => 20,
            _ => 25,
        };

        // Data recency (0-25): More recent = higher confidence
        // < 5s = 25, < 30s = 20, < 60s = 15, < 300s = 10, < 600s = 5, > 600s = 0
        let data_recency = match seconds_since_update {
            Some(secs) if secs < 5 => 25,
            Some(secs) if secs < 30 => 20,
            Some(secs) if secs < 60 => 15,
            Some(secs) if secs < 300 => 10,
            Some(secs) if secs < 600 => 5,
            _ => 0,
        };

        // Price consistency (0-20): Lower spread = higher consistency
        // < 0.1% = 20, < 0.5% = 16, < 1% = 12, < 2% = 8, < 5% = 4, > 5% = 0
        let price_consistency = match price_spread_percent {
            Some(spread) if spread < 0.1 => 20,
            Some(spread) if spread < 0.5 => 16,
            Some(spread) if spread < 1.0 => 12,
            Some(spread) if spread < 2.0 => 8,
            Some(spread) if spread < 5.0 => 4,
            Some(_) => 0,
            None if source_count == 1 => 15, // Single source, assume good
            None => 0,
        };

        let score =
            (source_diversity + update_frequency + data_recency + price_consistency).min(100);

        SymbolConfidence {
            score,
            source_count,
            online_sources,
            total_updates,
            current_price,
            price_spread_percent,
            seconds_since_update,
            factors: ConfidenceFactors {
                source_diversity,
                update_frequency,
                data_recency,
                price_consistency,
            },
        }
    }

    /// Load update counts from Redis.
    pub async fn load_update_counts(&self) {
        let conn_guard = self.redis.read().await;
        let Some(ref conn) = *conn_guard else { return };

        let mut conn = conn.clone();

        // Load total count
        let total_result: Result<Option<u64>, _> = redis::cmd("GET")
            .arg(REDIS_UPDATE_COUNT_KEY)
            .query_async(&mut conn)
            .await;

        if let Ok(Some(count)) = total_result {
            self.total_updates.store(count, Ordering::Relaxed);
            info!("Loaded total update count from Redis: {}", count);
        }

        // Load source counts
        let source_result: Result<Option<String>, _> = redis::cmd("GET")
            .arg(REDIS_SOURCE_COUNTS_KEY)
            .query_async(&mut conn)
            .await;

        if let Ok(Some(data)) = source_result {
            if let Ok(counts) = serde_json::from_str::<HashMap<String, u64>>(&data) {
                for (source_str, count) in counts {
                    if let Ok(source) =
                        serde_json::from_str::<PriceSource>(&format!("\"{}\"", source_str))
                    {
                        self.source_updates
                            .entry(source)
                            .or_insert_with(|| AtomicU64::new(0))
                            .store(count, Ordering::Relaxed);
                    }
                }
                info!("Loaded source update counts from Redis");
            }
        }
    }

    /// Save update counts to Redis.
    pub async fn save_update_counts(&self) {
        let conn_guard = self.redis.read().await;
        let Some(ref conn) = *conn_guard else { return };

        let mut conn = conn.clone();

        // Save total count
        let total = self.total_updates.load(Ordering::Relaxed);
        let _ = redis::cmd("SET")
            .arg(REDIS_UPDATE_COUNT_KEY)
            .arg(total)
            .query_async::<_, ()>(&mut conn)
            .await;

        // Save source counts
        let source_counts: HashMap<String, u64> = self
            .source_updates
            .iter()
            .map(|entry| {
                (
                    entry.key().to_string(),
                    entry.value().load(Ordering::Relaxed),
                )
            })
            .collect();

        if let Ok(json) = serde_json::to_string(&source_counts) {
            let _ = redis::cmd("SET")
                .arg(REDIS_SOURCE_COUNTS_KEY)
                .arg(&json)
                .query_async::<_, ()>(&mut conn)
                .await;
        }

        debug!("Saved update counts to Redis: total={}", total);
    }
}

/// Exchange statistics for API response.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeStats {
    pub source: PriceSource,
    pub update_count: u64,
    pub update_percent: f64,
    pub online: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    /// Last successful update timestamp (unix ms).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_update: Option<u64>,
}

/// Per-symbol source statistics for API response.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolSourceStat {
    pub source: PriceSource,
    pub update_count: u64,
    pub update_percent: f64,
    pub online: bool,
}

/// Confidence metrics for a symbol.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolConfidence {
    /// Overall confidence score (0-100).
    pub score: u8,
    /// Number of data sources providing prices.
    pub source_count: usize,
    /// Number of online sources.
    pub online_sources: usize,
    /// Total update count for this symbol.
    pub total_updates: u64,
    /// Current aggregated price.
    pub current_price: Option<f64>,
    /// Price spread as percentage (max - min) / avg.
    pub price_spread_percent: Option<f64>,
    /// Seconds since last update.
    pub seconds_since_update: Option<i64>,
    /// Breakdown of confidence factors.
    pub factors: ConfidenceFactors,
}

/// Breakdown of confidence calculation factors.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfidenceFactors {
    /// Score from source diversity (0-30).
    pub source_diversity: u8,
    /// Score from update frequency (0-25).
    pub update_frequency: u8,
    /// Score from data recency (0-25).
    pub data_recency: u8,
    /// Score from price consistency (0-20).
    pub price_consistency: u8,
}

/// Helper struct for Redis serialization.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct SymbolPriceData {
    price: f64,
    sources: Vec<(PriceSource, f64)>,
    timestamp: i64,
}

/// Helper struct for async Redis operations.
struct PriceCacheRedisRef {
    redis: Arc<RwLock<Option<ConnectionManager>>>,
}

impl PriceCacheRedisRef {
    async fn save_to_redis(&self, symbol: &str, price: f64, sources: &HashMap<PriceSource, f64>) {
        let conn_guard = self.redis.read().await;
        let Some(ref conn) = *conn_guard else { return };

        let key = format!("{}{}", REDIS_PRICE_PREFIX, symbol.to_lowercase());
        let data = SymbolPriceData {
            price,
            sources: sources.iter().map(|(k, v)| (*k, *v)).collect(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        if let Ok(json) = serde_json::to_string(&data) {
            let mut conn = conn.clone();
            let _ = redis::cmd("SETEX")
                .arg(&key)
                .arg(3600)
                .arg(&json)
                .query_async::<_, ()>(&mut conn)
                .await;
        }
    }
}

// TUI helper methods
impl PriceCache {
    /// Get top N prices with their 24h change percentage.
    /// Returns (symbol, price, change_percent).
    pub fn get_top_prices(&self, limit: usize) -> Vec<(String, f64, f64)> {
        let mut prices: Vec<_> = self
            .prices
            .iter()
            .filter_map(|entry| {
                let symbol = entry.key().clone();
                let data = entry.value();
                let price = data.last_aggregated?;
                // Mock change for now (in real impl, track 24h ago price)
                // Use symbol hash to generate consistent but varied changes
                let hash = symbol.bytes().map(|b| b as u64).sum::<u64>();
                let change = ((hash % 1000) as f64 / 100.0) - 5.0; // -5% to +5%
                Some((symbol, price, change))
            })
            .collect();

        prices.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        prices.truncate(limit);
        prices
    }

    /// Get update counts per symbol.
    pub fn get_update_counts(&self) -> HashMap<String, u64> {
        self.symbol_source_updates
            .iter()
            .map(|entry| {
                let symbol = entry.key().clone();
                let sources = entry.value();
                let total: u64 = sources.iter().map(|s| s.value().load(Ordering::Relaxed)).sum();
                (symbol, total)
            })
            .collect()
    }
}
