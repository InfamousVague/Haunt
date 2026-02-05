//! Exchange metrics API endpoints.
//!
//! Provides endpoints for:
//! - Exchange latency statistics
//! - Volume dominance tracking
//! - Exchange health status

use crate::services::exchange_metrics::{ExchangeHealth, ExchangeMetrics, LatencyStats, VolumeStats};
use crate::types::PriceSource;
use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Serialize;

/// Create exchange metrics API router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_all_metrics))
        .route("/latency", get(get_all_latency))
        .route("/latency/:source", get(get_source_latency))
        .route("/volume", get(get_all_volume))
        .route("/health", get(get_all_health))
        .route("/health/:source", get(get_source_health))
        .route("/:source", get(get_source_metrics))
        .route("/summary", get(get_metrics_summary))
}

/// All exchange metrics response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllMetricsResponse {
    pub exchanges: Vec<ExchangeMetrics>,
    pub exchange_count: usize,
    pub timestamp: i64,
}

/// Get metrics for all exchanges.
pub async fn get_all_metrics(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let metrics = state.exchange_metrics.get_all_metrics();
    let exchange_count = metrics.len();

    Ok(Json(AllMetricsResponse {
        exchanges: metrics,
        exchange_count,
        timestamp: chrono::Utc::now().timestamp_millis(),
    }))
}

/// Latency summary response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllLatencyResponse {
    pub exchanges: Vec<ExchangeLatencyInfo>,
    pub aggregated: LatencyStats,
    pub timestamp: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeLatencyInfo {
    pub source: PriceSource,
    pub latency: LatencyStats,
}

/// Get latency statistics for all exchanges.
pub async fn get_all_latency(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let sources = state.exchange_metrics.tracked_sources();

    let exchanges: Vec<ExchangeLatencyInfo> = sources
        .into_iter()
        .map(|source| ExchangeLatencyInfo {
            source,
            latency: state.exchange_metrics.get_latency_stats(source),
        })
        .collect();

    let aggregated = state.exchange_metrics.get_aggregated_latency();

    Ok(Json(AllLatencyResponse {
        exchanges,
        aggregated,
        timestamp: chrono::Utc::now().timestamp_millis(),
    }))
}

/// Single exchange latency response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceLatencyResponse {
    pub source: String,
    pub latency: LatencyStats,
    pub timestamp: i64,
}

/// Get latency statistics for a specific exchange.
pub async fn get_source_latency(
    State(state): State<AppState>,
    Path(source): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let price_source = parse_price_source(&source)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, format!("Unknown source: {}", source)))?;

    let latency = state.exchange_metrics.get_latency_stats(price_source);

    Ok(Json(SourceLatencyResponse {
        source,
        latency,
        timestamp: chrono::Utc::now().timestamp_millis(),
    }))
}

/// Volume dominance response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllVolumeResponse {
    pub exchanges: Vec<ExchangeVolumeInfo>,
    pub total_volume_usd: f64,
    pub timestamp: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeVolumeInfo {
    pub source: PriceSource,
    pub volume: VolumeStats,
}

/// Get volume dominance for all exchanges.
pub async fn get_all_volume(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let volumes = state.exchange_metrics.get_all_volume_stats();

    let total_volume_usd: f64 = volumes.iter().map(|(_, v)| v.volume_24h_usd).sum();

    let exchanges: Vec<ExchangeVolumeInfo> = volumes
        .into_iter()
        .map(|(source, volume)| ExchangeVolumeInfo { source, volume })
        .collect();

    Ok(Json(AllVolumeResponse {
        exchanges,
        total_volume_usd,
        timestamp: chrono::Utc::now().timestamp_millis(),
    }))
}

/// Health status response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllHealthResponse {
    pub exchanges: Vec<ExchangeHealthInfo>,
    pub online_count: usize,
    pub total_count: usize,
    pub timestamp: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeHealthInfo {
    pub source: PriceSource,
    pub health: ExchangeHealth,
}

/// Get health status for all exchanges.
pub async fn get_all_health(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let sources = state.exchange_metrics.tracked_sources();

    let exchanges: Vec<ExchangeHealthInfo> = sources
        .iter()
        .map(|&source| ExchangeHealthInfo {
            source,
            health: state.exchange_metrics.get_health(source),
        })
        .collect();

    let online_count = exchanges.iter().filter(|e| e.health.online).count();
    let total_count = exchanges.len();

    Ok(Json(AllHealthResponse {
        exchanges,
        online_count,
        total_count,
        timestamp: chrono::Utc::now().timestamp_millis(),
    }))
}

