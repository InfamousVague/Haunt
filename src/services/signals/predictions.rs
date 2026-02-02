//! Prediction recording and validation for accuracy tracking.

use crate::types::{PredictionOutcome, SignalPrediction};
use dashmap::DashMap;
use redis::{aio::ConnectionManager, AsyncCommands};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Maximum predictions to keep per symbol/indicator pair.
const MAX_PREDICTIONS_PER_KEY: usize = 100;

/// Redis key prefix for predictions.
const REDIS_PREDICTIONS_PREFIX: &str = "haunt:predictions:";

/// Store for recording and validating signal predictions.
pub struct PredictionStore {
    /// In-memory prediction storage: key = "{symbol}:{indicator}"
    predictions: DashMap<String, VecDeque<SignalPrediction>>,
    /// Pending validation queue: predictions awaiting validation
    pending_5m: DashMap<String, Vec<SignalPrediction>>,
    pending_1h: DashMap<String, Vec<SignalPrediction>>,
    pending_4h: DashMap<String, Vec<SignalPrediction>>,
    pending_24h: DashMap<String, Vec<SignalPrediction>>,
    /// Redis connection for persistence.
    redis: RwLock<Option<ConnectionManager>>,
}

impl PredictionStore {
    /// Create a new prediction store.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            predictions: DashMap::new(),
            pending_5m: DashMap::new(),
            pending_1h: DashMap::new(),
            pending_4h: DashMap::new(),
            pending_24h: DashMap::new(),
            redis: RwLock::new(None),
        })
    }

    /// Connect to Redis for persistence.
    pub async fn connect_redis(&self, redis_url: &str) {
        match redis::Client::open(redis_url) {
            Ok(client) => match ConnectionManager::new(client).await {
                Ok(conn) => {
                    info!("PredictionStore connected to Redis");
                    *self.redis.write().await = Some(conn);
                }
                Err(e) => {
                    warn!("Failed to connect PredictionStore to Redis: {}", e);
                }
            },
            Err(e) => {
                warn!("Invalid Redis URL for PredictionStore: {}", e);
            }
        }
    }

    /// Add a new prediction.
    pub async fn add_prediction(&self, prediction: SignalPrediction) {
        let key = format!("{}:{}", prediction.symbol.to_lowercase(), prediction.indicator);

        // Add to main storage
        let mut entry = self.predictions.entry(key.clone()).or_insert_with(VecDeque::new);
        entry.push_back(prediction.clone());

        // Trim if too many
        while entry.len() > MAX_PREDICTIONS_PER_KEY {
            entry.pop_front();
        }

        // Add to pending validation queues
        self.pending_5m
            .entry(prediction.symbol.to_lowercase())
            .or_default()
            .push(prediction.clone());
        self.pending_1h
            .entry(prediction.symbol.to_lowercase())
            .or_default()
            .push(prediction.clone());
        self.pending_4h
            .entry(prediction.symbol.to_lowercase())
            .or_default()
            .push(prediction.clone());
        self.pending_24h
            .entry(prediction.symbol.to_lowercase())
            .or_default()
            .push(prediction);

        debug!("Added prediction for {}", key);
    }

    /// Get predictions for a symbol. If symbol is empty, returns all predictions.
    pub fn get_predictions(&self, symbol: &str) -> Vec<SignalPrediction> {
        let symbol_lower = symbol.to_lowercase();
        let mut all_predictions = Vec::new();

        for entry in self.predictions.iter() {
            // If symbol is empty, get all predictions; otherwise filter by symbol prefix
            if symbol_lower.is_empty() || entry.key().starts_with(&format!("{}:", symbol_lower)) {
                all_predictions.extend(entry.value().iter().cloned());
            }
        }

        // Sort by timestamp descending
        all_predictions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        all_predictions
    }

    /// Get all unique symbols with pending predictions.
    pub fn get_pending_symbols(&self) -> Vec<String> {
        let mut symbols: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Collect from all pending queues
        for entry in self.pending_5m.iter() {
            symbols.insert(entry.key().clone());
        }
        for entry in self.pending_1h.iter() {
            symbols.insert(entry.key().clone());
        }
        for entry in self.pending_4h.iter() {
            symbols.insert(entry.key().clone());
        }
        for entry in self.pending_24h.iter() {
            symbols.insert(entry.key().clone());
        }

        symbols.into_iter().collect()
    }

    /// Get predictions pending validation for a timeframe.
    pub fn get_pending(&self, symbol: &str, timeframe: &str) -> Vec<SignalPrediction> {
        let symbol_lower = symbol.to_lowercase();
        let queue = match timeframe {
            "5m" => &self.pending_5m,
            "1h" => &self.pending_1h,
            "4h" => &self.pending_4h,
            "24h" => &self.pending_24h,
            _ => return Vec::new(),
        };

        queue
            .get(&symbol_lower)
            .map(|v| v.clone())
            .unwrap_or_default()
    }

    /// Validate pending predictions and return outcomes.
    pub async fn validate_pending(
        &self,
        symbol: &str,
        current_price: f64,
        timeframe: &str,
    ) -> Vec<(String, PredictionOutcome)> {
        let symbol_lower = symbol.to_lowercase();
        let now = chrono::Utc::now().timestamp_millis();

        let (queue, threshold_ms) = match timeframe {
            "5m" => (&self.pending_5m, 300_000i64),    // 5 minutes
            "1h" => (&self.pending_1h, 3600_000i64),   // 1 hour
            "4h" => (&self.pending_4h, 14400_000i64),  // 4 hours
            "24h" => (&self.pending_24h, 86400_000i64), // 24 hours
            _ => return Vec::new(),
        };

        let mut outcomes = Vec::new();

        if let Some(mut pending) = queue.get_mut(&symbol_lower) {
            let mut remaining = Vec::new();

            for mut prediction in pending.drain(..) {
                let age = now - prediction.timestamp;

                if age >= threshold_ms {
                    // Time to validate
                    let outcome = prediction.validate(current_price, timeframe);
                    outcomes.push((prediction.indicator.clone(), outcome));

                    // Update in main storage
                    let key = format!("{}:{}", symbol_lower, prediction.indicator);
                    if let Some(mut entry) = self.predictions.get_mut(&key) {
                        for stored in entry.iter_mut() {
                            if stored.id == prediction.id {
                                *stored = prediction.clone();
                                break;
                            }
                        }
                    }

                    debug!(
                        "Validated {} prediction for {}: {:?}",
                        timeframe, prediction.indicator, outcome
                    );
                } else {
                    // Keep for later
                    remaining.push(prediction);
                }
            }

            *pending = remaining;
        }

        outcomes
    }

    /// Save predictions to Redis.
    pub async fn save_to_redis(&self, symbol: &str) {
        let conn_guard = self.redis.read().await;
        let Some(ref conn) = *conn_guard else {
            return;
        };

        let symbol_lower = symbol.to_lowercase();
        let mut conn = conn.clone();

        for entry in self.predictions.iter() {
            if entry.key().starts_with(&format!("{}:", symbol_lower)) {
                let key = format!("{}{}", REDIS_PREDICTIONS_PREFIX, entry.key());

                // Serialize predictions
                if let Ok(json) = serde_json::to_string(&entry.value().iter().collect::<Vec<_>>()) {
                    let _: Result<(), _> = conn.set_ex(&key, json, 604800).await; // 7 days TTL
                }
            }
        }
    }

    /// Load predictions from Redis.
    pub async fn load_from_redis(&self, symbol: &str) {
        let conn_guard = self.redis.read().await;
        let Some(ref conn) = *conn_guard else {
            return;
        };

        let symbol_lower = symbol.to_lowercase();
        let mut conn = conn.clone();
        let pattern = format!("{}{}:*", REDIS_PREDICTIONS_PREFIX, symbol_lower);

        // Scan for matching keys
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(&pattern)
            .query_async(&mut conn)
            .await
            .unwrap_or_default();

        for key in keys {
            if let Ok(json) = conn.get::<_, String>(&key).await {
                if let Ok(predictions) = serde_json::from_str::<Vec<SignalPrediction>>(&json) {
                    let store_key = key.strip_prefix(REDIS_PREDICTIONS_PREFIX).unwrap_or(&key);
                    let mut entry = self
                        .predictions
                        .entry(store_key.to_string())
                        .or_insert_with(VecDeque::new);

                    for prediction in predictions {
                        entry.push_back(prediction);
                    }

                    debug!("Loaded {} predictions for {}", entry.len(), store_key);
                }
            }
        }
    }
}

impl Default for PredictionStore {
    fn default() -> Self {
        Self {
            predictions: DashMap::new(),
            pending_5m: DashMap::new(),
            pending_1h: DashMap::new(),
            pending_4h: DashMap::new(),
            pending_24h: DashMap::new(),
            redis: RwLock::new(None),
        }
    }
}
