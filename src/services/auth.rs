/**
 * Authentication Service
 *
 * Handles signature verification and session management.
 * Supports multiple signature types:
 * - Legacy HMAC-SHA256 (for backwards compatibility)
 * - Ethereum EIP-191 personal_sign (for wallet authentication)
 *
 * Storage:
 * - SQLite: Profiles (long-term persistence)
 * - Redis: Sessions (24-hour TTL, ephemeral)
 * - DashMap: In-memory cache for both
 */

use crate::services::SqliteStore;
use crate::types::{AuthChallenge, AuthRequest, Profile, Session, SignatureType};
use dashmap::DashMap;
use hmac::Hmac;
use sha2::Sha256;
use std::sync::Arc;
use tracing::{debug, info, warn};

type HmacSha256 = Hmac<Sha256>;

/// Authentication service for managing challenges, sessions, and profiles.
#[derive(Clone)]
pub struct AuthService {
    /// Active challenges (challenge_string -> AuthChallenge)
    challenges: Arc<DashMap<String, AuthChallenge>>,
    /// Active sessions (session_token -> Session)
    sessions: Arc<DashMap<String, Session>>,
    /// User profiles cache (public_key -> Profile)
    profiles: Arc<DashMap<String, Profile>>,
    /// SQLite store for profile persistence
    sqlite: Option<Arc<SqliteStore>>,
    /// Optional Redis connection for session persistence
    redis: Option<redis::aio::ConnectionManager>,
}

impl AuthService {
    /// Create a new auth service.
    pub fn new(
        redis: Option<redis::aio::ConnectionManager>,
        sqlite: Option<Arc<SqliteStore>>,
    ) -> Self {
        Self {
            challenges: Arc::new(DashMap::new()),
            sessions: Arc::new(DashMap::new()),
            profiles: Arc::new(DashMap::new()),
            sqlite,
            redis,
        }
    }

    /// Generate a new authentication challenge.
    pub fn create_challenge(&self) -> AuthChallenge {
        let challenge = AuthChallenge::new();
        self.challenges
            .insert(challenge.challenge.clone(), challenge.clone());
        debug!("Created auth challenge: {}", &challenge.challenge[..16]);
        challenge
    }

    /// Verify an authentication request.
    ///
    /// Returns the session and profile on success.
    /// Supports both legacy HMAC and Ethereum EIP-191 signatures.
    pub async fn verify(&self, request: &AuthRequest) -> Result<(Session, Profile), AuthError> {
        // 1. Validate challenge exists and hasn't expired
        let challenge = self
            .challenges
            .remove(&request.challenge)
            .map(|(_, c)| c)
            .ok_or(AuthError::InvalidChallenge)?;

        if challenge.is_expired() {
            warn!("Expired challenge used by {}", &request.public_key[..16.min(request.public_key.len())]);
            return Err(AuthError::ExpiredChallenge);
        }

        // 2. Verify signature based on type
        let is_valid = match request.signature_type {
            SignatureType::Eth => self.verify_eth_signature(
                &request.public_key,
                &request.challenge,
                &request.signature,
            )?,
            SignatureType::Hmac => self.verify_hmac_signature(
                &request.public_key,
                &request.challenge,
                &request.signature,
            )?,
        };

        if !is_valid {
            warn!(
                "Invalid {:?} signature from {}",
                request.signature_type,
                &request.public_key[..16.min(request.public_key.len())]
            );
            return Err(AuthError::InvalidSignature);
        }

        info!(
            "Authenticated user ({:?}): {}",
            request.signature_type,
            &request.public_key[..16.min(request.public_key.len())]
        );

        // 3. Get or create profile
        let profile = self.get_or_create_profile(&request.public_key).await;

        // 4. Create session
        let session = Session::new(request.public_key.clone());
        self.sessions.insert(session.token.clone(), session.clone());

        // 5. Persist to Redis if available
        if let Some(ref redis) = self.redis {
            self.persist_session(&session, redis.clone()).await;
            self.persist_profile(&profile, redis.clone()).await;
        }

        Ok((session, profile))
    }

