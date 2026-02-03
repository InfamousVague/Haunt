/**
 * User API
 *
 * Endpoints for user preferences management and cross-server sync.
 *
 * Endpoints:
 * - GET  /api/user/preferences     - Get user preferences
 * - PUT  /api/user/preferences     - Update preferences
 * - POST /api/user/preferences/sync - Merge client preferences with server
 */

use axum::{
    extract::State,
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::api::auth::Authenticated;
use crate::services::AuthError;
use crate::types::{PartialPreferences, PreferencesSyncResponse, UserPreferences};
use crate::AppState;

/// Create user router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/preferences", get(get_preferences))
        .route("/preferences", put(update_preferences))
        .route("/preferences/sync", post(sync_preferences))
}

/// API response wrapper.
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub data: T,
}

/// GET /api/user/preferences
///
/// Get the authenticated user's preferences.
async fn get_preferences(
    State(state): State<AppState>,
    auth: Authenticated,
) -> Result<Json<ApiResponse<UserPreferences>>, AuthError> {
    let prefs = state
        .preferences_service
        .get_preferences(&auth.user.public_key)
        .await;

    Ok(Json(ApiResponse { data: prefs }))
}

/// PUT /api/user/preferences
///
/// Update the authenticated user's preferences.
async fn update_preferences(
    State(state): State<AppState>,
    auth: Authenticated,
    Json(request): Json<PartialPreferences>,
) -> Result<Json<ApiResponse<UserPreferences>>, AuthError> {
    let prefs = state
        .preferences_service
        .update_preferences(&auth.user.public_key, request)
        .await?;

    Ok(Json(ApiResponse { data: prefs }))
}

/// Sync request from client.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRequest {
    /// Client's current preferences
    pub preferences: UserPreferences,
}

/// POST /api/user/preferences/sync
///
/// Sync client preferences with server.
/// Uses timestamp-based conflict resolution.
async fn sync_preferences(
    State(state): State<AppState>,
    auth: Authenticated,
    Json(request): Json<SyncRequest>,
) -> Result<Json<ApiResponse<PreferencesSyncResponse>>, AuthError> {
    let response = state
        .preferences_service
        .sync_preferences(&auth.user.public_key, request.preferences)
        .await?;

    Ok(Json(ApiResponse { data: response }))
}
