// Note: RedisStore is not currently used in favor of ChartStore with direct Redis commands.
// Keeping this module for potential future use.
#![allow(dead_code)]

use redis::{aio::ConnectionManager, AsyncCommands, RedisResult};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Redis key prefixes
const SPARKLINE_PREFIX: &str = "haunt:sparkline:";
const PRICE_PREFIX: &str = "haunt:price:";

/// Maximum sparkline points to store per symbol
const MAX_SPARKLINE_POINTS: usize = 3600; // 1 hour at 1-second granularity

/// RedisStore for persistent caching of price and chart data.
#[derive(Clone)]
pub struct RedisStore {
    conn: Arc<RwLock<Option<ConnectionManager>>>,
}

impl RedisStore {
    /// Create a new RedisStore, connecting to Redis at the given URL.
    pub async fn new(redis_url: &str) -> Self {
        let conn = match Self::connect(redis_url).await {
            Ok(c) => {
                info!("Connected to Redis at {}", redis_url);
                Some(c)
            }
            Err(e) => {
                warn!(
                    "Failed to connect to Redis: {}. Running without persistence.",
                    e
                );
                None
            }
        };

        Self {
            conn: Arc::new(RwLock::new(conn)),
        }
    }

    async fn connect(redis_url: &str) -> RedisResult<ConnectionManager> {
        let client = redis::Client::open(redis_url)?;
        ConnectionManager::new(client).await
    }

    /// Check if Redis is connected.
    pub async fn is_connected(&self) -> bool {
        self.conn.read().await.is_some()
    }

    /// Add a price point to the sparkline for a symbol.
    pub async fn add_sparkline_point(&self, symbol: &str, price: f64, timestamp: i64) {
        let conn_guard = self.conn.read().await;
        let Some(ref conn) = *conn_guard else {
            return;
        };

        let key = format!("{}{}", SPARKLINE_PREFIX, symbol.to_lowercase());
        let value = format!("{}:{}", timestamp, price);

        let mut conn = conn.clone();
        if let Err(e) = redis::cmd("RPUSH")
            .arg(&key)
            .arg(&value)
            .query_async::<_, i64>(&mut conn)
            .await
        {
            error!("Failed to add sparkline point: {}", e);
            return;
        }

        // Trim to keep only the last MAX_SPARKLINE_POINTS
        if let Err(e) = redis::cmd("LTRIM")
            .arg(&key)
            .arg(-(MAX_SPARKLINE_POINTS as i64))
            .arg(-1)
            .query_async::<_, ()>(&mut conn)
            .await
        {
            error!("Failed to trim sparkline: {}", e);
        }

        // Set TTL of 2 hours
        let _ = redis::cmd("EXPIRE")
            .arg(&key)
            .arg(7200)
            .query_async::<_, ()>(&mut conn)
            .await;
    }

    /// Get the sparkline for a symbol (last N points).
    pub async fn get_sparkline(&self, symbol: &str, points: usize) -> Vec<f64> {
        let conn_guard = self.conn.read().await;
        let Some(ref conn) = *conn_guard else {
            return Vec::new();
        };

        let key = format!("{}{}", SPARKLINE_PREFIX, symbol.to_lowercase());
        let mut conn = conn.clone();

        let values: RedisResult<Vec<String>> = redis::cmd("LRANGE")
            .arg(&key)
            .arg(-(points as i64))
            .arg(-1)
            .query_async(&mut conn)
            .await;

        match values {
            Ok(vals) => vals
                .iter()
                .filter_map(|v| v.split(':').nth(1).and_then(|p| p.parse::<f64>().ok()))
                .collect(),
            Err(e) => {
                debug!("Failed to get sparkline for {}: {}", symbol, e);
                Vec::new()
            }
        }
    }

    /// Store the current price for a symbol.
    pub async fn set_price(
        &self,
        symbol: &str,
        price: f64,
        volume: Option<f64>,
        change_24h: Option<f64>,
    ) {
        let conn_guard = self.conn.read().await;
        let Some(ref conn) = *conn_guard else {
            return;
        };

        let key = format!("{}{}", PRICE_PREFIX, symbol.to_lowercase());
        let data = serde_json::json!({
            "price": price,
            "volume": volume,
            "change24h": change_24h,
            "timestamp": chrono::Utc::now().timestamp_millis()
        });

        let mut conn = conn.clone();
        if let Err(e) = conn.set_ex::<_, _, ()>(&key, data.to_string(), 300).await {
            error!("Failed to set price: {}", e);
        }
    }

    /// Get the current price for a symbol.
    pub async fn get_price(&self, symbol: &str) -> Option<(f64, Option<f64>, Option<f64>)> {
        let conn_guard = self.conn.read().await;
        let conn = conn_guard.as_ref()?;

        let key = format!("{}{}", PRICE_PREFIX, symbol.to_lowercase());
        let mut conn = conn.clone();

        let value: RedisResult<Option<String>> = conn.get(&key).await;

        match value {
            Ok(Some(json_str)) => {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    let price = data.get("price").and_then(|v| v.as_f64())?;
                    let volume = data.get("volume").and_then(|v| v.as_f64());
                    let change_24h = data.get("change24h").and_then(|v| v.as_f64());
                    Some((price, volume, change_24h))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Seed sparkline data for a symbol from historical prices.
    pub async fn seed_sparkline(&self, symbol: &str, prices: &[f64]) {
        if prices.is_empty() {
            return;
        }

        let conn_guard = self.conn.read().await;
        let Some(ref conn) = *conn_guard else {
            return;
        };

        let key = format!("{}{}", SPARKLINE_PREFIX, symbol.to_lowercase());
        let now = chrono::Utc::now().timestamp_millis();
        let interval_ms = 60_000; // 1 minute between points

        let mut conn = conn.clone();

        // Clear existing data
        let _ = redis::cmd("DEL")
            .arg(&key)
            .query_async::<_, ()>(&mut conn)
            .await;

        // Add all points
        for (i, price) in prices.iter().enumerate() {
            let timestamp = now - (prices.len() - 1 - i) as i64 * interval_ms;
            let value = format!("{}:{}", timestamp, price);
            let _ = redis::cmd("RPUSH")
                .arg(&key)
                .arg(&value)
                .query_async::<_, i64>(&mut conn)
                .await;
        }

        // Set TTL
        let _ = redis::cmd("EXPIRE")
            .arg(&key)
            .arg(7200)
            .query_async::<_, ()>(&mut conn)
            .await;

        debug!("Seeded {} sparkline points for {}", prices.len(), symbol);
    }
}
