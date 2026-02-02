//! Signal API endpoints.

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::types::{AccuracyResponse, PredictionsResponse, Recommendation, SignalAccuracy, SymbolSignals, TradingTimeframe};
use crate::AppState;

/// API response wrapper.
#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub data: T,
    pub meta: ApiMeta,
}

#[derive(Serialize)]
pub struct ApiMeta {
    pub cached: bool,
}

impl<T> ApiResponse<T> {
    fn new(data: T) -> Self {
        Self {
            data,
            meta: ApiMeta { cached: false },
        }
    }
}

/// Query parameters for signals endpoint.
#[derive(Debug, Deserialize)]
pub struct SignalsQuery {
    /// Trading timeframe: scalping, day_trading, swing_trading, position_trading
    pub timeframe: Option<String>,
}

/// Query parameters for predictions endpoint.
#[derive(Debug, Deserialize)]
pub struct PredictionsQuery {
    /// Filter by status: "all", "validated", "pending"
    pub status: Option<String>,
    /// Limit number of results (default: 50, max: 100)
    pub limit: Option<usize>,
}

/// Create the signals router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/:symbol", get(get_signals))
        .route("/:symbol/generate", post(generate_predictions))
        .route("/:symbol/recommendation", get(get_recommendation))
        .route("/:symbol/accuracy", get(get_symbol_accuracy))
        .route("/:symbol/predictions", get(get_symbol_predictions))
        .route("/accuracy/:indicator", get(get_indicator_accuracy))
}

/// Get all signals for a symbol.
async fn get_signals(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<SignalsQuery>,
) -> Result<Json<ApiResponse<SymbolSignals>>, (axum::http::StatusCode, String)> {
    // Parse timeframe, default to day trading
    let timeframe = query
        .timeframe
        .as_deref()
        .and_then(TradingTimeframe::from_str)
        .unwrap_or_default();

    let signals = state
        .signal_store
        .get_signals(&symbol, timeframe)
        .await
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            format!("No signals available for {}", symbol),
        ))?;

    Ok(Json(ApiResponse::new(signals)))
}

/// Generate predictions for a symbol immediately.
/// This bypasses the cache to force new prediction generation.
async fn generate_predictions(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<SignalsQuery>,
) -> Result<Json<ApiResponse<SymbolSignals>>, (axum::http::StatusCode, String)> {
    // Parse timeframe, default to day trading
    let timeframe = query
        .timeframe
        .as_deref()
        .and_then(TradingTimeframe::from_str)
        .unwrap_or_default();

    // Invalidate cache to force fresh computation
    state.signal_store.invalidate(&symbol);

    let signals = state
        .signal_store
        .get_signals(&symbol, timeframe)
        .await
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            format!("No chart data available for {}", symbol),
        ))?;

    Ok(Json(ApiResponse::new(signals)))
}

/// Get accuracy stats for a symbol.
async fn get_symbol_accuracy(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Json<ApiResponse<AccuracyResponse>> {
    let accuracies = state
        .signal_store
        .accuracy_store()
        .get_symbol_accuracies(&symbol);

    Json(ApiResponse::new(AccuracyResponse {
        symbol: symbol.to_uppercase(),
        accuracies,
        timestamp: chrono::Utc::now().timestamp_millis(),
    }))
}

/// Get predictions for a symbol with optional filtering.
async fn get_symbol_predictions(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<PredictionsQuery>,
) -> Json<ApiResponse<PredictionsResponse>> {
    let limit = query.limit.unwrap_or(50).min(500); // Increased max to 500 for historical queries
    let status = query.status.as_deref();

    // First try to get from SQLite for complete historical data
    let mut predictions = state
        .sqlite_store
        .get_predictions(&symbol, status, limit);

    // If SQLite returned nothing, fall back to in-memory store
    if predictions.is_empty() {
        predictions = state
            .signal_store
            .prediction_store()
            .get_predictions(&symbol);

        // Filter by status if specified
        predictions = match status {
            Some("validated") => predictions
                .into_iter()
                .filter(|p| {
                    // Validated = has ANY outcome (5m, 1h, 4h, or 24h)
                    p.outcome_5m.is_some() || p.outcome_1h.is_some() ||
                    p.outcome_4h.is_some() || p.outcome_24h.is_some()
                })
                .collect(),
            Some("pending") => predictions
                .into_iter()
                .filter(|p| {
                    // Pending = no outcomes yet
                    p.outcome_5m.is_none() && p.outcome_1h.is_none() &&
                    p.outcome_4h.is_none() && p.outcome_24h.is_none()
                })
                .collect(),
            _ => predictions,
        };

        // Apply limit
        predictions.truncate(limit);
    }

    Json(ApiResponse::new(PredictionsResponse {
        symbol: symbol.to_uppercase(),
        predictions,
        timestamp: chrono::Utc::now().timestamp_millis(),
    }))
}

/// Get global accuracy for an indicator.
async fn get_indicator_accuracy(
    State(state): State<AppState>,
    Path(indicator): Path<String>,
) -> Json<ApiResponse<Vec<SignalAccuracy>>> {
    let accuracies = state
        .signal_store
        .accuracy_store()
        .get_indicator_accuracies(&indicator);

    Json(ApiResponse::new(accuracies))
}

/// Get accuracy-weighted recommendation for a symbol.
async fn get_recommendation(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<SignalsQuery>,
) -> Result<Json<ApiResponse<Recommendation>>, (axum::http::StatusCode, String)> {
    // Parse timeframe, default to day trading
    let timeframe = query
        .timeframe
        .as_deref()
        .and_then(TradingTimeframe::from_str)
        .unwrap_or_default();

    let recommendation = state
        .signal_store
        .get_recommendation(&symbol, timeframe)
        .await
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            format!("Cannot generate recommendation for {}", symbol),
        ))?;

    Ok(Json(ApiResponse::new(recommendation)))
}
