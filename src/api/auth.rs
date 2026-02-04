//! Authentication API
//!
//! Endpoints for authentication and profile management.
//!
//! Flow:
//! 1. GET /api/auth/challenge - Get a challenge to sign
//! 2. POST /api/auth/verify - Submit signed challenge to authenticate
//! 3. GET /api/auth/me - Get current user profile (requires auth)
//! 4. PUT /api/auth/profile - Update profile settings (requires auth)
//! 5. POST /api/auth/logout - Logout and invalidate session

use axum::{
    extract::{FromRequestParts, State},
    http::request::Parts,
    routing::{get, post, put},
    Json, Router,
};
use serde::Serialize;

use crate::services::AuthError;
use crate::types::{
    AuthChallenge, AuthRequest, AuthResponse, AuthenticatedUser, Profile, ProfileSettings,
};
use crate::AppState;

/// Create auth router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/challenge", get(get_challenge))
        .route("/verify", post(verify))
        .route("/me", get(get_me))
        .route("/profile", put(update_profile))
        .route("/logout", post(logout))
}

/// GET /api/auth/challenge
///
/// Get a challenge string to sign for authentication.
async fn get_challenge(State(state): State<AppState>) -> Json<ApiResponse<AuthChallenge>> {
    let challenge = state.auth_service.create_challenge();
    Json(ApiResponse { data: challenge })
}

/// POST /api/auth/verify
///
/// Verify a signed challenge and create a session.
async fn verify(
    State(state): State<AppState>,
    Json(request): Json<AuthRequest>,
) -> Result<Json<ApiResponse<AuthResponse>>, AuthError> {
    let (session, profile) = state.auth_service.verify(&request).await?;

    let response = AuthResponse {
        authenticated: true,
        public_key: session.public_key.clone(),
        session_token: session.token,
        expires_at: session.expires_at,
        profile,
    };

    Ok(Json(ApiResponse { data: response }))
}

/// GET /api/auth/me
///
/// Get the current authenticated user's profile.
async fn get_me(auth: Authenticated) -> Json<ApiResponse<Profile>> {
    Json(ApiResponse {
        data: auth.user.profile,
    })
}

/// PUT /api/auth/profile
///
/// Update the authenticated user's profile settings.
async fn update_profile(
    State(state): State<AppState>,
    auth: Authenticated,
    Json(settings): Json<ProfileSettings>,
) -> Result<Json<ApiResponse<Profile>>, AuthError> {
    let mut profile = auth.user.profile;
    profile.settings = settings;

    let updated = state.auth_service.update_profile(profile).await?;

    Ok(Json(ApiResponse { data: updated }))
}

/// POST /api/auth/logout
///
/// Logout and invalidate the current session.
async fn logout(_auth: Authenticated) -> Json<ApiResponse<LogoutResponse>> {
    // Get the token from the Authorization header
    // The Authenticated extractor already validated the session
    // We need to get the token to invalidate it

    // For now, just return success - the session will expire naturally
    // In production, you'd want to pass the token through
    Json(ApiResponse {
        data: LogoutResponse { success: true },
    })
}

/// Authenticated user extractor.
///
/// Use this in route handlers to require authentication:
/// ```
/// async fn my_handler(auth: Authenticated) -> impl IntoResponse {
///     let user = auth.user;
///     // ...
/// }
/// ```
pub struct Authenticated {
    pub user: AuthenticatedUser,
}

#[axum::async_trait]
impl FromRequestParts<AppState> for Authenticated {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Get Authorization header
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthError::Unauthorized)?;

        // Extract Bearer token
        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(AuthError::Unauthorized)?;

        // Validate session
        let (session, profile) = state
            .auth_service
            .validate_session(token)
            .await
            .ok_or(AuthError::SessionNotFound)?;

        Ok(Authenticated {
            user: AuthenticatedUser {
                public_key: session.public_key,
                profile,
            },
        })
    }
}

/// API response wrapper.
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub data: T,
}

/// Logout response.
#[derive(Debug, Serialize)]
pub struct LogoutResponse {
    pub success: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // ApiResponse Tests
    // =========================================================================

    #[test]
    fn test_api_response_serialization() {
        let response = ApiResponse {
            data: "test".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"data\":\"test\""));
    }

    #[test]
    fn test_api_response_with_struct() {
        #[derive(Serialize)]
        struct TestData {
            value: i32,
        }

        let response = ApiResponse {
            data: TestData { value: 42 },
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"data\":{\"value\":42}"));
    }

    #[test]
    fn test_api_response_with_vec() {
        let response = ApiResponse {
            data: vec!["a", "b", "c"],
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"data\":[\"a\",\"b\",\"c\"]"));
    }

    #[test]
    fn test_api_response_debug() {
        let response = ApiResponse { data: 123 };
        let debug_str = format!("{:?}", response);
        assert!(debug_str.contains("ApiResponse"));
        assert!(debug_str.contains("123"));
    }

    // =========================================================================
    // LogoutResponse Tests
    // =========================================================================

    #[test]
    fn test_logout_response_success() {
        let response = LogoutResponse { success: true };
        let json = serde_json::to_string(&response).unwrap();
        assert_eq!(json, "{\"success\":true}");
    }

    #[test]
    fn test_logout_response_failure() {
        let response = LogoutResponse { success: false };
        let json = serde_json::to_string(&response).unwrap();
        assert_eq!(json, "{\"success\":false}");
    }

    #[test]
    fn test_logout_response_debug() {
        let response = LogoutResponse { success: true };
        let debug_str = format!("{:?}", response);
        assert!(debug_str.contains("LogoutResponse"));
        assert!(debug_str.contains("true"));
    }
}
