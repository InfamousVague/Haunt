//! Username validation API endpoints.
//!
//! Provides endpoints for:
//! - Username availability check
//! - Username validation
//! - Username change (with rate limiting)

use crate::services::{UsernameErrorCode, UsernameFilterService, ValidationResult};
use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

/// Create username API router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/validate/:username", get(validate_username))
        .route("/check/:username", get(check_availability))
        .route("/validate", post(validate_username_body))
        .route("/suggestions/:base", get(get_suggestions))
}

/// Username validation request body.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateUsernameRequest {
    pub username: String,
    /// Optional user ID for rate limit tracking.
    pub user_id: Option<String>,
}

/// Username validation response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsernameValidationResponse {
    pub is_valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<UsernameErrorCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<String>,
    pub timestamp: i64,
}

impl From<ValidationResult> for UsernameValidationResponse {
    fn from(result: ValidationResult) -> Self {
        Self {
            is_valid: result.is_valid,
            error: result.error,
            error_code: result.error_code,
            normalized: result.normalized,
            suggestions: result.suggestions,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// Availability check response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailabilityResponse {
    pub username: String,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<String>,
    pub timestamp: i64,
}

/// Suggestions response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionsResponse {
    pub base: String,
    pub suggestions: Vec<String>,
    pub timestamp: i64,
}

/// Validate username via URL path parameter.
pub async fn validate_username(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let result = state.username_filter.validate(&username);
    Ok(Json(UsernameValidationResponse::from(result)))
}

/// Validate username via POST body (with optional rate limiting).
pub async fn validate_username_body(
    State(state): State<AppState>,
    Json(req): Json<ValidateUsernameRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let result = if let Some(user_id) = &req.user_id {
        state
            .username_filter
            .validate_with_rate_limit(&req.username, user_id)
    } else {
        state.username_filter.validate(&req.username)
    };

    Ok(Json(UsernameValidationResponse::from(result)))
}

/// Check username availability.
pub async fn check_availability(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let result = state.username_filter.validate(&username);

    let (available, reason) = if result.is_valid {
        (true, None)
    } else {
        (false, result.error)
    };

    Ok(Json(AvailabilityResponse {
        username,
        available,
        reason,
        suggestions: result.suggestions,
        timestamp: chrono::Utc::now().timestamp_millis(),
    }))
}

/// Get username suggestions based on a base name.
pub async fn get_suggestions(
    State(state): State<AppState>,
    Path(base): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Validate to generate suggestions
    let result = state.username_filter.validate(&base);

    // If valid, no suggestions needed
    let suggestions = if result.is_valid {
        vec![]
    } else {
        result.suggestions
    };

    Ok(Json(SuggestionsResponse {
        base,
        suggestions,
        timestamp: chrono::Utc::now().timestamp_millis(),
    }))
}
