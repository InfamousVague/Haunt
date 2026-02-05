//! Trading API
//!
//! Endpoints for paper trading functionality:
//!
//! Leaderboard:
//! - GET /api/trading/leaderboard - Get portfolio leaderboard by return %
//!
//! Portfolios:
//! - GET /api/trading/portfolios - List user's portfolios
//! - POST /api/trading/portfolios - Create a new portfolio
//! - GET /api/trading/portfolios/:id - Get portfolio details
//! - GET /api/trading/portfolios/:id/summary - Get portfolio summary with metrics
//! - GET /api/trading/portfolios/:id/history - Get portfolio equity history for charting
//! - PUT /api/trading/portfolios/:id - Update portfolio settings
//! - POST /api/trading/portfolios/:id/reset - Reset portfolio to starting balance
//! - DELETE /api/trading/portfolios/:id - Delete a portfolio
//!
//! Orders:
//! - GET /api/trading/orders - List orders (with filters)
//! - POST /api/trading/orders - Place a new order
//! - GET /api/trading/orders/:id - Get order details
//! - DELETE /api/trading/orders/:id - Cancel an order
//!
//! Positions:
//! - GET /api/trading/positions - List open positions
//! - GET /api/trading/positions/:id - Get position details
//! - PUT /api/trading/positions/:id - Modify position (SL/TP)
//! - DELETE /api/trading/positions/:id - Close a position
//!
//! Trades:
//! - GET /api/trading/trades - List trade history

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::api::auth::Authenticated;
use crate::services::TradingError;
use crate::types::{
    EquityPoint, LeaderboardEntry, ModifyPositionRequest, Order, OrderType, PlaceOrderRequest,
    Portfolio, Position, PortfolioSummary, RiskSettings, Trade,
};
use crate::AppState;

/// Create trading router.
pub fn router() -> Router<AppState> {
    Router::new()
        // Leaderboard
        .route("/leaderboard", get(get_leaderboard))
        // Portfolio routes
        .route("/portfolios", get(list_portfolios))
        .route("/portfolios", post(create_portfolio))
        .route("/portfolios/:id", get(get_portfolio))
        .route("/portfolios/:id/summary", get(get_portfolio_summary))
        .route("/portfolios/:id/history", get(get_portfolio_history))
        .route("/portfolios/:id", put(update_portfolio))
        .route("/portfolios/:id/reset", post(reset_portfolio))
        .route("/portfolios/:id", delete(delete_portfolio))
        // Order routes
        .route("/orders", get(list_orders))
        .route("/orders", post(place_order))
        .route("/orders/:id", get(get_order))
        .route("/orders/:id", delete(cancel_order))
        // Position routes
        .route("/positions", get(list_positions))
        .route("/positions/:id", get(get_position))
        .route("/positions/:id", put(modify_position))
        .route("/positions/:id", delete(close_position))
        // Trade routes
        .route("/trades", get(list_trades))
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

/// Convert TradingError to HTTP response.
impl IntoResponse for TradingError {
    fn into_response(self) -> axum::response::Response {
        let (status, code) = match &self {
            TradingError::PortfolioNotFound(_) => (StatusCode::NOT_FOUND, "PORTFOLIO_NOT_FOUND"),
            TradingError::OrderNotFound(_) => (StatusCode::NOT_FOUND, "ORDER_NOT_FOUND"),
            TradingError::PositionNotFound(_) => (StatusCode::NOT_FOUND, "POSITION_NOT_FOUND"),
            TradingError::InsufficientFunds { .. } => {
                (StatusCode::BAD_REQUEST, "INSUFFICIENT_FUNDS")
            }
            TradingError::InsufficientMargin { .. } => {
                (StatusCode::BAD_REQUEST, "INSUFFICIENT_MARGIN")
            }
            TradingError::PositionLimitExceeded { .. } => {
                (StatusCode::BAD_REQUEST, "POSITION_LIMIT_EXCEEDED")
            }
            TradingError::InvalidOrder(_) => (StatusCode::BAD_REQUEST, "INVALID_ORDER"),
            TradingError::CannotCancelOrder(_) => (StatusCode::BAD_REQUEST, "CANNOT_CANCEL_ORDER"),
            TradingError::LeverageExceeded { .. } => {
                (StatusCode::BAD_REQUEST, "LEVERAGE_EXCEEDED")
            }
            TradingError::PortfolioStopped => (StatusCode::FORBIDDEN, "PORTFOLIO_STOPPED"),
            TradingError::DatabaseError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR")
            }
            TradingError::NoPriceData(_) => (StatusCode::SERVICE_UNAVAILABLE, "NO_PRICE_DATA"),
            TradingError::Unauthorized(_) => (StatusCode::FORBIDDEN, "UNAUTHORIZED"),
        };

        let body = Json(ErrorResponse {
            error: self.to_string(),
            code: code.to_string(),
        });

        (status, body).into_response()
    }
}

