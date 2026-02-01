use crate::types::{ChartRange, ChartResolution, OhlcPoint};
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

        if let Some(last) = self.buckets.back_mut() {
            if last.time == bucket_time {
                last.update(price, volume);
                return;
            }
        }

        // Start a new bucket
        let bucket = OhlcBucket::new(bucket_time, price);
        self.buckets.push_back(bucket);

        // Trim old buckets
        while self.buckets.len() > self.max_buckets {
            self.buckets.pop_front();
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
}

impl Default for SymbolChartData {
    fn default() -> Self {
        Self {
            one_minute: TimeSeries::new(ChartResolution::OneMinute),
            five_minute: TimeSeries::new(ChartResolution::FiveMinute),
            one_hour: TimeSeries::new(ChartResolution::OneHour),
        }
    }
}

const REDIS_SPARKLINE_PREFIX: &str = "haunt:sparkline:";

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

        let sparkline = self.get_sparkline(symbol, 120); // Save up to 2 hours
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

        // Set TTL of 4 hours
        let _ = redis::cmd("EXPIRE")
            .arg(&key)
            .arg(14400)
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

    /// Add a price point for a symbol.
    pub fn add_price(&self, symbol: &str, price: f64, volume: Option<f64>, timestamp: i64) {
        let symbol_lower = symbol.to_lowercase();
        let mut entry = self.data.entry(symbol_lower).or_default();
        let chart_data = entry.value_mut();

        chart_data.one_minute.add_price(price, volume, timestamp);
        chart_data.five_minute.add_price(price, volume, timestamp);
        chart_data.one_hour.add_price(price, volume, timestamp);
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
    /// Returns prices at 1-minute intervals for the past hour (up to 60 points).
    pub fn get_sparkline(&self, symbol: &str, points: usize) -> Vec<f64> {
        let symbol_lower = symbol.to_lowercase();
        let entry = match self.data.get(&symbol_lower) {
            Some(e) => e,
            None => return Vec::new(),
        };

        let now = chrono::Utc::now().timestamp();
        let start_time = now - 3600; // Last hour

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