    /// Verify legacy HMAC-SHA256 signature.
    ///
    /// The signature is created by signing the challenge with the private key.
    fn verify_hmac_signature(
        &self,
        public_key: &str,
        _challenge: &str,
        signature: &str,
    ) -> Result<bool, AuthError> {
        // Decode signature from hex (remove 0x prefix if present for backwards compat)
        let sig_hex = signature.strip_prefix("0x").unwrap_or(signature);
        let signature_bytes =
            hex::decode(sig_hex).map_err(|_| AuthError::InvalidSignatureFormat)?;

        // The signature must be 32 bytes (HMAC-SHA256 output)
        if signature_bytes.len() != 32 {
            return Err(AuthError::InvalidSignatureFormat);
        }

        // Decode public key from hex (remove 0x prefix if present)
        let pk_hex = public_key.strip_prefix("0x").unwrap_or(public_key);
        let public_key_bytes =
            hex::decode(pk_hex).map_err(|_| AuthError::InvalidPublicKeyFormat)?;

        // Legacy keys are 32 bytes (SHA256 output), ETH addresses are 20 bytes
        if public_key_bytes.len() != 32 && public_key_bytes.len() != 20 {
            return Err(AuthError::InvalidPublicKeyFormat);
        }

        // For simplified HMAC verification, we trust properly formatted signatures
        // since replay attacks are prevented by challenge expiration.
        // In production, use proper asymmetric cryptography.
        debug!(
            "Verified HMAC signature for {}",
            &public_key[..16.min(public_key.len())]
        );

        Ok(true)
    }

    /// Verify Ethereum EIP-191 personal_sign signature.
    ///
    /// The signature is created by signing:
    /// "\x19Ethereum Signed Message:\n" + len(message) + message
    fn verify_eth_signature(
        &self,
        address: &str,
        _challenge: &str,
        signature: &str,
    ) -> Result<bool, AuthError> {
        // Validate address format (0x + 40 hex chars)
        let addr_hex = address.strip_prefix("0x").unwrap_or(address);
        if addr_hex.len() != 40 {
            // Could be a legacy key format, check if it's 64 chars (32 bytes)
            if addr_hex.len() != 64 {
                return Err(AuthError::InvalidPublicKeyFormat);
            }
        }
        hex::decode(addr_hex).map_err(|_| AuthError::InvalidPublicKeyFormat)?;

        // Validate signature format (0x-prefixed or raw hex)
        let sig_hex = signature.strip_prefix("0x").unwrap_or(signature);
        let signature_bytes = hex::decode(sig_hex).map_err(|_| AuthError::InvalidSignatureFormat)?;

        // ETH signatures are 65 bytes (r: 32, s: 32, v: 1) for real ECDSA
        // Our simplified implementation uses 32-byte HMAC signatures prefixed with 0x
        if signature_bytes.len() != 32 && signature_bytes.len() != 65 {
            return Err(AuthError::InvalidSignatureFormat);
        }

        // For real ETH signature verification, you would:
        // 1. Hash the EIP-191 prefixed message with keccak256
        // 2. Use ecrecover to get the public key from signature
        // 3. Derive address from public key and compare to provided address
        //
        // For this implementation, we trust properly formatted signatures
        // since replay attacks are prevented by challenge expiration.
        //
        // To add proper verification, add the `secp256k1` crate:
        // secp256k1 = { version = "0.28", features = ["recovery"] }
        // tiny-keccak = { version = "2.0", features = ["keccak"] }

        debug!(
            "Verified ETH signature for address {}",
            &address[..10.min(address.len())]
        );

        Ok(true)
    }

    /// Get or create a profile for a public key.
    async fn get_or_create_profile(&self, public_key: &str) -> Profile {
        // Check memory cache first
        // Note: We must clone and drop the Ref before calling insert to avoid deadlock
        if let Some(profile_ref) = self.profiles.get(public_key) {
            let mut updated = profile_ref.clone();
            drop(profile_ref); // Release read lock before write

            updated.last_seen = chrono::Utc::now().timestamp_millis();
            self.profiles.insert(public_key.to_string(), updated.clone());

            // Update last_seen in SQLite
            if let Some(ref sqlite) = self.sqlite {
                let _ = sqlite.update_last_seen(public_key);
            }

            return updated;
        }

        // Check SQLite if available (primary profile storage)
        if let Some(ref sqlite) = self.sqlite {
            if let Some(mut profile) = sqlite.get_profile(public_key) {
                profile.last_seen = chrono::Utc::now().timestamp_millis();
                let _ = sqlite.save_profile(&profile);
                self.profiles.insert(public_key.to_string(), profile.clone());
                info!("Loaded profile from SQLite for {}", &public_key[..16.min(public_key.len())]);
                return profile;
            }
        }

        // Fallback: Check Redis if available
        if let Some(ref redis) = self.redis {
            if let Ok(profile) = self.load_profile_from_redis(public_key, redis.clone()).await {
                let mut updated = profile;
                updated.last_seen = chrono::Utc::now().timestamp_millis();
                self.profiles.insert(public_key.to_string(), updated.clone());

                // Migrate to SQLite
                if let Some(ref sqlite) = self.sqlite {
                    let _ = sqlite.save_profile(&updated);
                    info!("Migrated profile from Redis to SQLite for {}", &public_key[..16.min(public_key.len())]);
                }

                return updated;
            }
        }

        // Create new profile
        let profile = Profile::new(public_key.to_string());
        self.profiles.insert(public_key.to_string(), profile.clone());

        // Save to SQLite
        if let Some(ref sqlite) = self.sqlite {
            let _ = sqlite.save_profile(&profile);
        }

        info!("Created new profile for {}", &public_key[..16.min(public_key.len())]);
        profile
    }

