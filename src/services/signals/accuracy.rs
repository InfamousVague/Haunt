//! Accuracy tracking for signal indicators.

use crate::types::{PredictionOutcome, SignalAccuracy};
use dashmap::DashMap;
use redis::{aio::ConnectionManager, AsyncCommands};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Redis key prefix for accuracy stats.
const REDIS_ACCURACY_PREFIX: &str = "haunt:accuracy:";

/// Store for tracking signal accuracy.
pub struct AccuracyStore {
    /// Accuracy stats: key = "{symbol}:{indicator}:{timeframe}"
    accuracies: DashMap<String, SignalAccuracy>,
    /// Global accuracy (across all symbols): key = "{indicator}:{timeframe}"
    global_accuracies: DashMap<String, SignalAccuracy>,
    /// Redis connection for persistence.
    redis: RwLock<Option<ConnectionManager>>,
}

impl AccuracyStore {
    /// Create a new accuracy store.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            accuracies: DashMap::new(),
            global_accuracies: DashMap::new(),
            redis: RwLock::new(None),
        })
    }

    /// Connect to Redis for persistence.
    pub async fn connect_redis(&self, redis_url: &str) {
        match redis::Client::open(redis_url) {
            Ok(client) => match ConnectionManager::new(client).await {
                Ok(conn) => {
                    info!("AccuracyStore connected to Redis");
                    *self.redis.write().await = Some(conn);
                }
                Err(e) => {
                    warn!("Failed to connect AccuracyStore to Redis: {}", e);
                }
            },
            Err(e) => {
                warn!("Invalid Redis URL for AccuracyStore: {}", e);
            }
        }
    }

    /// Record a prediction outcome.
    pub async fn record_outcome(
        &self,
        symbol: &str,
        indicator: &str,
        timeframe: &str,
        outcome: PredictionOutcome,
    ) {
        let symbol_lower = symbol.to_lowercase();
        let key = format!("{}:{}:{}", symbol_lower, indicator, timeframe);
        let global_key = format!("{}:{}", indicator, timeframe);

        // Update symbol-specific accuracy
        {
            let mut entry = self.accuracies.entry(key.clone()).or_insert_with(|| {
                SignalAccuracy::new(
                    indicator.to_string(),
                    symbol.to_uppercase(),
                    timeframe.to_string(),
                )
            });
            entry.record_outcome(outcome);
            debug!(
                "Updated accuracy for {}: {:.1}% ({} total)",
                key, entry.accuracy_pct, entry.total_predictions
            );
        }

        // Update global accuracy
        {
            let mut entry = self
                .global_accuracies
                .entry(global_key.clone())
                .or_insert_with(|| {
                    SignalAccuracy::new(
                        indicator.to_string(),
                        "global".to_string(),
                        timeframe.to_string(),
                    )
                });
            entry.record_outcome(outcome);
        }

        // Persist to Redis
        self.save_accuracy(&key).await;
        self.save_global_accuracy(&global_key).await;
    }

    /// Get accuracy for a specific symbol/indicator/timeframe.
    pub async fn get_accuracy(
        &self,
        indicator: &str,
        symbol: &str,
        timeframe: &str,
    ) -> Option<SignalAccuracy> {
        let key = format!("{}:{}:{}", symbol.to_lowercase(), indicator, timeframe);

        // Try memory first
        if let Some(entry) = self.accuracies.get(&key) {
            return Some(entry.clone());
        }

        // Try to load from Redis
        self.load_accuracy(&key).await;
        self.accuracies.get(&key).map(|e| e.clone())
    }

    /// Get global accuracy for an indicator.
    pub async fn get_global_accuracy(
        &self,
        indicator: &str,
        timeframe: &str,
    ) -> Option<SignalAccuracy> {
        let key = format!("{}:{}", indicator, timeframe);

        // Try memory first
        if let Some(entry) = self.global_accuracies.get(&key) {
            return Some(entry.clone());
        }

        // Try to load from Redis
        self.load_global_accuracy(&key).await;
        self.global_accuracies.get(&key).map(|e| e.clone())
    }

    /// Get all accuracies for a symbol.
    pub fn get_symbol_accuracies(&self, symbol: &str) -> Vec<SignalAccuracy> {
        let symbol_lower = symbol.to_lowercase();
        self.accuracies
            .iter()
            .filter(|entry| entry.key().starts_with(&format!("{}:", symbol_lower)))
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get all global accuracies for an indicator.
    pub fn get_indicator_accuracies(&self, indicator: &str) -> Vec<SignalAccuracy> {
        self.global_accuracies
            .iter()
            .filter(|entry| entry.key().starts_with(&format!("{}:", indicator)))
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Save accuracy to Redis.
    async fn save_accuracy(&self, key: &str) {
        let conn_guard = self.redis.read().await;
        let Some(ref conn) = *conn_guard else {
            return;
        };

        if let Some(entry) = self.accuracies.get(key) {
            if let Ok(json) = serde_json::to_string(entry.value()) {
                let redis_key = format!("{}{}", REDIS_ACCURACY_PREFIX, key);
                let mut conn = conn.clone();
                let _: Result<(), _> = conn.set(&redis_key, json).await;
            }
        }
    }

    /// Save global accuracy to Redis.
    async fn save_global_accuracy(&self, key: &str) {
        let conn_guard = self.redis.read().await;
        let Some(ref conn) = *conn_guard else {
            return;
        };

        if let Some(entry) = self.global_accuracies.get(key) {
            if let Ok(json) = serde_json::to_string(entry.value()) {
                let redis_key = format!("{}global:{}", REDIS_ACCURACY_PREFIX, key);
                let mut conn = conn.clone();
                let _: Result<(), _> = conn.set(&redis_key, json).await;
            }
        }
    }

    /// Load accuracy from Redis.
    async fn load_accuracy(&self, key: &str) {
        let conn_guard = self.redis.read().await;
        let Some(ref conn) = *conn_guard else {
            return;
        };

        let redis_key = format!("{}{}", REDIS_ACCURACY_PREFIX, key);
        let mut conn = conn.clone();

        if let Ok(json) = conn.get::<_, String>(&redis_key).await {
            if let Ok(accuracy) = serde_json::from_str::<SignalAccuracy>(&json) {
                self.accuracies.insert(key.to_string(), accuracy);
            }
        }
    }

    /// Load global accuracy from Redis.
    async fn load_global_accuracy(&self, key: &str) {
        let conn_guard = self.redis.read().await;
        let Some(ref conn) = *conn_guard else {
            return;
        };

        let redis_key = format!("{}global:{}", REDIS_ACCURACY_PREFIX, key);
        let mut conn = conn.clone();

        if let Ok(json) = conn.get::<_, String>(&redis_key).await {
            if let Ok(accuracy) = serde_json::from_str::<SignalAccuracy>(&json) {
                self.global_accuracies.insert(key.to_string(), accuracy);
            }
        }
    }

    /// Load all accuracies from Redis.
    pub async fn load_all_from_redis(&self) {
        let conn_guard = self.redis.read().await;
        let Some(ref conn) = *conn_guard else {
            return;
        };

        let mut conn = conn.clone();
        let pattern = format!("{}*", REDIS_ACCURACY_PREFIX);

        // Scan for keys
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(&pattern)
            .query_async(&mut conn)
            .await
            .unwrap_or_default();

        let mut loaded = 0;
        for key in keys {
            if let Ok(json) = conn.get::<_, String>(&key).await {
                if let Ok(accuracy) = serde_json::from_str::<SignalAccuracy>(&json) {
                    let store_key = key
                        .strip_prefix(REDIS_ACCURACY_PREFIX)
                        .unwrap_or(&key)
                        .to_string();

                    if store_key.starts_with("global:") {
                        let global_key = store_key.strip_prefix("global:").unwrap_or(&store_key);
                        self.global_accuracies
                            .insert(global_key.to_string(), accuracy);
                    } else {
                        self.accuracies.insert(store_key, accuracy);
                    }
                    loaded += 1;
                }
            }
        }

        if loaded > 0 {
            info!("Loaded {} accuracy records from Redis", loaded);
        }
    }
}

impl Default for AccuracyStore {
    fn default() -> Self {
        Self {
            accuracies: DashMap::new(),
            global_accuracies: DashMap::new(),
            redis: RwLock::new(None),
        }
    }
}
