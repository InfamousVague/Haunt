//! Signal API endpoints.

use axum::{
    extract::{Path, Query, State},
    routing::get,
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

/// Create the signals router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/:symbol", get(get_signals))
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

/// Get predictions for a symbol.
async fn get_symbol_predictions(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Json<ApiResponse<PredictionsResponse>> {
    let predictions = state
        .signal_store
        .prediction_store()
        .get_predictions(&symbol);

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
