//! Developer API
//!
//! Endpoints for developer/testing tools:
//!
//! RAT (Random Auto Trader):
//! - POST /api/developer/rat/start - Start RAT for a portfolio
//! - POST /api/developer/rat/stop - Stop RAT for a portfolio
//! - GET /api/developer/rat/status - Get RAT status and stats
//! - PUT /api/developer/rat/config - Update RAT configuration

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::services::RatError;
use crate::types::{RatConfig, RatConfigUpdate, RatState, StartRatRequest, StopRatRequest};
use crate::AppState;

/// Create developer router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/rat/start", post(start_rat))
        .route("/rat/stop", post(stop_rat))
        .route("/rat/status", get(get_rat_status))
        .route("/rat/config", put(update_rat_config))
}

// =============================================================================
// Response Types
// =============================================================================

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub data: T,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

/// Convert RatError to HTTP response.
impl IntoResponse for RatError {
    fn into_response(self) -> axum::response::Response {
        let (status, code) = match &self {
            RatError::PortfolioNotFound(_) => (StatusCode::NOT_FOUND, "PORTFOLIO_NOT_FOUND"),
            RatError::AlreadyRunning(_) => (StatusCode::CONFLICT, "RAT_ALREADY_RUNNING"),
            RatError::NotRunning(_) => (StatusCode::NOT_FOUND, "RAT_NOT_RUNNING"),
            RatError::TradingError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "TRADING_ERROR"),
            RatError::DatabaseError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR"),
            RatError::NoPriceData => (StatusCode::SERVICE_UNAVAILABLE, "NO_PRICE_DATA"),
        };

        let body = Json(ErrorResponse {
            error: self.to_string(),
            code: code.to_string(),
        });

        (status, body).into_response()
    }
}

// =============================================================================
// RAT Endpoints
// =============================================================================

/// Start RAT for a portfolio.
///
/// POST /api/developer/rat/start
async fn start_rat(
    State(state): State<AppState>,
    Json(request): Json<StartRatRequest>,
) -> Result<Json<ApiResponse<RatState>>, RatError> {
    let rat_state = state
        .rat_service
        .clone()
        .start(&request.portfolio_id, request.config)?;

    Ok(Json(ApiResponse { data: rat_state }))
}

/// Stop RAT for a portfolio.
///
/// POST /api/developer/rat/stop
async fn stop_rat(
    State(state): State<AppState>,
    Json(request): Json<StopRatRequest>,
) -> Result<Json<ApiResponse<RatState>>, RatError> {
    let rat_state = state.rat_service.stop(&request.portfolio_id)?;

    Ok(Json(ApiResponse { data: rat_state }))
}

/// Query params for RAT status.
#[derive(Debug, Deserialize)]
pub struct RatStatusQuery {
    pub portfolio_id: String,
}

/// Get RAT status for a portfolio.
/// Creates a default config if none exists, so settings can be changed before first start.
///
/// GET /api/developer/rat/status?portfolio_id=...
async fn get_rat_status(
    State(state): State<AppState>,
    Query(query): Query<RatStatusQuery>,
) -> Result<Json<ApiResponse<RatState>>, RatError> {
    // get_state now creates a default config if none exists
    let rat_state = state
        .rat_service
        .get_state(&query.portfolio_id)
        .ok_or_else(|| RatError::PortfolioNotFound(query.portfolio_id.clone()))?;

    Ok(Json(ApiResponse { data: rat_state }))
}

/// Request to update RAT config.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRatConfigRequest {
    pub portfolio_id: String,
    #[serde(flatten)]
    pub config: RatConfigUpdate,
}

/// Update RAT configuration.
///
/// PUT /api/developer/rat/config
async fn update_rat_config(
    State(state): State<AppState>,
    Json(request): Json<UpdateRatConfigRequest>,
) -> Result<Json<ApiResponse<RatConfig>>, RatError> {
    let config = state
        .rat_service
        .update_config(&request.portfolio_id, request.config)?;

    Ok(Json(ApiResponse { data: config }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_response_serialization() {
        let response = ErrorResponse {
            error: "Portfolio not found".to_string(),
            code: "PORTFOLIO_NOT_FOUND".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"error\":\"Portfolio not found\""));
        assert!(json.contains("\"code\":\"PORTFOLIO_NOT_FOUND\""));
    }
}
