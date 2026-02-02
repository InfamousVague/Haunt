//! SQLite persistence layer for long-term profile and prediction storage.
//!
//! SQLite is used for data that should survive Redis restarts:
//! - User profiles (persist forever)
//! - Prediction history archive (historical accuracy tracking)
//!
//! Redis is still used for:
//! - Sessions (24-hour TTL, ephemeral)
//! - Recent predictions (7-day TTL, quick access)

use crate::types::{Profile, ProfileSettings, SignalPrediction, PredictionOutcome};
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// SQLite store for persistent profile and prediction data.
pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    /// Create a new SQLite store at the given path.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        info!("SQLite store initialized");
        Ok(store)
    }

    /// Create an in-memory SQLite store (for testing).
    pub fn new_in_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        debug!("In-memory SQLite store initialized");
        Ok(store)
    }

    /// Initialize database schema.
    fn init_schema(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        // Profiles table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS profiles (
                id TEXT PRIMARY KEY,
                public_key TEXT UNIQUE NOT NULL,
                created_at INTEGER NOT NULL,
                last_seen INTEGER NOT NULL,
                settings_json TEXT DEFAULT '{}'
            )",
            [],
        )?;

        // Index on public_key for faster lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_profiles_public_key ON profiles(public_key)",
            [],
        )?;

        // Prediction history table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS prediction_history (
                id TEXT PRIMARY KEY,
                symbol TEXT NOT NULL,
                indicator TEXT NOT NULL,
                direction TEXT NOT NULL,
                score INTEGER NOT NULL,
                price_at_prediction REAL NOT NULL,
                timestamp INTEGER NOT NULL,
                price_after_5m REAL,
                price_after_1h REAL,
                price_after_4h REAL,
                price_after_24h REAL,
                outcome_5m TEXT,
                outcome_1h TEXT,
                outcome_4h TEXT,
                outcome_24h TEXT
            )",
            [],
        )?;

        // Indexes for prediction queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_predictions_symbol ON prediction_history(symbol)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_predictions_timestamp ON prediction_history(timestamp DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_predictions_symbol_indicator
             ON prediction_history(symbol, indicator)",
            [],
        )?;

        info!("SQLite schema initialized");
        Ok(())
    }

    // ========== Profile Methods ==========

    /// Get a profile by public key.
    pub fn get_profile(&self, public_key: &str) -> Option<Profile> {
        let conn = self.conn.lock().unwrap();

        let result = conn.query_row(
            "SELECT id, public_key, created_at, last_seen, settings_json
             FROM profiles WHERE public_key = ?1",
            params![public_key],
            |row| {
                let settings_json: String = row.get(4)?;
                let settings: ProfileSettings =
                    serde_json::from_str(&settings_json).unwrap_or_default();

                Ok(Profile {
                    id: row.get(0)?,
                    public_key: row.get(1)?,
                    created_at: row.get(2)?,
                    last_seen: row.get(3)?,
                    settings,
                })
            },
        );

        match result {
            Ok(profile) => Some(profile),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => {
                error!("Error fetching profile: {}", e);
                None
            }
        }
    }

    /// Save or update a profile.
    pub fn save_profile(&self, profile: &Profile) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let settings_json = serde_json::to_string(&profile.settings).unwrap_or_default();

        conn.execute(
            "INSERT INTO profiles (id, public_key, created_at, last_seen, settings_json)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(public_key) DO UPDATE SET
                last_seen = excluded.last_seen,
                settings_json = excluded.settings_json",
            params![
                profile.id,
                profile.public_key,
                profile.created_at,
                profile.last_seen,
                settings_json,
            ],
        )?;

        debug!("Saved profile for {}", &profile.public_key[..16.min(profile.public_key.len())]);
        Ok(())
    }

    /// Update profile's last_seen timestamp.
    pub fn update_last_seen(&self, public_key: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp_millis();

        conn.execute(
            "UPDATE profiles SET last_seen = ?1 WHERE public_key = ?2",
            params![now, public_key],
        )?;

        Ok(())
    }

    /// Delete a profile.
    pub fn delete_profile(&self, public_key: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM profiles WHERE public_key = ?1", params![public_key])?;
        Ok(())
    }

    /// Get total profile count.
    pub fn profile_count(&self) -> usize {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM profiles", [], |row| row.get(0))
            .unwrap_or(0)
    }

    // ========== Prediction History Methods ==========

    /// Archive a prediction to SQLite.
    pub fn archive_prediction(&self, prediction: &SignalPrediction) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO prediction_history
             (id, symbol, indicator, direction, score, price_at_prediction, timestamp,
              price_after_5m, price_after_1h, price_after_4h, price_after_24h,
              outcome_5m, outcome_1h, outcome_4h, outcome_24h)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(id) DO UPDATE SET
                price_after_5m = COALESCE(excluded.price_after_5m, price_after_5m),
                price_after_1h = COALESCE(excluded.price_after_1h, price_after_1h),
                price_after_4h = COALESCE(excluded.price_after_4h, price_after_4h),
                price_after_24h = COALESCE(excluded.price_after_24h, price_after_24h),
                outcome_5m = COALESCE(excluded.outcome_5m, outcome_5m),
                outcome_1h = COALESCE(excluded.outcome_1h, outcome_1h),
                outcome_4h = COALESCE(excluded.outcome_4h, outcome_4h),
                outcome_24h = COALESCE(excluded.outcome_24h, outcome_24h)",
            params![
                prediction.id.to_string(),
                prediction.symbol.to_lowercase(),
                prediction.indicator,
                format!("{:?}", prediction.direction).to_lowercase(),
                prediction.score,
                prediction.price_at_prediction,
                prediction.timestamp,
                prediction.price_after_5m,
                prediction.price_after_1h,
                prediction.price_after_4h,
                prediction.price_after_24h,
                prediction.outcome_5m.as_ref().map(|o| format!("{:?}", o).to_lowercase()),
                prediction.outcome_1h.as_ref().map(|o| format!("{:?}", o).to_lowercase()),
                prediction.outcome_4h.as_ref().map(|o| format!("{:?}", o).to_lowercase()),
                prediction.outcome_24h.as_ref().map(|o| format!("{:?}", o).to_lowercase()),
            ],
        )?;

        debug!("Archived prediction {} for {}", prediction.id, prediction.symbol);
        Ok(())
    }

    /// Get predictions for a symbol with optional filtering.
    pub fn get_predictions(
        &self,
        symbol: &str,
        status: Option<&str>,
        limit: usize,
    ) -> Vec<SignalPrediction> {
        let conn = self.conn.lock().unwrap();
        let symbol_lower = symbol.to_lowercase();

        let query = match status {
            Some("validated") => {
                // Return predictions with ANY validated outcome (5m, 1h, 4h, or 24h)
                "SELECT id, symbol, indicator, direction, score, price_at_prediction, timestamp,
                        price_after_5m, price_after_1h, price_after_4h, price_after_24h,
                        outcome_5m, outcome_1h, outcome_4h, outcome_24h
                 FROM prediction_history
                 WHERE symbol = ?1 AND (outcome_5m IS NOT NULL OR outcome_1h IS NOT NULL OR outcome_4h IS NOT NULL OR outcome_24h IS NOT NULL)
                 ORDER BY timestamp DESC
                 LIMIT ?2"
            }
            Some("pending") => {
                // Return predictions with NO validated outcomes yet
                "SELECT id, symbol, indicator, direction, score, price_at_prediction, timestamp,
                        price_after_5m, price_after_1h, price_after_4h, price_after_24h,
                        outcome_5m, outcome_1h, outcome_4h, outcome_24h
                 FROM prediction_history
                 WHERE symbol = ?1 AND outcome_5m IS NULL AND outcome_1h IS NULL AND outcome_4h IS NULL AND outcome_24h IS NULL
                 ORDER BY timestamp DESC
                 LIMIT ?2"
            }
            _ => {
                "SELECT id, symbol, indicator, direction, score, price_at_prediction, timestamp,
                        price_after_5m, price_after_1h, price_after_4h, price_after_24h,
                        outcome_5m, outcome_1h, outcome_4h, outcome_24h
                 FROM prediction_history
                 WHERE symbol = ?1
                 ORDER BY timestamp DESC
                 LIMIT ?2"
            }
        };

        let mut stmt = match conn.prepare(query) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing prediction query: {}", e);
                return Vec::new();
            }
        };

        let predictions = stmt
            .query_map(params![symbol_lower, limit as i64], |row| {
                let id_str: String = row.get(0)?;
                Ok(SignalPrediction {
                    id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                    symbol: row.get(1)?,
                    indicator: row.get(2)?,
                    direction: parse_direction(&row.get::<_, String>(3)?),
                    score: row.get(4)?,
                    price_at_prediction: row.get(5)?,
                    timestamp: row.get(6)?,
                    validated: row.get::<_, Option<String>>(14)?.is_some(),
                    price_after_5m: row.get(7)?,
                    price_after_1h: row.get(8)?,
                    price_after_4h: row.get(9)?,
                    price_after_24h: row.get(10)?,
                    outcome_5m: row.get::<_, Option<String>>(11)?.map(|s| parse_outcome(&s)),
                    outcome_1h: row.get::<_, Option<String>>(12)?.map(|s| parse_outcome(&s)),
                    outcome_4h: row.get::<_, Option<String>>(13)?.map(|s| parse_outcome(&s)),
                    outcome_24h: row.get::<_, Option<String>>(14)?.map(|s| parse_outcome(&s)),
                })
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();

        predictions
    }

    /// Get all predictions across all symbols (for loading on startup).
    pub fn get_all_predictions(&self, limit: usize) -> Vec<SignalPrediction> {
        let conn = self.conn.lock().unwrap();

        let query = "SELECT id, symbol, indicator, direction, score, price_at_prediction, timestamp,
                            price_after_5m, price_after_1h, price_after_4h, price_after_24h,
                            outcome_5m, outcome_1h, outcome_4h, outcome_24h
                     FROM prediction_history
                     ORDER BY timestamp DESC
                     LIMIT ?1";

        let mut stmt = match conn.prepare(query) {
            Ok(stmt) => stmt,
            Err(e) => {
                error!("Error preparing all predictions query: {}", e);
                return Vec::new();
            }
        };

        let predictions = stmt
            .query_map(params![limit as i64], |row| {
                let id_str: String = row.get(0)?;
                Ok(SignalPrediction {
                    id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                    symbol: row.get(1)?,
                    indicator: row.get(2)?,
                    direction: parse_direction(&row.get::<_, String>(3)?),
                    score: row.get(4)?,
                    price_at_prediction: row.get(5)?,
                    timestamp: row.get(6)?,
                    validated: row.get::<_, Option<String>>(14)?.is_some(),
                    price_after_5m: row.get(7)?,
                    price_after_1h: row.get(8)?,
                    price_after_4h: row.get(9)?,
                    price_after_24h: row.get(10)?,
                    outcome_5m: row.get::<_, Option<String>>(11)?.map(|s| parse_outcome(&s)),
                    outcome_1h: row.get::<_, Option<String>>(12)?.map(|s| parse_outcome(&s)),
                    outcome_4h: row.get::<_, Option<String>>(13)?.map(|s| parse_outcome(&s)),
                    outcome_24h: row.get::<_, Option<String>>(14)?.map(|s| parse_outcome(&s)),
                })
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();

        predictions
    }

    /// Check if connection is available (used by other services).
    pub fn get_connection(&self) -> Option<()> {
        // Just verify the mutex can be locked
        self.conn.lock().ok().map(|_| ())
    }

    /// Get accuracy statistics for a symbol.
    pub fn get_accuracy_stats(&self, symbol: &str, timeframe: &str) -> AccuracyStats {
        let conn = self.conn.lock().unwrap();
        let symbol_lower = symbol.to_lowercase();

        let outcome_col = match timeframe {
            "5m" => "outcome_5m",
            "1h" => "outcome_1h",
            "4h" => "outcome_4h",
            "24h" => "outcome_24h",
            _ => "outcome_1h",
        };

        let query = format!(
            "SELECT
                COUNT(*) as total,
                SUM(CASE WHEN {} = 'correct' THEN 1 ELSE 0 END) as correct,
                SUM(CASE WHEN {} = 'incorrect' THEN 1 ELSE 0 END) as incorrect,
                SUM(CASE WHEN {} = 'neutral' THEN 1 ELSE 0 END) as neutral
             FROM prediction_history
             WHERE symbol = ?1 AND {} IS NOT NULL",
            outcome_col, outcome_col, outcome_col, outcome_col
        );

        let result = conn.query_row(&query, params![symbol_lower], |row| {
            Ok(AccuracyStats {
                total: row.get(0)?,
                correct: row.get(1)?,
                incorrect: row.get(2)?,
                neutral: row.get(3)?,
            })
        });

        result.unwrap_or(AccuracyStats::default())
    }

    /// Get overall accuracy across all symbols.
    pub fn get_global_accuracy(&self, timeframe: &str) -> AccuracyStats {
        let conn = self.conn.lock().unwrap();

        let outcome_col = match timeframe {
            "5m" => "outcome_5m",
            "1h" => "outcome_1h",
            "4h" => "outcome_4h",
            "24h" => "outcome_24h",
            _ => "outcome_1h",
        };

        let query = format!(
            "SELECT
                COUNT(*) as total,
                SUM(CASE WHEN {} = 'correct' THEN 1 ELSE 0 END) as correct,
                SUM(CASE WHEN {} = 'incorrect' THEN 1 ELSE 0 END) as incorrect,
                SUM(CASE WHEN {} = 'neutral' THEN 1 ELSE 0 END) as neutral
             FROM prediction_history
             WHERE {} IS NOT NULL",
            outcome_col, outcome_col, outcome_col, outcome_col
        );

        let result = conn.query_row(&query, [], |row| {
            Ok(AccuracyStats {
                total: row.get(0)?,
                correct: row.get(1)?,
                incorrect: row.get(2)?,
                neutral: row.get(3)?,
            })
        });

        result.unwrap_or(AccuracyStats::default())
    }

    /// Get prediction count for a symbol.
    pub fn prediction_count(&self, symbol: &str) -> usize {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM prediction_history WHERE symbol = ?1",
            params![symbol.to_lowercase()],
            |row| row.get(0),
        )
        .unwrap_or(0)
    }

    /// Clean up old predictions (older than days_to_keep).
    pub fn cleanup_old_predictions(&self, days_to_keep: i64) -> Result<usize, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let cutoff = chrono::Utc::now().timestamp_millis() - (days_to_keep * 24 * 60 * 60 * 1000);

        let count = conn.execute(
            "DELETE FROM prediction_history WHERE timestamp < ?1",
            params![cutoff],
        )?;

        if count > 0 {
            info!("Cleaned up {} old predictions", count);
        }

        Ok(count)
    }
}