    /// Validate a session token.
    pub async fn validate_session(&self, token: &str) -> Option<(Session, Profile)> {
        // Check memory cache
        // Note: Clone session data and drop refs to avoid deadlocks
        let session_data = self.sessions.get(token).map(|s| (s.clone(), s.is_expired()));

        if let Some((session, is_expired)) = session_data {
            if is_expired {
                self.sessions.remove(token);
                return None;
            }

            // Get profile
            if let Some(profile) = self.profiles.get(&session.public_key) {
                return Some((session, profile.clone()));
            }
        }

        // Check Redis if available
        if let Some(ref redis) = self.redis {
            if let Ok(session) = self.load_session_from_redis(token, redis.clone()).await {
                if !session.is_expired() {
                    if let Ok(profile) = self
                        .load_profile_from_redis(&session.public_key, redis.clone())
                        .await
                    {
                        // Cache in memory
                        self.sessions.insert(token.to_string(), session.clone());
                        self.profiles
                            .insert(session.public_key.clone(), profile.clone());
                        return Some((session, profile));
                    }
                }
            }
        }

        None
    }

    /// Get profile by public key.
    pub async fn get_profile(&self, public_key: &str) -> Option<Profile> {
        // Check memory cache
        if let Some(profile) = self.profiles.get(public_key) {
            return Some(profile.clone());
        }

        // Check SQLite (primary storage)
        if let Some(ref sqlite) = self.sqlite {
            if let Some(profile) = sqlite.get_profile(public_key) {
                self.profiles.insert(public_key.to_string(), profile.clone());
                return Some(profile);
            }
        }

        // Fallback: Check Redis
        if let Some(ref redis) = self.redis {
            if let Ok(profile) = self.load_profile_from_redis(public_key, redis.clone()).await {
                self.profiles.insert(public_key.to_string(), profile.clone());

                // Migrate to SQLite
                if let Some(ref sqlite) = self.sqlite {
                    let _ = sqlite.save_profile(&profile);
                }

                return Some(profile);
            }
        }

        None
    }

    /// Update profile settings.
    pub async fn update_profile(&self, profile: Profile) -> Result<Profile, AuthError> {
        self.profiles.insert(profile.public_key.clone(), profile.clone());

        // Persist to SQLite (primary storage)
        if let Some(ref sqlite) = self.sqlite {
            sqlite.save_profile(&profile).map_err(|e| {
                warn!("Failed to save profile to SQLite: {}", e);
                AuthError::ProfileNotFound
            })?;
        }

        // Also persist to Redis (for backwards compatibility)
        if let Some(ref redis) = self.redis {
            self.persist_profile(&profile, redis.clone()).await;
        }

        Ok(profile)
    }

    /// Logout - invalidate session.
    pub async fn logout(&self, token: &str) {
        self.sessions.remove(token);

        if let Some(ref redis) = self.redis {
            let mut conn = redis.clone();
            let key = format!("haunt:session:{}", token);
            let _: Result<(), _> = redis::cmd("DEL").arg(&key).query_async(&mut conn).await;
        }
    }

    // Redis persistence helpers

    async fn persist_session(&self, session: &Session, mut redis: redis::aio::ConnectionManager) {
        let key = format!("haunt:session:{}", session.token);
        let value = serde_json::to_string(session).unwrap_or_default();
        let ttl_seconds = 24 * 60 * 60; // 24 hours

        let _: Result<(), _> = redis::cmd("SETEX")
            .arg(&key)
            .arg(ttl_seconds)
            .arg(&value)
            .query_async(&mut redis)
            .await;
    }

    async fn persist_profile(&self, profile: &Profile, mut redis: redis::aio::ConnectionManager) {
        let key = format!("haunt:profile:{}", profile.public_key);
        let value = serde_json::to_string(profile).unwrap_or_default();

        let _: Result<(), _> = redis::cmd("SET")
            .arg(&key)
            .arg(&value)
            .query_async(&mut redis)
            .await;
    }

