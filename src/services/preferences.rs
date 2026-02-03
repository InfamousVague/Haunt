/**
 * Preferences Service
 *
 * Manages user preferences with storage and cross-server sync.
 *
 * Storage:
 * - DashMap: In-memory cache for fast access
 * - SQLite: Persistent storage
 * - Mesh broadcast: Cross-server replication
 */

use crate::services::{AuthError, SqliteStore};
use crate::types::{PartialPreferences, PreferencesSyncResponse, UserPreferences};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

/// Message for preference updates via mesh.
#[derive(Debug, Clone)]
pub struct PreferencesUpdate {
    /// User's public key / address
    pub user_address: String,
    /// Updated preferences
    pub preferences: UserPreferences,
    /// Signature to verify authenticity
    pub signature: String,
}

/// Preferences service for managing user settings.
#[derive(Clone)]
pub struct PreferencesService {
    /// In-memory cache (user_address -> preferences)
    cache: Arc<DashMap<String, UserPreferences>>,
    /// SQLite store for persistence
    sqlite: Option<Arc<SqliteStore>>,
    /// Broadcast channel for mesh replication
    broadcast_tx: Option<broadcast::Sender<PreferencesUpdate>>,
}

impl PreferencesService {
    /// Create a new preferences service.
    pub fn new(sqlite: Option<Arc<SqliteStore>>) -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
            sqlite,
            broadcast_tx: None,
        }
    }

    /// Set the broadcast channel for mesh replication.
    pub fn set_broadcast(&mut self, tx: broadcast::Sender<PreferencesUpdate>) {
        self.broadcast_tx = Some(tx);
    }

    /// Get a subscription to preference updates.
    pub fn subscribe(&self) -> Option<broadcast::Receiver<PreferencesUpdate>> {
        self.broadcast_tx.as_ref().map(|tx| tx.subscribe())
    }

    /// Get user preferences.
    pub async fn get_preferences(&self, user_address: &str) -> UserPreferences {
        // Check cache first
        if let Some(prefs) = self.cache.get(user_address) {
            return prefs.clone();
        }

        // Check SQLite
        if let Some(ref sqlite) = self.sqlite {
            if let Some(prefs) = sqlite.get_preferences(user_address) {
                self.cache.insert(user_address.to_string(), prefs.clone());
                return prefs;
            }
        }

        // Return defaults
        let prefs = UserPreferences::new();
        self.cache.insert(user_address.to_string(), prefs.clone());
        prefs
    }

    /// Update user preferences.
    pub async fn update_preferences(
        &self,
        user_address: &str,
        partial: PartialPreferences,
    ) -> Result<UserPreferences, AuthError> {
        let mut prefs = self.get_preferences(user_address).await;
        partial.apply_to(&mut prefs);

        // Save to cache
        self.cache.insert(user_address.to_string(), prefs.clone());

        // Persist to SQLite
        if let Some(ref sqlite) = self.sqlite {
            sqlite.save_preferences(user_address, &prefs).map_err(|e| {
                warn!("Failed to save preferences: {}", e);
                AuthError::ProfileNotFound
            })?;
        }

        // Broadcast to mesh
        self.broadcast_update(user_address, &prefs);

        debug!("Updated preferences for {}", &user_address[..16.min(user_address.len())]);
        Ok(prefs)
    }

    /// Sync preferences with client.
    /// Uses timestamp-based conflict resolution.
    pub async fn sync_preferences(
        &self,
        user_address: &str,
        client_prefs: UserPreferences,
    ) -> Result<PreferencesSyncResponse, AuthError> {
        let server_prefs = self.get_preferences(user_address).await;

        let (merged, server_updated, client_should_update) = if client_prefs.updated_at > server_prefs.updated_at {
            // Client is newer - update server
            self.cache.insert(user_address.to_string(), client_prefs.clone());

            if let Some(ref sqlite) = self.sqlite {
                sqlite.save_preferences(user_address, &client_prefs).map_err(|e| {
                    warn!("Failed to save preferences: {}", e);
                    AuthError::ProfileNotFound
                })?;
            }

            self.broadcast_update(user_address, &client_prefs);

            (client_prefs, true, false)
        } else if server_prefs.updated_at > client_prefs.updated_at {
            // Server is newer - client should update
            (server_prefs, false, true)
        } else {
            // Same timestamp - no changes needed
            (server_prefs, false, false)
        };

        info!(
            "Synced preferences for {}: server_updated={}, client_should_update={}",
            &user_address[..16.min(user_address.len())],
            server_updated,
            client_should_update
        );

        Ok(PreferencesSyncResponse {
            preferences: merged,
            server_updated,
            client_should_update,
        })
    }

    /// Handle incoming preference update from mesh.
    pub async fn handle_mesh_update(&self, update: PreferencesUpdate) {
        // Verify signature would go here in production

        let current = self.get_preferences(&update.user_address).await;

        // Only update if incoming is newer
        if update.preferences.updated_at > current.updated_at {
            self.cache.insert(update.user_address.clone(), update.preferences.clone());

            if let Some(ref sqlite) = self.sqlite {
                let _ = sqlite.save_preferences(&update.user_address, &update.preferences);
            }

            debug!(
                "Applied mesh preference update for {}",
                &update.user_address[..16.min(update.user_address.len())]
            );
        }
    }

    /// Broadcast preference update to mesh.
    fn broadcast_update(&self, user_address: &str, prefs: &UserPreferences) {
        if let Some(ref tx) = self.broadcast_tx {
            let update = PreferencesUpdate {
                user_address: user_address.to_string(),
                preferences: prefs.clone(),
                signature: String::new(), // Would be signed by user in production
            };

            if tx.send(update).is_err() {
                debug!("No mesh receivers for preference update");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_default_preferences() {
        let service = PreferencesService::new(None);
        let prefs = service.get_preferences("test_user").await;

        assert_eq!(prefs.theme, "dark");
        assert_eq!(prefs.language, "en");
    }

    #[tokio::test]
    async fn test_update_preferences() {
        let service = PreferencesService::new(None);

        let partial = PartialPreferences {
            theme: Some("light".to_string()),
            ..Default::default()
        };

        let prefs = service.update_preferences("test_user", partial).await.unwrap();
        assert_eq!(prefs.theme, "light");

        // Should persist in cache
        let cached = service.get_preferences("test_user").await;
        assert_eq!(cached.theme, "light");
    }

    #[tokio::test]
    async fn test_sync_client_newer() {
        let service = PreferencesService::new(None);

        // Set server preferences
        let _ = service.update_preferences("test_user", PartialPreferences {
            theme: Some("dark".to_string()),
            ..Default::default()
        }).await;

        // Client has newer preferences
        let mut client_prefs = UserPreferences::new();
        client_prefs.theme = "light".to_string();
        client_prefs.updated_at = chrono::Utc::now().timestamp_millis() + 1000;

        let response = service.sync_preferences("test_user", client_prefs).await.unwrap();

        assert!(response.server_updated);
        assert!(!response.client_should_update);
        assert_eq!(response.preferences.theme, "light");
    }
}
