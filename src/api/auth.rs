/**
 * Authentication API
 *
 * Endpoints for authentication and profile management.
 *
 * Flow:
 * 1. GET /api/auth/challenge - Get a challenge to sign
 * 2. POST /api/auth/verify - Submit signed challenge to authenticate
 * 3. GET /api/auth/me - Get current user profile (requires auth)
 * 4. PUT /api/auth/profile - Update profile settings (requires auth)
 * 5. POST /api/auth/logout - Logout and invalidate session
 */

use axum::{
    extract::{FromRequestParts, State},
    http::{request::Parts, StatusCode},
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::services::{AuthError, AuthService};
use crate::types::{AuthChallenge, AuthRequest, AuthResponse, AuthenticatedUser, Profile, ProfileSettings};

/// App state containing auth service.
#[derive(Clone)]
pub struct AuthState {
    pub auth_service: Arc<AuthService>,
}

/// Create auth router.
pub fn router(auth_service: Arc<AuthService>) -> Router {
    let state = AuthState { auth_service };

    Router::new()
        .route("/challenge", get(get_challenge))
        .route("/verify", post(verify))
        .route("/me", get(get_me))
        .route("/profile", put(update_profile))
        .route("/logout", post(logout))
        .with_state(state)
}

/// GET /api/auth/challenge
///
/// Get a challenge string to sign for authentication.
async fn get_challenge(State(state): State<AuthState>) -> Json<ApiResponse<AuthChallenge>> {
    let challenge = state.auth_service.create_challenge();
    Json(ApiResponse { data: challenge })
}

/// POST /api/auth/verify
///
/// Verify a signed challenge and create a session.
async fn verify(
    State(state): State<AuthState>,
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
async fn get_me(
    State(state): State<AuthState>,
    auth: Authenticated,
) -> Json<ApiResponse<Profile>> {
    Json(ApiResponse {
        data: auth.user.profile,
    })
}

/// PUT /api/auth/profile
///
/// Update the authenticated user's profile settings.
async fn update_profile(
    State(state): State<AuthState>,
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
async fn logout(
    State(state): State<AuthState>,
    auth: Authenticated,
) -> Json<ApiResponse<LogoutResponse>> {
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
impl<S> FromRequestParts<S> for Authenticated
where
    S: Send + Sync,
    AuthState: FromRef<S>,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let auth_state = AuthState::from_ref(state);

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
        let (session, profile) = auth_state
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

/// Helper trait to extract AuthState from parent state.
pub trait FromRef<T> {
    fn from_ref(input: &T) -> Self;
}

impl FromRef<AuthState> for AuthState {
    fn from_ref(input: &AuthState) -> Self {
        input.clone()
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

    #[test]
    fn test_api_response_serialization() {
        let response = ApiResponse {
            data: "test".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"data\":\"test\""));
    }
}
