//! Order Book API
//!
//! Provides aggregated order book data from multiple exchanges.

use crate::error::Result;
use crate::types::AggregatedOrderBook;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

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
    let book = state
        .orderbook_service
        .get_aggregated(&symbol, query.depth)
        .await;
    Ok(Json(ApiResponse { data: book }))
}

/// Create the order book router.
pub fn router() -> Router<AppState> {
    Router::new().route("/:symbol", get(get_orderbook))
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // OrderBookQuery Tests
    // =========================================================================

    #[test]
    fn test_orderbook_query_none_depth() {
        let query = OrderBookQuery { depth: None };
        assert!(query.depth.is_none());
    }

    #[test]
    fn test_orderbook_query_with_depth() {
        let query = OrderBookQuery { depth: Some(25) };
        assert_eq!(query.depth, Some(25));
    }

    #[test]
    fn test_orderbook_query_max_depth() {
        let query = OrderBookQuery { depth: Some(100) };
        assert_eq!(query.depth, Some(100));
    }

    #[test]
    fn test_orderbook_query_deserialization() {
        let json = r#"{"depth": 50}"#;
        let query: OrderBookQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.depth, Some(50));
    }

    #[test]
    fn test_orderbook_query_deserialization_empty() {
        let json = r#"{}"#;
        let query: OrderBookQuery = serde_json::from_str(json).unwrap();
        assert!(query.depth.is_none());
    }

    // =========================================================================
    // ApiResponse Tests
    // =========================================================================

    #[test]
    fn test_api_response_serialization() {
        let response = ApiResponse { data: "test" };
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
            data: vec![1, 2, 3],
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"data\":[1,2,3]"));
    }

    #[test]
    fn test_api_response_debug() {
        let response = ApiResponse { data: 42 };
        let debug_str = format!("{:?}", response);
        assert!(debug_str.contains("ApiResponse"));
        assert!(debug_str.contains("42"));
    }
}
