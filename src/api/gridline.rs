//! Gridline Trading API
//!
//! REST endpoints for the Gridline prediction trading system:
//!
//! - GET  /api/grid/state/:symbol       - Grid state (config + multipliers + active positions)
//! - POST /api/grid/trade               - Place a new gridline trade
//! - DELETE /api/grid/trade/:id         - Cancel an active trade
//! - GET  /api/grid/positions/:portfolio_id  - Active positions for a portfolio
//! - GET  /api/grid/history/:portfolio_id - Trade history (paginated)
//! - GET  /api/grid/stats/:portfolio_id/:symbol - Stats for portfolio + symbol
//! - GET  /api/grid/config/:symbol      - Get optimal grid config for symbol
//! - GET  /api/grid/multipliers/:symbol - Current multiplier matrix

use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::api::auth::Authenticated;
use crate::types::{
    GridConfig, GridConfigRequest, GridState, GridStats, GridlineError, GridlinePosition,
    PlaceGridlineRequest,
};
use crate::AppState;

// =============================================================================
// Router
// =============================================================================

/// Create gridline trading router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/state/:symbol", get(get_grid_state))
        .route("/trade", post(place_gridline_trade))
        .route("/trade/:id", delete(cancel_gridline_trade))
        .route("/positions/:portfolio_id", get(get_active_positions))
        .route("/history/:portfolio_id", get(get_trade_history))
        .route("/stats/:portfolio_id/:symbol", get(get_grid_stats))
        .route("/config/:symbol", get(get_grid_config))
        .route("/multipliers/:symbol", get(get_multipliers))
}

// =============================================================================
// Response types
// =============================================================================

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub data: T,
}

