use crate::types::{ChartRange, ChartResolution, Mover, MoverTimeframe, OhlcPoint};
use dashmap::DashMap;
use redis::{aio::ConnectionManager, AsyncCommands};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// OHLC bucket for a time period.
#[derive(Debug, Clone)]
struct OhlcBucket {
    time: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}

impl OhlcBucket {
    fn new(time: i64, price: f64) -> Self {
        Self {
            time,
            open: price,
            high: price,
            low: price,
            close: price,
            volume: 0.0,
        }
    }

    fn update(&mut self, price: f64, volume: Option<f64>) {
        self.high = self.high.max(price);
        self.low = self.low.min(price);
        self.close = price;
        if let Some(v) = volume {
            self.volume += v;
        }
    }

    fn to_ohlc_point(&self) -> OhlcPoint {
        OhlcPoint {
            time: self.time,
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            volume: if self.volume > 0.0 { Some(self.volume) } else { None },
        }
    }
}

/// Time series data for a single resolution.
#[derive(Debug)]
struct TimeSeries {
    resolution: ChartResolution,
    buckets: VecDeque<OhlcBucket>,
    max_buckets: usize,
}

impl TimeSeries {
    fn new(resolution: ChartResolution) -> Self {
        let retention_seconds = resolution.retention_seconds();
        let bucket_seconds = resolution.seconds();
        let max_buckets = (retention_seconds / bucket_seconds) as usize;

        Self {
            resolution,
            buckets: VecDeque::with_capacity(max_buckets),
            max_buckets,
        }
    }

    fn add_price(&mut self, price: f64, volume: Option<f64>, timestamp: i64) {
        let bucket_time = (timestamp / 1000) / self.resolution.seconds() * self.resolution.seconds();

        // Fast path: check if this is an update to the last bucket (real-time data)
        if let Some(last) = self.buckets.back_mut() {
            if last.time == bucket_time {
                last.update(price, volume);
                return;
            }
            // If new data is after the last bucket, just push
            if bucket_time > last.time {
                self.buckets.push_back(OhlcBucket::new(bucket_time, price));
                // Trim old buckets
                while self.buckets.len() > self.max_buckets {
                    self.buckets.pop_front();
                }
                return;
            }
        }

        // Historical data: need to find correct position or update existing bucket
        // Linear search through buckets (they should be sorted by time)
        let mut found_idx = None;
        let mut insert_idx = self.buckets.len(); // Default to end

        for (idx, bucket) in self.buckets.iter().enumerate() {
            if bucket.time == bucket_time {
                found_idx = Some(idx);
                break;
            }
            if bucket.time > bucket_time {
                insert_idx = idx;
                break;
            }
        }

        if let Some(idx) = found_idx {
            // Bucket exists, update it
            if let Some(bucket) = self.buckets.get_mut(idx) {
                bucket.update(price, volume);
            }
        } else {
            // Insert new bucket at correct sorted position
            let bucket = OhlcBucket::new(bucket_time, price);
            self.buckets.insert(insert_idx, bucket);

            // Trim old buckets from the front
            while self.buckets.len() > self.max_buckets {
                self.buckets.pop_front();
            }
        }
    }

    fn get_data(&self, start_time: i64) -> Vec<OhlcPoint> {
        self.buckets
            .iter()
            .filter(|b| b.time >= start_time)
            .map(|b| b.to_ohlc_point())
            .collect()
    }
}

/// Symbol-specific chart data.
#[derive(Debug)]
struct SymbolChartData {
    one_minute: TimeSeries,
    five_minute: TimeSeries,
    one_hour: TimeSeries,
    /// Current price for quick access.
    current_price: Option<f64>,
    /// Last update timestamp (unix seconds).
    last_update: i64,
    /// Cached 24h volume from authoritative sources.
    volume_24h: Option<f64>,
}

impl Default for SymbolChartData {
    fn default() -> Self {
        Self {
            one_minute: TimeSeries::new(ChartResolution::OneMinute),
            five_minute: TimeSeries::new(ChartResolution::FiveMinute),
            one_hour: TimeSeries::new(ChartResolution::OneHour),
            current_price: None,
            last_update: 0,
            volume_24h: None,
        }
    }
}

const REDIS_SPARKLINE_PREFIX: &str = "haunt:sparkline:";

