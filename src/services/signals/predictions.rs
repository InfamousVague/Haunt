//! Prediction recording and validation for accuracy tracking.

use crate::services::SqliteStore;
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
    /// Redis connection for caching.
    redis: RwLock<Option<ConnectionManager>>,
    /// SQLite store for permanent prediction history.
    sqlite: RwLock<Option<Arc<SqliteStore>>>,
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
            sqlite: RwLock::new(None),
        })
    }

    /// Connect SQLite store for permanent persistence.
    pub async fn connect_sqlite(&self, sqlite_store: Arc<SqliteStore>) {
        info!("PredictionStore connected to SQLite");
        *self.sqlite.write().await = Some(sqlite_store);
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

    /// Check if we should create a new prediction for this indicator.
    /// Returns false if there's already a recent unvalidated prediction.
    pub fn should_create_prediction(
        &self,
        symbol: &str,
        indicator: &str,
        cooldown_ms: i64,
    ) -> bool {
        let key = format!("{}:{}", symbol.to_lowercase(), indicator);
        let now = chrono::Utc::now().timestamp_millis();

        if let Some(predictions) = self.predictions.get(&key) {
            // Check if there's a recent unvalidated prediction
            for prediction in predictions.iter().rev() {
                // If prediction is not fully validated and was created within cooldown
                if !prediction.validated && (now - prediction.timestamp) < cooldown_ms {
                    return false;
                }
                // Only check recent predictions
                if (now - prediction.timestamp) >= cooldown_ms {
                    break;
                }
            }
        }

        true
    }

    /// Add a new prediction.
    pub async fn add_prediction(&self, prediction: SignalPrediction) {
        let key = format!(
            "{}:{}",
            prediction.symbol.to_lowercase(),
            prediction.indicator
        );

        // Add to main storage
        let mut entry = self.predictions.entry(key.clone()).or_default();
        entry.push_back(prediction.clone());

        // Trim if too many - but prioritize removing unvalidated predictions first
        while entry.len() > MAX_PREDICTIONS_PER_KEY {
            // Find first unvalidated prediction to remove
            let unvalidated_idx = entry
                .iter()
                .position(|p| !p.validated && p.outcome_24h.is_none());

            if let Some(idx) = unvalidated_idx {
                entry.remove(idx);
            } else {
                // All predictions are validated, remove oldest
                entry.pop_front();
            }
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
            .push(prediction.clone());

        // Archive to SQLite for permanent storage
        if let Some(sqlite) = self.sqlite.read().await.as_ref() {
            if let Err(e) = sqlite.archive_prediction(&prediction) {
                warn!("Failed to archive prediction to SQLite: {}", e);
            }
        }

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
            "5m" => (&self.pending_5m, 300_000i64),      // 5 minutes
            "1h" => (&self.pending_1h, 3_600_000i64),    // 1 hour
            "4h" => (&self.pending_4h, 14_400_000i64),   // 4 hours
            "24h" => (&self.pending_24h, 86_400_000i64), // 24 hours
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

                    // Update in SQLite (clone sqlite ref to avoid holding lock)
                    let sqlite_opt = self.sqlite.read().await.clone();
                    if let Some(ref sqlite) = sqlite_opt {
                        if let Err(e) = sqlite.archive_prediction(&prediction) {
                            warn!("Failed to update prediction in SQLite: {}", e);
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
                    let _: Result<(), _> = conn.set_ex(&key, json, 604800).await;
                    // 7 days TTL
                }
            }
        }
    }

    /// Load all predictions from SQLite on startup.
    pub async fn load_from_sqlite(&self) {
        let sqlite_guard = self.sqlite.read().await;
        let Some(ref sqlite) = *sqlite_guard else {
            warn!("Cannot load predictions: SQLite not connected");
            return;
        };

        // Get all unique symbols from SQLite
        match sqlite.get_connection() {
            Some(c) => c,
            None => {
                warn!("Cannot get SQLite connection");
                return;
            }
        };

        // Query all predictions from SQLite (limit to recent ones for memory)
        let predictions = sqlite.get_all_predictions(500);

        let mut loaded_count = 0;
        let mut pending_count = 0;
        for prediction in predictions {
            let key = format!(
                "{}:{}",
                prediction.symbol.to_lowercase(),
                prediction.indicator
            );
            let symbol_lower = prediction.symbol.to_lowercase();

            // Add to main storage
            let mut entry = self.predictions.entry(key).or_default();
            entry.push_back(prediction.clone());
            loaded_count += 1;

            // Add to pending queues if not yet validated for that timeframe
            if prediction.outcome_5m.is_none() {
                self.pending_5m
                    .entry(symbol_lower.clone())
                    .or_default()
                    .push(prediction.clone());
                pending_count += 1;
            }
            if prediction.outcome_1h.is_none() {
                self.pending_1h
                    .entry(symbol_lower.clone())
                    .or_default()
                    .push(prediction.clone());
            }
            if prediction.outcome_4h.is_none() {
                self.pending_4h
                    .entry(symbol_lower.clone())
                    .or_default()
                    .push(prediction.clone());
            }
            if prediction.outcome_24h.is_none() {
                self.pending_24h
                    .entry(symbol_lower)
                    .or_default()
                    .push(prediction);
            }
        }

        if loaded_count > 0 {
            info!(
                "Loaded {} predictions from SQLite ({} pending validation)",
                loaded_count, pending_count
            );
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
                    let mut entry = self.predictions.entry(store_key.to_string()).or_default();

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
            sqlite: RwLock::new(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SignalDirection;

    fn create_test_prediction(symbol: &str, indicator: &str) -> SignalPrediction {
        SignalPrediction::new(
            symbol.to_string(),
            indicator.to_string(),
            SignalDirection::Buy,
            50,
            50000.0,
        )
    }

    #[tokio::test]
    async fn test_prediction_store_creation() {
        let store = PredictionStore::new();
        assert!(store.predictions.is_empty());
    }

    #[tokio::test]
    async fn test_prediction_store_default() {
        let store = PredictionStore::default();
        assert!(store.predictions.is_empty());
        assert!(store.pending_5m.is_empty());
    }

    #[tokio::test]
    async fn test_add_prediction() {
        let store = PredictionStore::new();
        let prediction = create_test_prediction("BTC", "RSI");

        store.add_prediction(prediction).await;

        let predictions = store.get_predictions("BTC");
        assert_eq!(predictions.len(), 1);
        assert_eq!(predictions[0].symbol, "BTC");
        assert_eq!(predictions[0].indicator, "RSI");
    }

    #[tokio::test]
    async fn test_add_multiple_predictions() {
        let store = PredictionStore::new();

        store
            .add_prediction(create_test_prediction("BTC", "RSI"))
            .await;
        store
            .add_prediction(create_test_prediction("BTC", "MACD"))
            .await;
        store
            .add_prediction(create_test_prediction("ETH", "RSI"))
            .await;

        let btc_predictions = store.get_predictions("BTC");
        assert_eq!(btc_predictions.len(), 2);

        let eth_predictions = store.get_predictions("ETH");
        assert_eq!(eth_predictions.len(), 1);
    }

    #[tokio::test]
    async fn test_get_all_predictions() {
        let store = PredictionStore::new();

        store
            .add_prediction(create_test_prediction("BTC", "RSI"))
            .await;
        store
            .add_prediction(create_test_prediction("ETH", "MACD"))
            .await;
        store
            .add_prediction(create_test_prediction("SOL", "ADX"))
            .await;

        // Empty symbol returns all predictions
        let all_predictions = store.get_predictions("");
        assert_eq!(all_predictions.len(), 3);
    }

    #[tokio::test]
    async fn test_should_create_prediction_no_cooldown() {
        let store = PredictionStore::new();

        // No existing predictions, should allow creation
        assert!(store.should_create_prediction("BTC", "RSI", 60_000));
    }

    #[tokio::test]
    async fn test_should_create_prediction_with_cooldown() {
        let store = PredictionStore::new();
        let prediction = create_test_prediction("BTC", "RSI");

        store.add_prediction(prediction).await;

        // Just added, should not allow another within cooldown (60 seconds)
        assert!(!store.should_create_prediction("BTC", "RSI", 60_000));

        // Different indicator should be allowed
        assert!(store.should_create_prediction("BTC", "MACD", 60_000));

        // Different symbol should be allowed
        assert!(store.should_create_prediction("ETH", "RSI", 60_000));
    }

    #[tokio::test]
    async fn test_get_pending_symbols() {
        let store = PredictionStore::new();

        store
            .add_prediction(create_test_prediction("BTC", "RSI"))
            .await;
        store
            .add_prediction(create_test_prediction("ETH", "MACD"))
            .await;
        store
            .add_prediction(create_test_prediction("BTC", "ADX"))
            .await;

        let symbols = store.get_pending_symbols();
        assert!(symbols.contains(&"btc".to_string()));
        assert!(symbols.contains(&"eth".to_string()));
    }

    #[tokio::test]
    async fn test_get_pending() {
        let store = PredictionStore::new();
        let prediction = create_test_prediction("BTC", "RSI");

        store.add_prediction(prediction).await;

        // All timeframes should have pending
        let pending_5m = store.get_pending("BTC", "5m");
        assert_eq!(pending_5m.len(), 1);

        let pending_1h = store.get_pending("BTC", "1h");
        assert_eq!(pending_1h.len(), 1);

        let pending_4h = store.get_pending("BTC", "4h");
        assert_eq!(pending_4h.len(), 1);

        let pending_24h = store.get_pending("BTC", "24h");
        assert_eq!(pending_24h.len(), 1);
    }

    #[tokio::test]
    async fn test_get_pending_invalid_timeframe() {
        let store = PredictionStore::new();
        store
            .add_prediction(create_test_prediction("BTC", "RSI"))
            .await;

        let pending = store.get_pending("BTC", "invalid");
        assert!(pending.is_empty());
    }

    #[tokio::test]
    async fn test_predictions_sorted_by_timestamp() {
        let store = PredictionStore::new();

        // Create predictions with slightly different timestamps
        let mut pred1 = create_test_prediction("BTC", "RSI");
        pred1.timestamp = 1000;
        let mut pred2 = create_test_prediction("BTC", "MACD");
        pred2.timestamp = 3000;
        let mut pred3 = create_test_prediction("BTC", "ADX");
        pred3.timestamp = 2000;

        store.add_prediction(pred1).await;
        store.add_prediction(pred2).await;
        store.add_prediction(pred3).await;

        let predictions = store.get_predictions("BTC");
        // Should be sorted descending by timestamp
        assert_eq!(predictions[0].timestamp, 3000);
        assert_eq!(predictions[1].timestamp, 2000);
        assert_eq!(predictions[2].timestamp, 1000);
    }

    #[tokio::test]
    async fn test_case_insensitive_symbol() {
        let store = PredictionStore::new();
        let prediction = create_test_prediction("BTC", "RSI");

        store.add_prediction(prediction).await;

        // Should find with lowercase
        let predictions = store.get_predictions("btc");
        assert_eq!(predictions.len(), 1);

        // Should find with uppercase
        let predictions = store.get_predictions("BTC");
        assert_eq!(predictions.len(), 1);
    }
}
