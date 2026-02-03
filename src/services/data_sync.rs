/**
 * Data Sync Service
 *
 * Manages cross-server data synchronization for:
 * - Predictions (signal predictions with outcomes)
 * - User preferences
 *
 * Tracks sync state between this server and peers, calculates
 * ahead/behind counts, and coordinates data exchange.
 */

use crate::services::SqliteStore;
use crate::types::{
    DataCounts, PeerMessage, SignalPrediction, SyncDataType, SyncStatus, UserPreferences,
};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

/// Sync state with a specific peer.
#[derive(Debug, Clone, Default)]
pub struct PeerSyncState {
    /// Peer's reported data counts.
    pub peer_counts: DataCounts,
    /// Our counts when we last compared.
    pub our_counts: DataCounts,
    /// Calculated sync status.
    pub status: SyncStatus,
    /// Whether we're currently syncing with this peer.
    pub syncing: bool,
}

/// Service for managing cross-server data synchronization.
#[derive(Clone)]
pub struct DataSyncService {
    /// Server ID.
    server_id: String,
    /// SQLite store for data access.
    sqlite: Arc<SqliteStore>,
    /// Sync state per peer (peer_id -> state).
    peer_states: Arc<DashMap<String, PeerSyncState>>,
    /// Channel for outgoing sync messages.
    message_tx: Option<broadcast::Sender<PeerMessage>>,
}

impl DataSyncService {
    /// Create a new data sync service.
    pub fn new(server_id: String, sqlite: Arc<SqliteStore>) -> Self {
        Self {
            server_id,
            sqlite,
            peer_states: Arc::new(DashMap::new()),
            message_tx: None,
        }
    }

    /// Set the broadcast channel for sending messages to peers.
    pub fn set_message_channel(&mut self, tx: broadcast::Sender<PeerMessage>) {
        self.message_tx = Some(tx);
    }

    /// Get current local data counts.
    pub fn get_local_counts(&self) -> DataCounts {
        let now = chrono::Utc::now().timestamp_millis();

        // Get prediction counts from SQLite
        let (predictions, latest_prediction_at) = self.get_prediction_stats();

        // Get preferences counts from SQLite
        let (preferences, latest_preference_at) = self.get_preferences_stats();

        DataCounts {
            predictions,
            latest_prediction_at,
            preferences,
            latest_preference_at,
            timestamp: now,
        }
    }