/// Maximum number of sparkline points to store in Redis (8+ hours at 1-min intervals)
const MAX_REDIS_SPARKLINE_POINTS: usize = 500;

/// TTL for sparkline data in Redis (24 hours)
const SPARKLINE_TTL_SECS: i64 = 86400;

/// Chart data store with multiple resolutions and optional Redis persistence.
pub struct ChartStore {
    data: DashMap<String, SymbolChartData>,
    redis: RwLock<Option<ConnectionManager>>,
}

impl ChartStore {
    /// Create a new chart store.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            data: DashMap::new(),
            redis: RwLock::new(None),
        })
    }

    /// Connect to Redis for persistence.
    pub async fn connect_redis(&self, redis_url: &str) {
        match redis::Client::open(redis_url) {
            Ok(client) => {
                match ConnectionManager::new(client).await {
                    Ok(conn) => {
                        info!("ChartStore connected to Redis at {}", redis_url);
                        *self.redis.write().await = Some(conn);
                    }
                    Err(e) => {
                        warn!("Failed to connect ChartStore to Redis: {}. Data will not persist.", e);
                    }
                }
            }
            Err(e) => {
                warn!("Invalid Redis URL: {}. Data will not persist.", e);
            }
        }
    }

    /// Load sparkline data from Redis for known symbols.
    pub async fn load_from_redis(&self, symbols: &[&str]) {
        let conn_guard = self.redis.read().await;
        let Some(ref conn) = *conn_guard else {
            return;
        };

        let mut conn = conn.clone();
        let mut loaded_count = 0;

        for symbol in symbols {
            let key = format!("{}{}", REDIS_SPARKLINE_PREFIX, symbol.to_lowercase());

            let values: Result<Vec<String>, _> = redis::cmd("LRANGE")
                .arg(&key)
                .arg(0)
                .arg(-1)
                .query_async(&mut conn)
                .await;

            if let Ok(vals) = values {
                if !vals.is_empty() {
                    let prices: Vec<f64> = vals
                        .iter()
                        .filter_map(|v| v.split(':').nth(1).and_then(|p| p.parse().ok()))
                        .collect();

                    if !prices.is_empty() {
                        self.seed_sparkline(symbol, &prices);
                        loaded_count += 1;
                        debug!("Loaded {} sparkline points for {} from Redis", prices.len(), symbol);
                    }
                }
            }
        }

        if loaded_count > 0 {
            info!("Loaded sparkline data for {} symbols from Redis", loaded_count);
        }
    }

    /// Save sparkline data to Redis.
    pub async fn save_to_redis(&self, symbol: &str) {
        let conn_guard = self.redis.read().await;
        let Some(ref conn) = *conn_guard else {
            return;
        };

        let sparkline = self.get_sparkline(symbol, MAX_REDIS_SPARKLINE_POINTS);
        if sparkline.is_empty() {
            return;
        }

        let key = format!("{}{}", REDIS_SPARKLINE_PREFIX, symbol.to_lowercase());
        let now = chrono::Utc::now().timestamp_millis();
        let interval_ms = 60_000i64; // 1 minute

        let mut conn = conn.clone();

        // Clear existing data
        let _ = redis::cmd("DEL")
            .arg(&key)
            .query_async::<_, ()>(&mut conn)
            .await;

        // Add all points
        for (i, price) in sparkline.iter().enumerate() {
            let timestamp = now - (sparkline.len() - 1 - i) as i64 * interval_ms;
            let value = format!("{}:{}", timestamp, price);
            let _ = redis::cmd("RPUSH")
                .arg(&key)
                .arg(&value)
                .query_async::<_, i64>(&mut conn)
                .await;
        }

        // Set TTL to 24 hours
        let _ = redis::cmd("EXPIRE")
            .arg(&key)
            .arg(SPARKLINE_TTL_SECS)
            .query_async::<_, ()>(&mut conn)
            .await;
    }

    /// Save all sparklines to Redis.
    pub async fn save_all_to_redis(&self) {
        let symbols: Vec<String> = self.data.iter().map(|e| e.key().clone()).collect();
        for symbol in symbols {
            self.save_to_redis(&symbol).await;
        }
        debug!("Saved all sparklines to Redis");
    }

    /// Load all available sparkline data from Redis by scanning for keys.
    /// This finds all symbols that have been persisted, not just a hardcoded list.
    pub async fn load_all_from_redis(&self) {
        let conn_guard = self.redis.read().await;
        let Some(ref conn) = *conn_guard else {
            return;
        };

        let mut conn = conn.clone();
        let pattern = format!("{}*", REDIS_SPARKLINE_PREFIX);
        let mut cursor: u64 = 0;
        let mut loaded_count = 0;

        loop {
            // SCAN for keys matching the sparkline pattern
            let result: Result<(u64, Vec<String>), _> = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await;

            match result {
                Ok((new_cursor, keys)) => {
                    for key in keys {
                        // Extract symbol from key (e.g., "haunt:sparkline:btc" -> "btc")
                        if let Some(symbol) = key.strip_prefix(REDIS_SPARKLINE_PREFIX) {
                            let values: Result<Vec<String>, _> = redis::cmd("LRANGE")
                                .arg(&key)
                                .arg(0)
                                .arg(-1)
                                .query_async(&mut conn)
                                .await;

                            if let Ok(vals) = values {
                                if !vals.is_empty() {
                                    let prices: Vec<f64> = vals
                                        .iter()
                                        .filter_map(|v| v.split(':').nth(1).and_then(|p| p.parse().ok()))
                                        .collect();

                                    if !prices.is_empty() {
                                        self.seed_sparkline(symbol, &prices);
                                        loaded_count += 1;
                                        debug!("Loaded {} sparkline points for {} from Redis", prices.len(), symbol);
                                    }
                                }
                            }
                        }
                    }

                    cursor = new_cursor;
                    if cursor == 0 {
                        break;
                    }
                }
                Err(e) => {
                    warn!("Failed to scan Redis for sparkline keys: {}", e);
                    break;
                }
            }
        }

        if loaded_count > 0 {
            info!("Loaded sparkline data for {} symbols from Redis (via SCAN)", loaded_count);
        }
    }

    /// Add a price point for a symbol.
    pub fn add_price(&self, symbol: &str, price: f64, volume: Option<f64>, timestamp: i64) {
        let symbol_lower = symbol.to_lowercase();
        let mut entry = self.data.entry(symbol_lower).or_default();
        let chart_data = entry.value_mut();

        chart_data.one_minute.add_price(price, volume, timestamp);
        chart_data.five_minute.add_price(price, volume, timestamp);
        chart_data.one_hour.add_price(price, volume, timestamp);

        // Track current price and update time
        chart_data.current_price = Some(price);
        chart_data.last_update = timestamp / 1000; // Convert ms to seconds
    }

    /// Update 24h volume for a symbol (from authoritative sources).
    pub fn update_volume(&self, symbol: &str, volume: f64) {
        let symbol_lower = symbol.to_lowercase();
        if let Some(mut entry) = self.data.get_mut(&symbol_lower) {
            entry.volume_24h = Some(volume);
        }
    }

    /// Get 24h volume for a symbol (cached or calculated from trades).
    pub fn get_volume_24h(&self, symbol: &str) -> Option<f64> {
        let symbol_lower = symbol.to_lowercase();
        let entry = self.data.get(&symbol_lower)?;

        // Return cached authoritative volume if available
        if let Some(vol) = entry.volume_24h {
            if vol > 0.0 {
                return Some(vol);
            }
        }

        // Otherwise calculate from 5-minute buckets over last 24 hours
        let now = chrono::Utc::now().timestamp();
        let start_time = now - 86400; // 24 hours ago
        let data = entry.five_minute.get_data(start_time);

        let total_volume: f64 = data.iter()
            .filter_map(|p| p.volume)
            .sum();

        if total_volume > 0.0 {
            Some(total_volume)
        } else {
            None
        }
    }

    /// Get the current price for a symbol.
    pub fn get_current_price(&self, symbol: &str) -> Option<f64> {
        self.data
            .get(&symbol.to_lowercase())
            .and_then(|e| e.current_price)
    }

    /// Get the price at a specific time ago.
    /// Returns the closest available price from the time series.
    pub fn get_price_at(&self, symbol: &str, seconds_ago: i64) -> Option<f64> {
        let symbol_lower = symbol.to_lowercase();
        let entry = self.data.get(&symbol_lower)?;
        let now = chrono::Utc::now().timestamp();
        let target_time = now - seconds_ago;

        // Choose the appropriate time series based on the time range
        let data = if seconds_ago <= 3600 {
            // Use 1-minute data for up to 1 hour
            entry.one_minute.get_data(target_time - 60)
        } else if seconds_ago <= 86400 {
            // Use 5-minute data for up to 24 hours
            entry.five_minute.get_data(target_time - 300)
        } else {
            // Use 1-hour data for longer periods
            entry.one_hour.get_data(target_time - 3600)
        };

        // Find the closest data point to our target time
        data.iter()
            .min_by_key(|p| (p.time - target_time).abs())
            .map(|p| p.close)
    }

    /// Calculate percentage change over a time window.
    pub fn get_price_change(&self, symbol: &str, seconds: i64) -> Option<f64> {
        let current = self.get_current_price(symbol)?;
        let past = self.get_price_at(symbol, seconds)?;

        if past <= 0.0 {
            return None;
        }

        Some(((current - past) / past) * 100.0)
    }

    /// Get top movers (gainers and losers) for a time window.
    /// If symbol_filter is Some, only include symbols in the filter set.
    pub fn get_top_movers(
        &self,
        timeframe: MoverTimeframe,
        limit: usize,
        symbol_filter: Option<&std::collections::HashSet<String>>,
    ) -> (Vec<Mover>, Vec<Mover>) {
        let seconds = timeframe.seconds();
        let mut movers: Vec<Mover> = Vec::new();

        // Calculate change for all tracked symbols
        for entry in self.data.iter() {
            let symbol = entry.key().clone();
            let chart_data = entry.value();

            // Apply symbol filter if provided
            if let Some(filter) = symbol_filter {
                if !filter.contains(&symbol) && !filter.contains(&symbol.to_uppercase()) {
                    continue;
                }
            }

            // Skip symbols without current price or recent updates
            let Some(current_price) = chart_data.current_price else {
                continue;
            };

            // Skip stale data (no update in last 5 minutes)
            let now = chrono::Utc::now().timestamp();
            if now - chart_data.last_update > 300 {
                continue;
            }

            // Get historical price
            if let Some(change_percent) = self.get_price_change(&symbol, seconds) {
                // Skip tiny changes (noise)
                if change_percent.abs() < 0.01 {
                    continue;
                }

                movers.push(Mover {
                    symbol: symbol.to_uppercase(),
                    price: current_price,
                    change_percent,
                    volume_24h: chart_data.volume_24h,
                });
            }
        }

        // Sort by change percentage
        let mut gainers: Vec<Mover> = movers
            .iter()
            .filter(|m| m.change_percent > 0.0)
            .cloned()
            .collect();
        gainers.sort_by(|a, b| b.change_percent.partial_cmp(&a.change_percent).unwrap_or(std::cmp::Ordering::Equal));
        gainers.truncate(limit);

        let mut losers: Vec<Mover> = movers
            .iter()
            .filter(|m| m.change_percent < 0.0)
            .cloned()
            .collect();
        losers.sort_by(|a, b| a.change_percent.partial_cmp(&b.change_percent).unwrap_or(std::cmp::Ordering::Equal));
        losers.truncate(limit);

        (gainers, losers)
    }

    /// Get chart data for a symbol and range.
    pub fn get_chart(&self, symbol: &str, range: ChartRange) -> Vec<OhlcPoint> {
        let symbol_lower = symbol.to_lowercase();
        let entry = match self.data.get(&symbol_lower) {
            Some(e) => e,
            None => return Vec::new(),
        };

        let now = chrono::Utc::now().timestamp();
        let start_time = now - range.duration_seconds();

        match range {
            ChartRange::OneHour | ChartRange::FourHours => {
                entry.one_minute.get_data(start_time)
            }
            ChartRange::OneDay => {
                entry.five_minute.get_data(start_time)
            }
            ChartRange::OneWeek | ChartRange::OneMonth => {
                entry.one_hour.get_data(start_time)
            }
        }
    }

    /// Get sparkline data for a symbol (last N close prices).
    /// Returns prices from available data, preferring 1-hour resolution for longer sparklines.
    pub fn get_sparkline(&self, symbol: &str, points: usize) -> Vec<f64> {
        let symbol_lower = symbol.to_lowercase();
        let entry = match self.data.get(&symbol_lower) {
            Some(e) => e,
            None => return Vec::new(),
        };

        let now = chrono::Utc::now().timestamp();

        // For longer sparklines (>120 points), use 1-hour data
        if points > 120 {
            let start_time = now - (points as i64 * 3600); // 1 hour per point
            let data: Vec<f64> = entry.one_hour
                .get_data(start_time)
                .iter()
                .map(|p| p.close)
                .collect();

            if data.len() >= points / 2 {
                return data.into_iter().rev().take(points).rev().collect();
            }
        }

        // For medium sparklines, use 5-minute data
        if points > 60 {
            let start_time = now - (points as i64 * 300); // 5 minutes per point
            let data: Vec<f64> = entry.five_minute
                .get_data(start_time)
                .iter()
                .map(|p| p.close)
                .collect();

            if data.len() >= points / 2 {
                return data.into_iter().rev().take(points).rev().collect();
            }
        }

        // Default: 1-minute data
        let start_time = now - (points as i64 * 60);
        entry.one_minute
            .get_data(start_time)
            .iter()
            .rev()
            .take(points)
            .rev()
            .map(|p| p.close)
            .collect()
    }

    /// Get total data point count across all resolutions for a symbol.
    pub fn get_data_point_count(&self, symbol: &str) -> usize {
        let symbol_lower = symbol.to_lowercase();
        let entry = match self.data.get(&symbol_lower) {
            Some(e) => e,
            None => return 0,
        };

        // Sum up data points from all time series
        let now = chrono::Utc::now().timestamp();
        let one_min_count = entry.one_minute.get_data(now - 86400).len(); // Last 24h of 1-min data
        let five_min_count = entry.five_minute.get_data(now - 604800).len(); // Last 7d of 5-min data
        let one_hour_count = entry.one_hour.get_data(now - 2592000).len(); // Last 30d of hourly data

        one_min_count + five_min_count + one_hour_count
    }

    /// Generate sparkline from historical OHLC data.
    /// Returns the last N close prices from the appropriate resolution.
    pub fn generate_sparkline_from_history(&self, symbol: &str, points: usize) -> Vec<f64> {
        let symbol_lower = symbol.to_lowercase();
        let entry = match self.data.get(&symbol_lower) {
            Some(e) => e,
            None => return Vec::new(),
        };

        let now = chrono::Utc::now().timestamp();

        // For sparklines, prefer hourly data for smoother visualization
        let start_time = now - (points as i64 * 3600);
        let data: Vec<f64> = entry.one_hour
            .get_data(start_time)
            .iter()
            .map(|p| p.close)
            .collect();

        if data.len() >= points / 2 {
            return data.into_iter().rev().take(points).rev().collect();
        }

        // Fall back to 5-minute data if hourly data is sparse
        let start_time = now - (points as i64 * 300);
        let data: Vec<f64> = entry.five_minute
            .get_data(start_time)
            .iter()
            .map(|p| p.close)
            .collect();

        if data.len() >= points / 2 {
            return data.into_iter().rev().take(points).rev().collect();
        }

        // Last resort: 1-minute data
        let start_time = now - (points as i64 * 60);
        entry.one_minute
            .get_data(start_time)
            .iter()
            .rev()
            .take(points)
            .rev()
            .map(|p| p.close)
            .collect()
    }

    /// Seed sparkline data for a symbol from historical prices.
    /// Used to initialize charts with data from external sources.
    pub fn seed_sparkline(&self, symbol: &str, prices: &[f64]) {
        if prices.is_empty() {
            return;
        }

        let symbol_lower = symbol.to_lowercase();
        let now = chrono::Utc::now().timestamp_millis();

        // Spread prices evenly over the past hour
        let interval_ms = 3600_000 / prices.len().max(1) as i64;

        for (i, price) in prices.iter().enumerate() {
            let timestamp = now - (prices.len() - 1 - i) as i64 * interval_ms;
            self.add_price(&symbol_lower, *price, None, timestamp);
        }
    }
}

impl Default for ChartStore {
    fn default() -> Self {
        Self {
            data: DashMap::new(),
            redis: RwLock::new(None),
        }
    }
}