/// Single exchange health response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceHealthResponse {
    pub source: String,
    pub health: ExchangeHealth,
    pub timestamp: i64,
}

/// Get health status for a specific exchange.
pub async fn get_source_health(
    State(state): State<AppState>,
    Path(source): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let price_source = parse_price_source(&source)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, format!("Unknown source: {}", source)))?;

    let health = state.exchange_metrics.get_health(price_source);

    Ok(Json(SourceHealthResponse {
        source,
        health,
        timestamp: chrono::Utc::now().timestamp_millis(),
    }))
}

/// Single exchange metrics response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceMetricsResponse {
    pub metrics: ExchangeMetrics,
    pub timestamp: i64,
}

/// Get complete metrics for a specific exchange.
pub async fn get_source_metrics(
    State(state): State<AppState>,
    Path(source): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let price_source = parse_price_source(&source)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, format!("Unknown source: {}", source)))?;

    let metrics = state.exchange_metrics.get_metrics(price_source);

    Ok(Json(SourceMetricsResponse {
        metrics,
        timestamp: chrono::Utc::now().timestamp_millis(),
    }))
}

/// Summary metrics response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsSummaryResponse {
    /// Number of tracked exchanges.
    pub exchange_count: usize,
    /// Number of online exchanges.
    pub online_count: usize,
    /// Aggregated latency across all exchanges.
    pub avg_latency_ms: f64,
    /// Minimum latency across all exchanges.
    pub min_latency_ms: u64,
    /// Maximum latency across all exchanges.
    pub max_latency_ms: u64,
    /// Total 24h volume across all exchanges (USD).
    pub total_volume_24h_usd: f64,
    /// Top exchange by volume.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_volume_exchange: Option<PriceSource>,
    /// Top exchange by lowest latency.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fastest_exchange: Option<PriceSource>,
    pub timestamp: i64,
}

/// Get a summary of exchange metrics.
pub async fn get_metrics_summary(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let sources = state.exchange_metrics.tracked_sources();
    let exchange_count = sources.len();

    // Get health stats
    let online_count = sources
        .iter()
        .filter(|&s| state.exchange_metrics.get_health(*s).online)
        .count();

    // Get aggregated latency
    let agg_latency = state.exchange_metrics.get_aggregated_latency();

    // Get volume stats
    let volumes = state.exchange_metrics.get_all_volume_stats();
    let total_volume_24h_usd: f64 = volumes.iter().map(|(_, v)| v.volume_24h_usd).sum();

    // Find top volume exchange
    let top_volume_exchange = volumes
        .iter()
        .max_by(|(_, a), (_, b)| a.volume_24h_usd.partial_cmp(&b.volume_24h_usd).unwrap())
        .map(|(s, _)| *s);

    // Find fastest exchange (lowest avg latency)
    let fastest_exchange = sources
        .iter()
        .map(|&s| (s, state.exchange_metrics.get_latency_stats(s)))
        .filter(|(_, l)| l.sample_count > 0)
        .min_by(|(_, a), (_, b)| a.avg_ms.partial_cmp(&b.avg_ms).unwrap())
        .map(|(s, _)| s);

    Ok(Json(MetricsSummaryResponse {
        exchange_count,
        online_count,
        avg_latency_ms: agg_latency.avg_ms,
        min_latency_ms: agg_latency.min_ms,
        max_latency_ms: agg_latency.max_ms,
        total_volume_24h_usd,
        top_volume_exchange,
        fastest_exchange,
        timestamp: chrono::Utc::now().timestamp_millis(),
    }))
}

/// Parse a source string to PriceSource enum.
fn parse_price_source(source: &str) -> Option<PriceSource> {
    match source.to_lowercase().as_str() {
        "coinbase" => Some(PriceSource::Coinbase),
        "coingecko" => Some(PriceSource::CoinGecko),
        "cryptocompare" => Some(PriceSource::CryptoCompare),
        "coinmarketcap" | "cmc" => Some(PriceSource::CoinMarketCap),
        "binance" => Some(PriceSource::Binance),
        "kraken" => Some(PriceSource::Kraken),
        "kucoin" => Some(PriceSource::KuCoin),
        "okx" => Some(PriceSource::Okx),
        "huobi" => Some(PriceSource::Huobi),
        "hyperliquid" => Some(PriceSource::Hyperliquid),
        "finnhub" => Some(PriceSource::Finnhub),
        "alphavantage" => Some(PriceSource::AlphaVantage),
        "alpaca" => Some(PriceSource::Alpaca),
        "tiingo" => Some(PriceSource::Tiingo),
        _ => None,
    }
}
