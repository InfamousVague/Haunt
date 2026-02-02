//! Order Book API
//!
//! Provides aggregated order book data from multiple exchanges.

use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use crate::error::Result;
use crate::types::AggregatedOrderBook;
use crate::AppState;

/// Query parameters for order book endpoint.
#[derive(Debug, Deserialize)]
pub struct OrderBookQuery {
    /// Number of depth levels (default: 50, max: 100)
    pub depth: Option<usize>,
}

/// API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub data: T,
}

/// GET /api/orderbook/:symbol
///
/// Returns aggregated order book data from multiple exchanges.
async fn get_orderbook(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<OrderBookQuery>,
) -> Result<Json<ApiResponse<AggregatedOrderBook>>> {
    let book = state.orderbook_service.get_aggregated(&symbol, query.depth).await;
    Ok(Json(ApiResponse { data: book }))
}

/// Create the order book router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/:symbol", get(get_orderbook))
}
