//! Authentication Types
//!
//! Types for signature-based authentication.
//! Uses HMAC-SHA256 for signature verification (matching frontend Web Crypto API).

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
    /// Auto-generated username (e.g., "CryptoWolf42")
    pub username: String,
    /// When account was created (ms)
    pub created_at: i64,
    /// Last authentication time (ms)
    pub last_seen: i64,
    /// Whether to show on public leaderboard (opt-in)
    #[serde(default)]
    pub show_on_leaderboard: bool,
    /// Signature proving consent to show on leaderboard
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leaderboard_signature: Option<String>,
    /// Timestamp when leaderboard consent was given
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leaderboard_consent_at: Option<i64>,
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
}

impl Default for ProfileSettings {
    fn default() -> Self {
        Self {
            default_timeframe: "day_trading".to_string(),
            preferred_indicators: Vec::new(),
            notifications_enabled: false,
        }
    }
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
#[allow(dead_code)]
pub struct AuthenticatedUser {
    /// The user's public key
    pub public_key: String,
    /// The user's profile
    pub profile: Profile,
}

impl Profile {
    /// Create a new profile for a public key with a given username.
    pub fn new(public_key: String, username: String) -> Self {
        let now = chrono::Utc::now().timestamp_millis();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            public_key,
            username,
            created_at: now,
            last_seen: now,
            show_on_leaderboard: false,
            leaderboard_signature: None,
            leaderboard_consent_at: None,
            settings: ProfileSettings::default(),
        }
    }
}

impl Default for AuthChallenge {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthChallenge {
    /// Create a new challenge with 5-minute expiry.
    pub fn new() -> Self {
        use rand::Rng;
        let timestamp = chrono::Utc::now().timestamp_millis();
        let mut rng = rand::thread_rng();
        let challenge: String = (0..32)
            .map(|_| format!("{:02x}", rng.gen::<u8>()))
            .collect();

        Self {
            challenge,
            timestamp,
            expires_at: timestamp + 5 * 60 * 1000, // 5 minutes
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

    // =========================================================================
    // AuthChallenge Tests
    // =========================================================================

    #[test]
    fn test_challenge_creation() {
        let challenge = AuthChallenge::new();
        assert_eq!(challenge.challenge.len(), 64); // 32 bytes = 64 hex chars
        assert!(!challenge.is_expired());
        assert!(challenge.expires_at > challenge.timestamp);
    }

    #[test]
    fn test_challenge_expiry_window() {
        let challenge = AuthChallenge::new();
        // Expiry should be 5 minutes (300,000 ms) from timestamp
        let expected_expiry = challenge.timestamp + 5 * 60 * 1000;
        assert_eq!(challenge.expires_at, expected_expiry);
    }

    #[test]
    fn test_challenge_serialization() {
        let challenge = AuthChallenge {
            challenge: "abc123".to_string(),
            timestamp: 1704067200000,
            expires_at: 1704067500000,
        };

        let json = serde_json::to_string(&challenge).unwrap();
        assert!(json.contains("\"challenge\":\"abc123\""));
        assert!(json.contains("\"expiresAt\":1704067500000"));

        let parsed: AuthChallenge = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.challenge, "abc123");
    }

    // =========================================================================
    // AuthRequest Tests
    // =========================================================================

    #[test]
    fn test_auth_request_creation() {
        let request = AuthRequest {
            public_key: "pubkey123".to_string(),
            challenge: "challenge456".to_string(),
            signature: "sig789".to_string(),
            timestamp: 1704067200000,
        };

        assert_eq!(request.public_key, "pubkey123");
        assert_eq!(request.challenge, "challenge456");
        assert_eq!(request.signature, "sig789");
    }

    #[test]
    fn test_auth_request_serialization() {
        let request = AuthRequest {
            public_key: "pk".to_string(),
            challenge: "ch".to_string(),
            signature: "sig".to_string(),
            timestamp: 1704067200000,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"publicKey\":\"pk\""));
        assert!(json.contains("\"signature\":\"sig\""));
    }

    // =========================================================================
    // Session Tests
    // =========================================================================

    #[test]
    fn test_session_creation() {
        let session = Session::new("abc123".to_string());
        assert_eq!(session.public_key, "abc123");
        assert!(!session.is_expired());
        assert!(session.expires_at > session.created_at);
    }

    #[test]
    fn test_session_expiry_window() {
        let session = Session::new("test".to_string());
        // Session should expire in 24 hours (86,400,000 ms)
        let expected_duration = 24 * 60 * 60 * 1000;
        let actual_duration = session.expires_at - session.created_at;
        assert_eq!(actual_duration, expected_duration);
    }

    #[test]
    fn test_session_token_is_uuid() {
        let session = Session::new("test".to_string());
        // UUID format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
        assert_eq!(session.token.len(), 36);
        assert!(session.token.contains('-'));
    }