/// Accuracy statistics.
#[derive(Debug, Default, Clone)]
pub struct AccuracyStats {
    pub total: i64,
    pub correct: i64,
    pub incorrect: i64,
    pub neutral: i64,
}

impl AccuracyStats {
    /// Calculate accuracy percentage (correct / (correct + incorrect) * 100).
    pub fn accuracy_pct(&self) -> f64 {
        let decidable = self.correct + self.incorrect;
        if decidable == 0 {
            return 0.0;
        }
        (self.correct as f64 / decidable as f64) * 100.0
    }
}

/// Parse direction string to SignalDirection.
fn parse_direction(s: &str) -> crate::types::SignalDirection {
    match s {
        "strong_buy" => crate::types::SignalDirection::StrongBuy,
        "buy" => crate::types::SignalDirection::Buy,
        "sell" => crate::types::SignalDirection::Sell,
        "strong_sell" => crate::types::SignalDirection::StrongSell,
        _ => crate::types::SignalDirection::Neutral,
    }
}

/// Parse outcome string to PredictionOutcome.
fn parse_outcome(s: &str) -> PredictionOutcome {
    match s {
        "correct" => PredictionOutcome::Correct,
        "incorrect" => PredictionOutcome::Incorrect,
        _ => PredictionOutcome::Neutral,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SignalDirection;

    #[test]
    fn test_profile_crud() {
        let store = SqliteStore::new_in_memory().unwrap();

        // Create profile
        let profile = Profile::new("abc123".repeat(8));
        store.save_profile(&profile).unwrap();

        // Read profile
        let loaded = store.get_profile(&profile.public_key).unwrap();
        assert_eq!(loaded.id, profile.id);
        assert_eq!(loaded.public_key, profile.public_key);

        // Update last_seen
        store.update_last_seen(&profile.public_key).unwrap();
        let updated = store.get_profile(&profile.public_key).unwrap();
        assert!(updated.last_seen >= loaded.last_seen);

        // Delete
        store.delete_profile(&profile.public_key).unwrap();
        assert!(store.get_profile(&profile.public_key).is_none());
    }

    #[test]
    fn test_prediction_archive() {
        let store = SqliteStore::new_in_memory().unwrap();

        let prediction = SignalPrediction::new(
            "BTC".to_string(),
            "RSI".to_string(),
            SignalDirection::Buy,
            75,
            50000.0,
        );

        store.archive_prediction(&prediction).unwrap();

        let predictions = store.get_predictions("btc", None, 10);
        assert_eq!(predictions.len(), 1);
        assert_eq!(predictions[0].indicator, "RSI");
    }

    #[test]
    fn test_accuracy_stats() {
        let store = SqliteStore::new_in_memory().unwrap();

        // Add some predictions with outcomes
        for i in 0..10 {
            let mut prediction = SignalPrediction::new(
                "ETH".to_string(),
                "MACD".to_string(),
                SignalDirection::Buy,
                60,
                3000.0,
            );
            prediction.price_after_5m = Some(3010.0);
            prediction.price_after_1h = Some(3050.0);
            prediction.validated = true;

            // 7 correct, 3 incorrect
            if i < 7 {
                prediction.outcome_1h = Some(PredictionOutcome::Correct);
            } else {
                prediction.outcome_1h = Some(PredictionOutcome::Incorrect);
            }

            store.archive_prediction(&prediction).unwrap();
        }

        let stats = store.get_accuracy_stats("eth", "1h");
        assert_eq!(stats.total, 10);
        assert_eq!(stats.correct, 7);
        assert_eq!(stats.incorrect, 3);
        assert!((stats.accuracy_pct() - 70.0).abs() < 0.01);
    }
}