    /// Get prediction statistics from SQLite.
    fn get_prediction_stats(&self) -> (i64, i64) {
        let conn = self.sqlite.get_connection();
        let conn = conn.lock().unwrap();

        // Count total predictions
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM prediction_history", [], |row| row.get(0))
            .unwrap_or(0);

        // Get latest prediction timestamp
        let latest: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(timestamp), 0) FROM prediction_history",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        (count, latest)
    }

    /// Get preferences statistics from SQLite.
    fn get_preferences_stats(&self) -> (i64, i64) {
        let conn = self.sqlite.get_connection();
        let conn = conn.lock().unwrap();

        // Count total preferences
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM user_preferences", [], |row| row.get(0))
            .unwrap_or(0);

        // Get latest preference update timestamp
        let latest: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(updated_at), 0) FROM user_preferences",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        (count, latest)
    }

    /// Handle incoming sync counts from a peer.
    pub fn handle_sync_counts(&self, peer_id: &str, peer_counts: DataCounts) {
        let our_counts = self.get_local_counts();

        // Calculate sync status
        let status = self.calculate_sync_status(&our_counts, &peer_counts);

        // Update peer state
        self.peer_states.insert(
            peer_id.to_string(),
            PeerSyncState {
                peer_counts,
                our_counts,
                status: status.clone(),
                syncing: false,
            },
        );

        debug!(
            "Sync status with {}: predictions ahead={} behind={}, prefs ahead={} behind={}",
            peer_id,
            status.predictions_ahead,
            status.predictions_behind,
            status.preferences_ahead,
            status.preferences_behind
        );

        // If we're behind, request missing data
        if status.predictions_behind > 0 || status.preferences_behind > 0 {
            self.request_missing_data(peer_id, &status);
        }
    }

    /// Calculate sync status between our counts and peer's counts.
    fn calculate_sync_status(&self, ours: &DataCounts, theirs: &DataCounts) -> SyncStatus {
        // For predictions: compare counts and timestamps
        // If they have more recent data, we're behind
        let predictions_ahead = (ours.predictions - theirs.predictions).max(0);
        let predictions_behind = (theirs.predictions - ours.predictions).max(0);

        // For preferences: compare by latest update timestamp
        let preferences_ahead = if ours.latest_preference_at > theirs.latest_preference_at {
            1
        } else {
            0
        };
        let preferences_behind = if theirs.latest_preference_at > ours.latest_preference_at {
            1
        } else {
            0
        };

        SyncStatus {
            predictions_ahead,
            predictions_behind,
            preferences_ahead,
            preferences_behind,
            last_sync_at: chrono::Utc::now().timestamp_millis(),
            syncing: false,
        }
    }

    /// Request missing data from a peer.
    fn request_missing_data(&self, peer_id: &str, status: &SyncStatus) {
        if let Some(ref tx) = self.message_tx {
            // Mark as syncing
            if let Some(mut state) = self.peer_states.get_mut(peer_id) {
                state.syncing = true;
                state.status.syncing = true;
            }

            // Request predictions if behind
            if status.predictions_behind > 0 {
                let our_latest = self
                    .peer_states
                    .get(peer_id)
                    .map(|s| s.our_counts.latest_prediction_at)
                    .unwrap_or(0);

                let msg = PeerMessage::SyncRequest {
                    data_type: SyncDataType::Predictions,
                    since_timestamp: Some(our_latest),
                    limit: Some(1000),
                };

                if tx.send(msg).is_err() {
                    debug!("No receivers for sync request");
                }

                info!(
                    "Requested {} predictions from {} since {}",
                    status.predictions_behind, peer_id, our_latest
                );
            }

            // Request preferences if behind
            if status.preferences_behind > 0 {
                let our_latest = self
                    .peer_states
                    .get(peer_id)
                    .map(|s| s.our_counts.latest_preference_at)
                    .unwrap_or(0);

                let msg = PeerMessage::SyncRequest {
                    data_type: SyncDataType::Preferences,
                    since_timestamp: Some(our_latest),
                    limit: Some(1000),
                };

                if tx.send(msg).is_err() {
                    debug!("No receivers for sync request");
                }

                info!(
                    "Requested preferences from {} since {}",
                    peer_id, our_latest
                );
            }
        }
    }

    /// Handle incoming sync request from a peer.
    pub fn handle_sync_request(
        &self,
        peer_id: &str,
        data_type: SyncDataType,
        since_timestamp: Option<i64>,
        limit: Option<i64>,
    ) {
        let since = since_timestamp.unwrap_or(0);
        let limit = limit.unwrap_or(1000) as usize;

        match data_type {
            SyncDataType::Predictions => {
                self.send_predictions_batch(peer_id, since, limit);
            }
            SyncDataType::Preferences => {
                self.send_preferences_batch(peer_id, since, limit);
            }
            SyncDataType::All => {
                self.send_predictions_batch(peer_id, since, limit);
                self.send_preferences_batch(peer_id, since, limit);
            }
        }
    }

    /// Send batch of predictions to a peer.
    fn send_predictions_batch(&self, _peer_id: &str, since: i64, limit: usize) {
        if let Some(ref tx) = self.message_tx {
            // Get predictions since timestamp
            let predictions = self.get_predictions_since(since, limit);

            if predictions.is_empty() {
                debug!("No predictions to send since {}", since);
                return;
            }

            let predictions_json = serde_json::to_string(&predictions).unwrap_or_default();

            let msg = PeerMessage::PredictionBatch {
                from_id: self.server_id.clone(),
                predictions_json,
                has_more: predictions.len() >= limit,
            };

            if tx.send(msg).is_err() {
                debug!("No receivers for prediction batch");
            }

            info!("Sent {} predictions to peer", predictions.len());
        }
    }

    /// Send batch of preferences to a peer.
    fn send_preferences_batch(&self, _peer_id: &str, since: i64, limit: usize) {
        if let Some(ref tx) = self.message_tx {
            // Get preferences since timestamp
            let preferences = self.get_preferences_since(since, limit);

            if preferences.is_empty() {
                debug!("No preferences to send since {}", since);
                return;
            }

            let preferences_json = serde_json::to_string(&preferences).unwrap_or_default();

            let msg = PeerMessage::PreferencesBatch {
                from_id: self.server_id.clone(),
                preferences_json,
                has_more: preferences.len() >= limit,
            };

            if tx.send(msg).is_err() {
                debug!("No receivers for preferences batch");
            }

            info!("Sent {} preference records to peer", preferences.len());
        }
    }

    /// Get predictions since a timestamp.
    fn get_predictions_since(&self, since: i64, limit: usize) -> Vec<SignalPrediction> {
        let conn = self.sqlite.get_connection();
        let conn = conn.lock().unwrap();

        let mut stmt = conn
            .prepare(
                "SELECT id, symbol, indicator, direction, score, price_at_prediction, timestamp,
                        validated, outcome_5m, outcome_1h, outcome_4h, outcome_24h,
                        price_after_5m, price_after_1h, price_after_4h, price_after_24h
                 FROM prediction_history
                 WHERE timestamp > ?1
                 ORDER BY timestamp ASC
                 LIMIT ?2",
            )
            .ok();

        if let Some(ref mut stmt) = stmt {
            stmt.query_map(rusqlite::params![since, limit as i64], |row| {
                Ok(SignalPrediction {
                    id: row.get(0)?,
                    symbol: row.get(1)?,
                    indicator: row.get(2)?,
                    direction: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or_default(),
                    score: row.get(4)?,
                    price_at_prediction: row.get(5)?,
                    timestamp: row.get(6)?,
                    validated: row.get(7)?,
                    outcome_5m: row.get(8).ok(),
                    outcome_1h: row.get(9).ok(),
                    outcome_4h: row.get(10).ok(),
                    outcome_24h: row.get(11).ok(),
                    price_after_5m: row.get(12).ok(),
                    price_after_1h: row.get(13).ok(),
                    price_after_4h: row.get(14).ok(),
                    price_after_24h: row.get(15).ok(),
                })
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    /// Get preferences since a timestamp.
    fn get_preferences_since(&self, since: i64, limit: usize) -> Vec<(String, UserPreferences)> {
        let conn = self.sqlite.get_connection();
        let conn = conn.lock().unwrap();

        let mut stmt = conn
            .prepare(
                "SELECT user_address, theme, language, performance_level, preferred_server,
                        auto_fastest, onboarding_progress, updated_at
                 FROM user_preferences
                 WHERE updated_at > ?1
                 ORDER BY updated_at ASC
                 LIMIT ?2",
            )
            .ok();

        if let Some(ref mut stmt) = stmt {
            stmt.query_map(rusqlite::params![since, limit as i64], |row| {
                let user_address: String = row.get(0)?;
                let onboarding_str: String = row.get::<_, String>(6).unwrap_or_default();
                let onboarding: Vec<String> =
                    serde_json::from_str(&onboarding_str).unwrap_or_default();

                Ok((
                    user_address,
                    UserPreferences {
                        theme: row.get(1)?,
                        language: row.get(2)?,
                        performance_level: row.get(3)?,
                        preferred_server: row.get(4).ok(),
                        auto_fastest: row.get(5).unwrap_or(false),
                        onboarding_progress: onboarding,
                        updated_at: row.get(7)?,
                    },
                ))
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    /// Handle incoming prediction batch from a peer.
    pub fn handle_prediction_batch(&self, from_id: &str, predictions_json: &str, has_more: bool) {
        let predictions: Vec<SignalPrediction> =
            serde_json::from_str(predictions_json).unwrap_or_default();

        if predictions.is_empty() {
            return;
        }

        let mut imported = 0;
        for prediction in &predictions {
            // Check if we already have this prediction
            if !self.prediction_exists(&prediction.id) {
                if self.sqlite.archive_prediction(prediction).is_ok() {
                    imported += 1;
                }
            }
        }

        info!(
            "Imported {}/{} predictions from {} (has_more={})",
            imported,
            predictions.len(),
            from_id,
            has_more
        );

        // Update sync state
        if let Some(mut state) = self.peer_states.get_mut(from_id) {
            if !has_more {
                state.syncing = false;
                state.status.syncing = false;
                state.status.predictions_behind = 0;
            }
        }
    }

    /// Check if a prediction already exists.
    fn prediction_exists(&self, id: &str) -> bool {
        let conn = self.sqlite.get_connection();
        let conn = conn.lock().unwrap();

        conn.query_row(
            "SELECT 1 FROM prediction_history WHERE id = ?1",
            rusqlite::params![id],
            |_| Ok(true),
        )
        .unwrap_or(false)
    }

    /// Handle incoming preferences batch from a peer.
    pub fn handle_preferences_batch(&self, from_id: &str, preferences_json: &str, has_more: bool) {
        let preferences: Vec<(String, UserPreferences)> =
            serde_json::from_str(preferences_json).unwrap_or_default();

        if preferences.is_empty() {
            return;
        }

        let mut imported = 0;
        for (user_address, prefs) in &preferences {
            // Check if we should update (newer timestamp wins)
            let should_update = self
                .sqlite
                .get_preferences(user_address)
                .map(|existing| prefs.updated_at > existing.updated_at)
                .unwrap_or(true);

            if should_update {
                if self.sqlite.save_preferences(user_address, prefs).is_ok() {
                    imported += 1;
                }
            }
        }

        info!(
            "Imported {}/{} preference records from {} (has_more={})",
            imported,
            preferences.len(),
            from_id,
            has_more
        );

        // Update sync state
        if let Some(mut state) = self.peer_states.get_mut(from_id) {
            if !has_more {
                state.syncing = false;
                state.status.syncing = false;
                state.status.preferences_behind = 0;
            }
        }
    }

    /// Broadcast our current counts to all peers.
    pub fn broadcast_counts(&self) {
        if let Some(ref tx) = self.message_tx {
            let counts = self.get_local_counts();

            let msg = PeerMessage::SyncCounts {
                from_id: self.server_id.clone(),
                counts,
            };

            if tx.send(msg).is_err() {
                debug!("No receivers for sync counts broadcast");
            }
        }
    }

    /// Get sync status for a specific peer.
    pub fn get_peer_sync_status(&self, peer_id: &str) -> Option<SyncStatus> {
        self.peer_states.get(peer_id).map(|s| s.status.clone())
    }

    /// Get sync status for all peers.
    pub fn get_all_sync_status(&self) -> Vec<(String, SyncStatus)> {
        self.peer_states
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().status.clone()))
            .collect()
    }
}