    #[test]
    fn test_session_serialization() {
        let session = Session {
            token: "token123".to_string(),
            public_key: "pk".to_string(),
            created_at: 1704067200000,
            expires_at: 1704153600000,
        };

        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("\"token\":\"token123\""));
        assert!(json.contains("\"publicKey\":\"pk\""));
    }

    // =========================================================================
    // Profile Tests
    // =========================================================================

    #[test]
    fn test_profile_creation() {
        let profile = Profile::new("pubkey123".to_string(), "TestUser123".to_string());
        assert_eq!(profile.public_key, "pubkey123");
        assert_eq!(profile.settings.default_timeframe, "day_trading");
        assert!(!profile.id.is_empty());
    }

    #[test]
    fn test_profile_id_is_uuid() {
        let profile = Profile::new("test".to_string(), "TestUser".to_string());
        assert_eq!(profile.id.len(), 36);
        assert!(profile.id.contains('-'));
    }

    #[test]
    fn test_profile_timestamps() {
        let before = chrono::Utc::now().timestamp_millis();
        let profile = Profile::new("test".to_string(), "TestUser".to_string());
        let after = chrono::Utc::now().timestamp_millis();

        assert!(profile.created_at >= before);
        assert!(profile.created_at <= after);
        assert_eq!(profile.created_at, profile.last_seen);
    }

    #[test]
    fn test_profile_serialization() {
        let profile = Profile {
            id: "id123".to_string(),
            public_key: "pk".to_string(),
            username: "TestUser".to_string(),
            created_at: 1704067200000,
            last_seen: 1704067200000,
            show_on_leaderboard: false,
            leaderboard_signature: None,
            leaderboard_consent_at: None,
            settings: ProfileSettings::default(),
        };

        let json = serde_json::to_string(&profile).unwrap();
        assert!(json.contains("\"publicKey\":\"pk\""));
        assert!(json.contains("\"createdAt\":1704067200000"));
    }

    // =========================================================================
    // ProfileSettings Tests
    // =========================================================================

    #[test]
    fn test_profile_settings_default() {
        let settings = ProfileSettings::default();
        assert_eq!(settings.default_timeframe, "day_trading");
        assert!(settings.preferred_indicators.is_empty());
        assert!(!settings.notifications_enabled);
    }

    #[test]
    fn test_profile_settings_custom() {
        let settings = ProfileSettings {
            default_timeframe: "swing_trading".to_string(),
            preferred_indicators: vec!["RSI".to_string(), "MACD".to_string()],
            notifications_enabled: true,
        };

        assert_eq!(settings.default_timeframe, "swing_trading");
        assert_eq!(settings.preferred_indicators.len(), 2);
        assert!(settings.notifications_enabled);
    }

    #[test]
    fn test_profile_settings_serialization() {
        let settings = ProfileSettings {
            default_timeframe: "scalping".to_string(),
            preferred_indicators: vec!["RSI".to_string()],
            notifications_enabled: true,
        };

        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("\"defaultTimeframe\":\"scalping\""));
        assert!(json.contains("\"preferredIndicators\":[\"RSI\"]"));
        assert!(json.contains("\"notificationsEnabled\":true"));
    }

    // =========================================================================
    // AuthResponse Tests
    // =========================================================================

    #[test]
    fn test_auth_response_creation() {
        let response = AuthResponse {
            authenticated: true,
            public_key: "pk".to_string(),
            session_token: "token".to_string(),
            expires_at: 1704153600000,
            profile: Profile::new("pk".to_string(), "TestPK".to_string()),
        };

        assert!(response.authenticated);
        assert_eq!(response.session_token, "token");
    }

    #[test]
    fn test_auth_response_serialization() {
        let response = AuthResponse {
            authenticated: true,
            public_key: "pk".to_string(),
            session_token: "token123".to_string(),
            expires_at: 1704153600000,
            profile: Profile {
                id: "id".to_string(),
                public_key: "pk".to_string(),
                username: "TestUser".to_string(),
                created_at: 1704067200000,
                last_seen: 1704067200000,
                show_on_leaderboard: false,
                leaderboard_signature: None,
                leaderboard_consent_at: None,
                settings: ProfileSettings::default(),
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"authenticated\":true"));
        assert!(json.contains("\"sessionToken\":\"token123\""));
    }

    // =========================================================================
    // AuthenticatedUser Tests
    // =========================================================================

    #[test]
    fn test_authenticated_user_creation() {
        let user = AuthenticatedUser {
            public_key: "user_pk".to_string(),
            profile: Profile::new("user_pk".to_string(), "TestUserPK".to_string()),
        };

        assert_eq!(user.public_key, "user_pk");
        assert_eq!(user.profile.public_key, "user_pk");
    }
}