// =============================================================================
// Query Parameters
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct ListOrdersQuery {
    pub portfolio_id: String,
    pub status: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct ListPositionsQuery {
    pub portfolio_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ListTradesQuery {
    pub portfolio_id: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct LeaderboardQuery {
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct ClosePositionQuery {
    pub price: Option<f64>,
}

// =============================================================================
// Portfolio Handlers
// =============================================================================

/// GET /api/trading/portfolios
///
/// List all portfolios for the authenticated user.
/// For now, uses user_id from query param (will use auth later).
#[derive(Debug, Deserialize)]
pub struct ListPortfoliosQuery {
    pub user_id: String,
}

async fn list_portfolios(
    State(state): State<AppState>,
    Query(query): Query<ListPortfoliosQuery>,
) -> Json<ApiResponse<Vec<Portfolio>>> {
    let portfolios = state.trading_service.get_user_portfolios(&query.user_id);
    Json(ApiResponse { data: portfolios })
}

/// POST /api/trading/portfolios
///
/// Create a new portfolio.
async fn create_portfolio(
    State(state): State<AppState>,
    Json(request): Json<CreatePortfolioWithUser>,
) -> Result<Json<ApiResponse<Portfolio>>, TradingError> {
    let portfolio = state.trading_service.create_portfolio(
        &request.user_id,
        &request.name,
        request.description,
        request.risk_settings,
    )?;

    Ok(Json(ApiResponse { data: portfolio }))
}

#[derive(Debug, Deserialize)]
pub struct CreatePortfolioWithUser {
    pub user_id: String,
    pub name: String,
    pub description: Option<String>,
    pub risk_settings: Option<RiskSettings>,
}

/// GET /api/trading/portfolios/:id
///
/// Get portfolio details.
async fn get_portfolio(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Portfolio>>, TradingError> {
    let portfolio = state
        .trading_service
        .get_portfolio(&id)
        .ok_or_else(|| TradingError::PortfolioNotFound(id))?;

    Ok(Json(ApiResponse { data: portfolio }))
}

/// GET /api/trading/portfolios/:id/summary
///
/// Get portfolio summary with performance metrics.
async fn get_portfolio_summary(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<PortfolioSummary>>, TradingError> {
    let summary = state.trading_service.get_portfolio_summary(&id)?;
    Ok(Json(ApiResponse { data: summary }))
}

/// Query parameters for portfolio history.
#[derive(Debug, Deserialize)]
pub struct PortfolioHistoryQuery {
    /// Filter snapshots since this timestamp (ms)
    pub since: Option<i64>,
    /// Maximum number of points to return
    pub limit: Option<usize>,
}

/// GET /api/trading/portfolios/:id/history
///
/// Get portfolio equity history for charting (equity curve).
async fn get_portfolio_history(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<PortfolioHistoryQuery>,
) -> Result<Json<ApiResponse<Vec<EquityPoint>>>, TradingError> {
    let history = state
        .trading_service
        .get_portfolio_history(&id, query.since, query.limit)?;
    Ok(Json(ApiResponse { data: history }))
}

/// PUT /api/trading/portfolios/:id
///
/// Update portfolio settings.
async fn update_portfolio(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(settings): Json<RiskSettings>,
) -> Result<Json<ApiResponse<Portfolio>>, TradingError> {
    let portfolio = state
        .trading_service
        .update_portfolio_settings(&id, settings)?;

    Ok(Json(ApiResponse { data: portfolio }))
}

/// POST /api/trading/portfolios/:id/reset
///
/// Reset portfolio to starting balance, closing all positions and orders.
async fn reset_portfolio(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Portfolio>>, TradingError> {
    let portfolio = state.trading_service.reset_portfolio(&id)?;
    Ok(Json(ApiResponse { data: portfolio }))
}

/// DELETE /api/trading/portfolios/:id
///
/// Delete a portfolio and all associated data.
async fn delete_portfolio(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<DeleteResponse>>, TradingError> {
    state.trading_service.delete_portfolio(&id)?;
    Ok(Json(ApiResponse {
        data: DeleteResponse {
            deleted: true,
            id,
        },
    }))
}

#[derive(Debug, Serialize)]
pub struct DeleteResponse {
    pub deleted: bool,
    pub id: String,
}

// =============================================================================
// Order Handlers
// =============================================================================

/// GET /api/trading/orders
///
/// List orders for a portfolio. Requires authentication.
async fn list_orders(
    auth: Authenticated,
    State(state): State<AppState>,
    Query(query): Query<ListOrdersQuery>,
) -> Result<Json<ApiResponse<Vec<Order>>>, TradingError> {
    // Verify user owns the portfolio
    let portfolio = state
        .trading_service
        .get_portfolio(&query.portfolio_id)
        .ok_or_else(|| TradingError::PortfolioNotFound(query.portfolio_id.clone()))?;

    if portfolio.user_id != auth.user.public_key {
        return Err(TradingError::Unauthorized(
            "You do not own this portfolio".to_string(),
        ));
    }

    let limit = query.limit.unwrap_or(100);

    let orders = if query.status.as_deref() == Some("open") {
        state.trading_service.get_open_orders(&query.portfolio_id)
    } else {
        state
            .trading_service
            .get_order_history(&query.portfolio_id, limit)
    };

    Ok(Json(ApiResponse { data: orders }))
}

/// POST /api/trading/orders
///
/// Place a new order. Requires authentication.
/// Market orders are executed immediately at current price.
async fn place_order(
    auth: Authenticated,
    State(state): State<AppState>,
    Json(request): Json<PlaceOrderRequest>,
) -> Result<Json<ApiResponse<Order>>, TradingError> {
    // Verify user owns the portfolio
    let portfolio = state
        .trading_service
        .get_portfolio(&request.portfolio_id)
        .ok_or_else(|| TradingError::PortfolioNotFound(request.portfolio_id.clone()))?;

    if portfolio.user_id != auth.user.public_key {
        return Err(TradingError::Unauthorized(
            "You do not own this portfolio".to_string(),
        ));
    }

    let is_market_order = request.order_type == OrderType::Market;
    let symbol = request.symbol.clone();

    // Place the order (creates it in pending state)
    let order = state.trading_service.place_order(request)?;

    // For market orders, execute immediately at current price
    if is_market_order {
        // Get current price from price cache
        if let Some(current_price) = state.price_cache.get_price(&symbol.to_lowercase()) {
            // Execute the market order
            match state.trading_service.execute_market_order(&order.id, current_price, None) {
                Ok(_trade) => {
                    // Return the updated (filled) order
                    if let Some(filled_order) = state.trading_service.get_order(&order.id) {
                        return Ok(Json(ApiResponse { data: filled_order }));
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to execute market order {}: {}", order.id, e);
                    // Return the pending order if execution failed
                }
            }
        } else {
            tracing::warn!("No price available for {}, market order stays pending", symbol);
        }
    }

    Ok(Json(ApiResponse { data: order }))
}

/// GET /api/trading/orders/:id
///
/// Get order details.
async fn get_order(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Order>>, TradingError> {
    let order = state
        .trading_service
        .get_order(&id)
        .ok_or_else(|| TradingError::OrderNotFound(id))?;

    Ok(Json(ApiResponse { data: order }))
}

/// DELETE /api/trading/orders/:id
///
/// Cancel an order. Requires authentication.
async fn cancel_order(
    auth: Authenticated,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Order>>, TradingError> {
    // Get the order to find its portfolio
    let order = state
        .trading_service
        .get_order(&id)
        .ok_or_else(|| TradingError::OrderNotFound(id.clone()))?;

    // Verify user owns the portfolio
    let portfolio = state
        .trading_service
        .get_portfolio(&order.portfolio_id)
        .ok_or_else(|| TradingError::PortfolioNotFound(order.portfolio_id.clone()))?;

    if portfolio.user_id != auth.user.public_key {
        return Err(TradingError::Unauthorized(
            "You do not own this order".to_string(),
        ));
    }

    let order = state.trading_service.cancel_order(&id)?;
    Ok(Json(ApiResponse { data: order }))
}

// =============================================================================
// Position Handlers
// =============================================================================

/// GET /api/trading/positions
///
/// List open positions for a portfolio. Requires authentication.
async fn list_positions(
    auth: Authenticated,
    State(state): State<AppState>,
    Query(query): Query<ListPositionsQuery>,
) -> Result<Json<ApiResponse<Vec<Position>>>, TradingError> {
    // Verify user owns the portfolio
    let portfolio = state
        .trading_service
        .get_portfolio(&query.portfolio_id)
        .ok_or_else(|| TradingError::PortfolioNotFound(query.portfolio_id.clone()))?;

    if portfolio.user_id != auth.user.public_key {
        return Err(TradingError::Unauthorized(
            "You do not own this portfolio".to_string(),
        ));
    }

    let positions = state.trading_service.get_positions(&query.portfolio_id);
    Ok(Json(ApiResponse { data: positions }))
}

/// GET /api/trading/positions/:id
///
/// Get position details.
async fn get_position(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Position>>, TradingError> {
    let position = state
        .trading_service
        .get_position(&id)
        .ok_or_else(|| TradingError::PositionNotFound(id))?;

    Ok(Json(ApiResponse { data: position }))
}

/// PUT /api/trading/positions/:id
///
/// Modify position stop loss and take profit. Requires authentication.
async fn modify_position(
    auth: Authenticated,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<ModifyPositionRequest>,
) -> Result<Json<ApiResponse<Position>>, TradingError> {
    // Get the position to find its portfolio
    let pos = state
        .trading_service
        .get_position(&id)
        .ok_or_else(|| TradingError::PositionNotFound(id.clone()))?;

    // Verify user owns the portfolio
    let portfolio = state
        .trading_service
        .get_portfolio(&pos.portfolio_id)
        .ok_or_else(|| TradingError::PortfolioNotFound(pos.portfolio_id.clone()))?;

    if portfolio.user_id != auth.user.public_key {
        return Err(TradingError::Unauthorized(
            "You do not own this position".to_string(),
        ));
    }

    let position = state
        .trading_service
        .modify_position(&id, request.stop_loss, request.take_profit)?;

    Ok(Json(ApiResponse { data: position }))
}

/// DELETE /api/trading/positions/:id
///
/// Close a position at market price. Requires authentication.
async fn close_position(
    auth: Authenticated,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<ClosePositionQuery>,
) -> Result<Json<ApiResponse<Trade>>, TradingError> {
    // Get the position to find its portfolio
    let pos = state
        .trading_service
        .get_position(&id)
        .ok_or_else(|| TradingError::PositionNotFound(id.clone()))?;

    // Verify user owns the portfolio
    let portfolio = state
        .trading_service
        .get_portfolio(&pos.portfolio_id)
        .ok_or_else(|| TradingError::PortfolioNotFound(pos.portfolio_id.clone()))?;

    if portfolio.user_id != auth.user.public_key {
        return Err(TradingError::Unauthorized(
            "You do not own this position".to_string(),
        ));
    }

    let close_price = query.price.unwrap_or(pos.current_price);
    let trade = state.trading_service.close_position(&id, close_price)?;
    Ok(Json(ApiResponse { data: trade }))
}

// =============================================================================
// Trade Handlers
// =============================================================================

/// GET /api/trading/trades
///
/// List trade history for a portfolio. Requires authentication.
async fn list_trades(
    auth: Authenticated,
    State(state): State<AppState>,
    Query(query): Query<ListTradesQuery>,
) -> Result<Json<ApiResponse<Vec<Trade>>>, TradingError> {
    // Verify user owns the portfolio
    let portfolio = state
        .trading_service
        .get_portfolio(&query.portfolio_id)
        .ok_or_else(|| TradingError::PortfolioNotFound(query.portfolio_id.clone()))?;

    if portfolio.user_id != auth.user.public_key {
        return Err(TradingError::Unauthorized(
            "You do not own this portfolio".to_string(),
        ));
    }

    let limit = query.limit.unwrap_or(100);
    let trades = state.trading_service.get_trades(&query.portfolio_id, limit);
    Ok(Json(ApiResponse { data: trades }))
}

// =============================================================================
// Leaderboard Handlers
// =============================================================================

/// GET /api/trading/leaderboard
///
/// Get leaderboard of portfolios ranked by total return percentage.
async fn get_leaderboard(
    State(state): State<AppState>,
    Query(query): Query<LeaderboardQuery>,
) -> Json<ApiResponse<Vec<LeaderboardEntry>>> {
    let limit = query.limit.unwrap_or(100);
    let leaderboard = state.trading_service.get_leaderboard(limit);
    Json(ApiResponse { data: leaderboard })
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_close_position_query_allows_missing_price() {
        let query: ClosePositionQuery = serde_urlencoded::from_str("").unwrap();
        assert!(query.price.is_none());
    }

    #[test]
    fn test_close_position_query_parses_price() {
        let query: ClosePositionQuery = serde_urlencoded::from_str("price=123.45").unwrap();
        assert_eq!(query.price, Some(123.45));
    }

    #[test]
    fn test_error_response_serialization() {
        let error = ErrorResponse {
            error: "Portfolio not found".to_string(),
            code: "PORTFOLIO_NOT_FOUND".to_string(),
        };

        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("PORTFOLIO_NOT_FOUND"));
    }

    #[test]
    fn test_delete_response_serialization() {
        let response = DeleteResponse {
            deleted: true,
            id: "test-123".to_string(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"deleted\":true"));
        assert!(json.contains("\"id\":\"test-123\""));
    }
}