// =============================================================================
// Query types
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct GridStateQuery {
    pub portfolio_id: Option<String>,
    pub row_count: Option<u32>,
    pub col_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct TradeHistoryQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// =============================================================================
// Handlers
// =============================================================================

/// GET /api/grid/state/:symbol
///
/// Get the full grid state including config, multiplier matrix, and active positions.
async fn get_grid_state(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<GridStateQuery>,
) -> Result<Json<ApiResponse<GridState>>, GridlineError> {
    let symbol_upper = symbol.to_uppercase();

    // Get current price from price cache
    let current_price = state
        .price_cache
        .get_price(&symbol_upper)
        .unwrap_or(0.0);

    // Build grid config
    let config = state.gridline_service.build_config(
        &symbol_upper,
        current_price,
        query.row_count,
        query.col_count,
    );

    // Get grid state
    let grid_state = state.gridline_service.get_grid_state(
        query.portfolio_id.as_deref(),
        &symbol_upper,
        &config,
    )?;

    Ok(Json(ApiResponse { data: grid_state }))
}

/// POST /api/grid/trade
///
/// Place a new gridline trade. Requires authentication.
async fn place_gridline_trade(
    auth: Authenticated,
    State(state): State<AppState>,
    Json(req): Json<PlaceGridlineRequest>,
) -> Result<Json<ApiResponse<GridlinePosition>>, GridlineError> {
    // Verify the user owns the portfolio
    let portfolio = state
        .trading_service
        .get_portfolio(&req.portfolio_id)
        .ok_or_else(|| GridlineError::PortfolioNotFound(req.portfolio_id.clone()))?;

    if portfolio.user_id != auth.user.public_key {
        return Err(GridlineError::PortfolioNotFound(req.portfolio_id.clone()));
    }

    let available_balance = portfolio.cash_balance;

    // Place the trade (validates amount, leverage, cell, etc.)
    let trade = state
        .gridline_service
        .place_trade(req, available_balance)?;

    // Debit the margin from portfolio balance
    state
        .trading_service
        .debit_gridline_trade(&trade.portfolio_id, trade.amount)
        .map_err(|e| GridlineError::TradingError(e.to_string()))?;

    Ok(Json(ApiResponse { data: trade }))
}

/// DELETE /api/grid/trade/:id
///
/// Cancel an active gridline trade. Refunds the margin to portfolio. Requires authentication.
async fn cancel_gridline_trade(
    auth: Authenticated,
    State(state): State<AppState>,
    Path(trade_id): Path<String>,
) -> Result<Json<ApiResponse<GridlinePosition>>, GridlineError> {
    // We need to look up the trade to find the portfolio, then verify ownership
    let trade = state
        .gridline_service
        .cancel_trade_lookup(&trade_id)
        .ok_or_else(|| GridlineError::TradeNotFound(trade_id.clone()))?;

    // Verify the user owns the portfolio
    let portfolio = state
        .trading_service
        .get_portfolio(&trade.portfolio_id)
        .ok_or_else(|| GridlineError::PortfolioNotFound(trade.portfolio_id.clone()))?;

    if portfolio.user_id != auth.user.public_key {
        return Err(GridlineError::TradeNotFound(trade_id));
    }

    // Cancel the trade
    let cancelled = state
        .gridline_service
        .cancel_trade(&trade.id, &trade.portfolio_id)?;

    // Refund the margin
    state
        .trading_service
        .credit_gridline_payout(&cancelled.portfolio_id, cancelled.amount)
        .map_err(|e| GridlineError::TradingError(e.to_string()))?;

    Ok(Json(ApiResponse { data: cancelled }))
}

/// GET /api/grid/positions/:portfolio_id
///
/// Get all active gridline positions for a portfolio. Requires authentication.
async fn get_active_positions(
    auth: Authenticated,
    State(state): State<AppState>,
    Path(portfolio_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<GridlinePosition>>>, GridlineError> {
    // Verify ownership
    let portfolio = state
        .trading_service
        .get_portfolio(&portfolio_id)
        .ok_or_else(|| GridlineError::PortfolioNotFound(portfolio_id.clone()))?;

    if portfolio.user_id != auth.user.public_key {
        return Err(GridlineError::PortfolioNotFound(portfolio_id));
    }

    let positions = state.sqlite_store.get_active_gridline_positions(&portfolio_id);
    Ok(Json(ApiResponse { data: positions }))
}

/// GET /api/grid/history/:portfolio_id
///
/// Get paginated gridline trade history for a portfolio. Requires authentication.
async fn get_trade_history(
    auth: Authenticated,
    State(state): State<AppState>,
    Path(portfolio_id): Path<String>,
    Query(query): Query<TradeHistoryQuery>,
) -> Result<Json<ApiResponse<Vec<GridlinePosition>>>, GridlineError> {
    // Verify ownership
    let portfolio = state
        .trading_service
        .get_portfolio(&portfolio_id)
        .ok_or_else(|| GridlineError::PortfolioNotFound(portfolio_id.clone()))?;

    if portfolio.user_id != auth.user.public_key {
        return Err(GridlineError::PortfolioNotFound(portfolio_id));
    }

    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);
    let history = state
        .sqlite_store
        .get_gridline_history(&portfolio_id, limit, offset);

    Ok(Json(ApiResponse { data: history }))
}

/// GET /api/grid/stats/:portfolio_id/:symbol
///
/// Get gridline trading statistics for a portfolio + symbol.
async fn get_grid_stats(
    State(state): State<AppState>,
    Path((portfolio_id, symbol)): Path<(String, String)>,
) -> Result<Json<ApiResponse<GridStats>>, GridlineError> {
    let symbol_upper = symbol.to_uppercase();
    let stats = state
        .gridline_service
        .get_stats(&portfolio_id, &symbol_upper)
        .unwrap_or(GridStats {
            portfolio_id: portfolio_id.clone(),
            symbol: symbol_upper,
            total_trades: 0,
            total_won: 0,
            total_lost: 0,
            total_wagered: 0.0,
            total_payout: 0.0,
            net_pnl: 0.0,
            best_multiplier_hit: 0.0,
            max_leverage_used: 0.0,
            updated_at: 0,
        });

    Ok(Json(ApiResponse { data: stats }))
}

/// GET /api/grid/config/:symbol
///
/// Get optimal grid configuration for a symbol (uses volatility to determine intervals).
async fn get_grid_config(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<GridConfigRequest>,
) -> Result<Json<ApiResponse<GridConfig>>, GridlineError> {
    let symbol_upper = symbol.to_uppercase();

    let current_price = state
        .price_cache
        .get_price(&symbol_upper)
        .unwrap_or(0.0);

    let config = state.gridline_service.build_config(
        &symbol_upper,
        current_price,
        query.row_count,
        query.col_count,
    );

    Ok(Json(ApiResponse { data: config }))
}

/// GET /api/grid/multipliers/:symbol
///
/// Get the current multiplier matrix for a symbol.
async fn get_multipliers(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<GridConfigRequest>,
) -> Result<Json<ApiResponse<Vec<Vec<f64>>>>, GridlineError> {
    let symbol_upper = symbol.to_uppercase();

    let current_price = state
        .price_cache
        .get_price(&symbol_upper)
        .unwrap_or(0.0);

    let config = state.gridline_service.build_config(
        &symbol_upper,
        current_price,
        query.row_count,
        query.col_count,
    );

    let multipliers = state.gridline_service.calculate_multipliers(
        &symbol_upper,
        current_price,
        &config,
    );

    Ok(Json(ApiResponse { data: multipliers }))
}
