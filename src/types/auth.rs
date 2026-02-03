/**
 * Authentication Types
 *
 * Types for signature-based authentication.
 * Uses HMAC-SHA256 for signature verification (matching frontend Web Crypto API).
 */

use serde::{Deserialize, Serialize};

/// Challenge issued to clients for authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthChallenge {
    /// Random challenge string to sign
    pub challenge: String,
    /// Timestamp when challenge was issued (ms)
    pub timestamp: i64,
    /// Expiration timestamp (ms) - challenges expire after 5 minutes
    pub expires_at: i64,
}

/// Authentication request from client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthRequest {
    /// Client's public key (hex-encoded)
    pub public_key: String,
    /// The challenge that was signed
    pub challenge: String,
    /// HMAC signature of the challenge (hex-encoded)
    pub signature: String,
    /// Timestamp when signature was created (ms)
    pub timestamp: i64,
}

/// Authentication response on successful verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponse {
    /// Whether authentication was successful
    pub authenticated: bool,
    /// The authenticated public key
    pub public_key: String,
    /// Session token for subsequent requests
    pub session_token: String,
    /// When the session expires (ms)
    pub expires_at: i64,
    /// User's profile (created if new)
    pub profile: Profile,
}

/// User profile stored in database.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// Unique profile ID
    pub id: String,
    /// Public key (primary identifier)
    pub public_key: String,
    /// When account was created (ms)
    pub created_at: i64,
    /// Last authentication time (ms)
    pub last_seen: i64,
    /// User settings
    pub settings: ProfileSettings,
}

/// User settings stored in profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSettings {
    /// Default trading timeframe
    #[serde(default = "default_timeframe")]
    pub default_timeframe: String,
    /// Preferred indicators to show
    #[serde(default)]
    pub preferred_indicators: Vec<String>,
    /// Notification preferences
    #[serde(default)]
    pub notifications_enabled: bool,
    /// Preferred server ID for auto-connect
    #[serde(default)]
    pub preferred_server: Option<String>,
    /// Auto-switch to fastest server
    #[serde(default)]
    pub auto_fastest: bool,
    /// UI theme (dark/light)
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Language code
    #[serde(default = "default_language")]
    pub language: String,
    /// Last updated timestamp for sync conflict resolution
    #[serde(default)]
    pub updated_at: i64,
}

impl Default for ProfileSettings {
    fn default() -> Self {
        Self {
            default_timeframe: "day_trading".to_string(),
            preferred_indicators: Vec::new(),
            notifications_enabled: false,
            preferred_server: None,
            auto_fastest: false,
            theme: "dark".to_string(),
            language: "en".to_string(),
            updated_at: chrono::Utc::now().timestamp_millis(),
        }
    }
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_language() -> String {
    "en".to_string()
}

fn default_timeframe() -> String {
    "day_trading".to_string()
}

/// Session stored in Redis.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    /// Session token
    pub token: String,
    /// Associated public key
    pub public_key: String,
    /// When session was created (ms)
    pub created_at: i64,
    /// When session expires (ms)
    pub expires_at: i64,
}

/// Authenticated user extracted from request.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    /// The user's public key
    pub public_key: String,
    /// The user's profile
    pub profile: Profile,
}

impl Profile {
    /// Create a new profile for a public key.
    pub fn new(public_key: String) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            public_key,
            created_at: now,
            last_seen: now,
            settings: ProfileSettings::default(),
        }
    }
}

impl AuthChallenge {
    /// Create a new challenge with 5-minute expiry.
    pub fn new() -> Self {
        use rand::Rng;
        let now = chrono::Utc::now().timestamp_millis();
        let mut rng = rand::thread_rng();
        let challenge: String = (0..32)
            .map(|_| format!("{:02x}", rng.gen::<u8>()))
            .collect();

        Self {
            challenge,
            timestamp: now,
            expires_at: now + 5 * 60 * 1000, // 5 minutes
        }
    }

    /// Check if challenge has expired.
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now().timestamp_millis() > self.expires_at
    }
}

impl Session {
    /// Create a new session with 24-hour expiry.
    pub fn new(public_key: String) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        let token = uuid::Uuid::new_v4().to_string();

        Self {
            token,
            public_key,
            created_at: now,
            expires_at: now + 24 * 60 * 60 * 1000, // 24 hours
        }
    }

    /// Check if session has expired.
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now().timestamp_millis() > self.expires_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_challenge_creation() {
        let challenge = AuthChallenge::new();
        assert_eq!(challenge.challenge.len(), 64); // 32 bytes = 64 hex chars
        assert!(!challenge.is_expired());
        assert!(challenge.expires_at > challenge.timestamp);
    }

    #[test]
    fn test_session_creation() {
        let session = Session::new("abc123".to_string());
        assert_eq!(session.public_key, "abc123");
        assert!(!session.is_expired());
        assert!(session.expires_at > session.created_at);
    }

    #[test]
    fn test_profile_creation() {
        let profile = Profile::new("pubkey123".to_string());
        assert_eq!(profile.public_key, "pubkey123");
        assert_eq!(profile.settings.default_timeframe, "day_trading");
        assert!(!profile.id.is_empty());
    }
}