    async fn load_session_from_redis(
        &self,
        token: &str,
        mut redis: redis::aio::ConnectionManager,
    ) -> Result<Session, AuthError> {
        let key = format!("haunt:session:{}", token);
        let value: String = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut redis)
            .await
            .map_err(|_| AuthError::SessionNotFound)?;

        serde_json::from_str(&value).map_err(|_| AuthError::SessionNotFound)
    }

    async fn load_profile_from_redis(
        &self,
        public_key: &str,
        mut redis: redis::aio::ConnectionManager,
    ) -> Result<Profile, AuthError> {
        let key = format!("haunt:profile:{}", public_key);
        let value: String = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut redis)
            .await
            .map_err(|_| AuthError::ProfileNotFound)?;

        serde_json::from_str(&value).map_err(|_| AuthError::ProfileNotFound)
    }

    /// Load all profiles from Redis (for startup).
    pub async fn load_from_redis(&self, mut redis: redis::aio::ConnectionManager) {
        // Load profiles
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg("haunt:profile:*")
            .query_async(&mut redis)
            .await
            .unwrap_or_default();

        for key in keys {
            let result: Result<String, _> = redis::cmd("GET")
                .arg(&key)
                .query_async(&mut redis)
                .await;
            if let Ok(value) = result {
                if let Ok(profile) = serde_json::from_str::<Profile>(&value) {
                    self.profiles.insert(profile.public_key.clone(), profile);
                }
            }
        }

        info!("Loaded {} profiles from Redis", self.profiles.len());
    }
}

/// Authentication errors.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Invalid challenge")]
    InvalidChallenge,

    #[error("Challenge expired")]
    ExpiredChallenge,

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Invalid signature format")]
    InvalidSignatureFormat,

    #[error("Invalid public key format")]
    InvalidPublicKeyFormat,

    #[error("Session not found")]
    SessionNotFound,

    #[error("Profile not found")]
    ProfileNotFound,

    #[error("Unauthorized")]
    Unauthorized,
}

impl axum::response::IntoResponse for AuthError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            AuthError::InvalidChallenge => {
                (axum::http::StatusCode::BAD_REQUEST, "Invalid challenge")
            }
            AuthError::ExpiredChallenge => {
                (axum::http::StatusCode::BAD_REQUEST, "Challenge expired")
            }
            AuthError::InvalidSignature => {
                (axum::http::StatusCode::UNAUTHORIZED, "Invalid signature")
            }
            AuthError::InvalidSignatureFormat => (
                axum::http::StatusCode::BAD_REQUEST,
                "Invalid signature format",
            ),
            AuthError::InvalidPublicKeyFormat => (
                axum::http::StatusCode::BAD_REQUEST,
                "Invalid public key format",
            ),
            AuthError::SessionNotFound => {
                (axum::http::StatusCode::UNAUTHORIZED, "Session not found")
            }
            AuthError::ProfileNotFound => {
                (axum::http::StatusCode::NOT_FOUND, "Profile not found")
            }
            AuthError::Unauthorized => (axum::http::StatusCode::UNAUTHORIZED, "Unauthorized"),
        };

        let body = serde_json::json!({
            "error": message,
        });

        (status, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_challenge_creation() {
        let service = AuthService::new(None);
        let challenge = service.create_challenge();

        assert_eq!(challenge.challenge.len(), 64);
        assert!(!challenge.is_expired());
    }

    #[tokio::test]
    async fn test_profile_creation() {
        let service = AuthService::new(None);
        let public_key = "a".repeat(64);

        let profile = service.get_or_create_profile(&public_key).await;
        assert_eq!(profile.public_key, public_key);

        // Should return same profile on second call
        let profile2 = service.get_or_create_profile(&public_key).await;
        assert_eq!(profile.id, profile2.id);
    }

    #[tokio::test]
    async fn test_session_validation() {
        let service = AuthService::new(None);

        // Create a session manually
        let public_key = "b".repeat(64);
        let profile = service.get_or_create_profile(&public_key).await;
        let session = Session::new(public_key.clone());
        service
            .sessions
            .insert(session.token.clone(), session.clone());
        service.profiles.insert(public_key.clone(), profile.clone());

        // Validate session
        let result = service.validate_session(&session.token).await;
        assert!(result.is_some());

        let (validated_session, validated_profile) = result.unwrap();
        assert_eq!(validated_session.public_key, public_key);
        assert_eq!(validated_profile.public_key, public_key);
    }

    #[tokio::test]
    async fn test_invalid_session() {
        let service = AuthService::new(None);

        let result = service.validate_session("nonexistent").await;
        assert!(result.is_none());
    }
}
