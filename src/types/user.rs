/**
 * User Types
 *
 * Types for user preferences and cross-server sync.
 */

use serde::{Deserialize, Serialize};

/// User preferences that sync across servers.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserPreferences {
    /// UI theme preference
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Language preference (e.g., "en", "ko", "ja")
    #[serde(default = "default_language")]
    pub language: String,

    /// Performance level for UI updates
    #[serde(default = "default_performance_level")]
    pub performance_level: String,

    /// Preferred server ID for auto-connect
    #[serde(default)]
    pub preferred_server: Option<String>,

    /// Whether to auto-switch to fastest server
    #[serde(default)]
    pub auto_fastest: bool,

    /// Completed onboarding hint IDs
    #[serde(default)]
    pub onboarding_progress: Vec<String>,

    /// Last update timestamp (ms) for conflict resolution
    #[serde(default)]
    pub updated_at: i64,
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_language() -> String {
    "en".to_string()
}

fn default_performance_level() -> String {
    "balanced".to_string()
}

impl UserPreferences {
    /// Create new preferences with defaults.
    pub fn new() -> Self {
        Self {
            theme: default_theme(),
            language: default_language(),
            performance_level: default_performance_level(),
            preferred_server: None,
            auto_fastest: false,
            onboarding_progress: Vec::new(),
            updated_at: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// Merge with another preferences object.
    /// Uses `updated_at` timestamp for conflict resolution - newer wins per field.
    pub fn merge(&mut self, other: &UserPreferences) {
        // If the other preferences are newer overall, take them
        if other.updated_at > self.updated_at {
            self.theme = other.theme.clone();
            self.language = other.language.clone();
            self.performance_level = other.performance_level.clone();
            self.preferred_server = other.preferred_server.clone();
            self.auto_fastest = other.auto_fastest;
            self.onboarding_progress = other.onboarding_progress.clone();
            self.updated_at = other.updated_at;
        }
    }

    /// Update the timestamp to now.
    pub fn touch(&mut self) {
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }
}

/// Request to update user preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePreferencesRequest {
    /// Partial preferences to update
    #[serde(flatten)]
    pub preferences: PartialPreferences,
}

/// Partial preferences for updates.
/// All fields are optional - only provided fields will be updated.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PartialPreferences {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub performance_level: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_server: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_fastest: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub onboarding_progress: Option<Vec<String>>,
}

impl PartialPreferences {
    /// Apply partial preferences to full preferences.
    pub fn apply_to(&self, prefs: &mut UserPreferences) {
        if let Some(ref theme) = self.theme {
            prefs.theme = theme.clone();
        }
        if let Some(ref language) = self.language {
            prefs.language = language.clone();
        }
        if let Some(ref level) = self.performance_level {
            prefs.performance_level = level.clone();
        }
        if let Some(ref server) = self.preferred_server {
            prefs.preferred_server = Some(server.clone());
        }
        if let Some(auto) = self.auto_fastest {
            prefs.auto_fastest = auto;
        }
        if let Some(ref progress) = self.onboarding_progress {
            prefs.onboarding_progress = progress.clone();
        }
        prefs.touch();
    }
}

/// Response from preferences sync endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferencesSyncResponse {
    /// Merged preferences after sync
    pub preferences: UserPreferences,
    /// Whether server preferences were updated
    pub server_updated: bool,
    /// Whether client should update its preferences
    pub client_should_update: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preferences_default() {
        let prefs = UserPreferences::new();
        assert_eq!(prefs.theme, "dark");
        assert_eq!(prefs.language, "en");
        assert_eq!(prefs.performance_level, "balanced");
        assert!(prefs.preferred_server.is_none());
        assert!(!prefs.auto_fastest);
        assert!(prefs.onboarding_progress.is_empty());
    }

    #[test]
    fn test_preferences_merge() {
        let mut older = UserPreferences::new();
        older.theme = "light".to_string();
        older.updated_at = 1000;

        let mut newer = UserPreferences::new();
        newer.theme = "dark".to_string();
        newer.language = "ko".to_string();
        newer.updated_at = 2000;

        older.merge(&newer);
        assert_eq!(older.theme, "dark");
        assert_eq!(older.language, "ko");
        assert_eq!(older.updated_at, 2000);
    }

    #[test]
    fn test_partial_apply() {
        let mut prefs = UserPreferences::new();
        let partial = PartialPreferences {
            theme: Some("light".to_string()),
            auto_fastest: Some(true),
            ..Default::default()
        };

        partial.apply_to(&mut prefs);
        assert_eq!(prefs.theme, "light");
        assert!(prefs.auto_fastest);
        assert_eq!(prefs.language, "en"); // Unchanged
    }
}
