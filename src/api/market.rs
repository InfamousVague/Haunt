use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use crate::error::Result;
use crate::services::price_cache::{ExchangeStats, SymbolConfidence, SymbolSourceStat};
use crate::types::{FearGreedData, GlobalMetrics, MoverTimeframe, MoversResponse};
use crate::AppState;

/// Stats response for total updates tracked.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsResponse {
    pub total_updates: u64,
    /// Transactions per second (average over last 60 seconds).
    pub tps: f64,
    /// Server uptime in seconds.
    pub uptime_secs: u64,
    /// Number of active symbols being tracked.
    pub active_symbols: usize,
    /// Number of online sources.
    pub online_sources: usize,
    /// Total number of sources.
    pub total_sources: usize,
    /// Per-exchange statistics.
    pub exchanges: Vec<ExchangeStats>,
}

/// API response wrapper matching frontend expectations
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub data: T,
    pub meta: ApiMeta,
}

#[derive(Debug, Serialize)]
pub struct ApiMeta {
    pub cached: bool,
}

impl ApiMeta {
    fn simple() -> Self {
        Self { cached: false }
    }
}

/// GET /api/market/global
async fn get_global(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<GlobalMetrics>>> {
    let metrics = state.cmc_client.get_global_metrics().await?;
    Ok(Json(ApiResponse {
        data: metrics,
        meta: ApiMeta::simple(),
    }))
}

/// GET /api/market/fear-greed
async fn get_fear_greed(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<FearGreedData>>> {
    let data = state.cmc_client.get_fear_greed().await?;
    Ok(Json(ApiResponse {
        data,
        meta: ApiMeta::simple(),
    }))
}

/// GET /api/market/exchanges
async fn get_exchanges(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<ExchangeStats>>>> {
    let stats = state.price_cache.get_exchange_stats();
    Ok(Json(ApiResponse {
        data: stats,
        meta: ApiMeta::simple(),
    }))
}

/// GET /api/market/stats
async fn get_stats(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<StatsResponse>>> {
    let total_updates = state.price_cache.get_total_updates();
    let tps = state.price_cache.get_tps();
    let uptime_secs = state.price_cache.get_uptime_secs();
    let active_symbols = state.price_cache.get_active_symbols();
    let online_sources = state.price_cache.get_online_sources();
    let exchanges = state.price_cache.get_exchange_stats();
    let total_sources = exchanges.len();

    Ok(Json(ApiResponse {
        data: StatsResponse {
            total_updates,
            tps,
            uptime_secs,
            active_symbols,
            online_sources,
            total_sources,
            exchanges,
        },
        meta: ApiMeta::simple(),
    }))
}

/// Response for per-symbol source statistics.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolSourceStatsResponse {
    pub symbol: String,
    pub sources: Vec<SymbolSourceStat>,
    pub total_updates: u64,
    pub timestamp: i64,
}

/// GET /api/market/source-stats/:symbol
async fn get_symbol_source_stats(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<SymbolSourceStatsResponse>>> {
    let symbol_lower = symbol.to_lowercase();
    let sources = state.price_cache.get_symbol_source_stats(&symbol_lower);
    let total_updates: u64 = sources.iter().map(|s| s.update_count).sum();

    Ok(Json(ApiResponse {
        data: SymbolSourceStatsResponse {
            symbol: symbol_lower,
            sources,
            total_updates,
            timestamp: chrono::Utc::now().timestamp(),
        },
        meta: ApiMeta::simple(),
    }))
}

/// Response for symbol confidence.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfidenceResponse {
    pub symbol: String,
    pub confidence: SymbolConfidence,
    /// Number of chart data points available.
    pub chart_data_points: usize,
    pub timestamp: i64,
}

/// GET /api/market/confidence/:symbol
async fn get_symbol_confidence(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Result<Json<ApiResponse<ConfidenceResponse>>> {
    let symbol_lower = symbol.to_lowercase();
    let confidence = state.price_cache.get_symbol_confidence(&symbol_lower);

    // Get chart data point count from chart_store
    let chart_data_points = state.chart_store.get_data_point_count(&symbol_lower);

    Ok(Json(ApiResponse {
        data: ConfidenceResponse {
            symbol: symbol_lower,
            confidence,
            chart_data_points,
            timestamp: chrono::Utc::now().timestamp(),
        },
        meta: ApiMeta::simple(),
    }))
}

/// Query params for movers endpoint.
#[derive(Debug, Deserialize)]
pub struct MoversQuery {
    #[serde(default)]
    pub timeframe: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    10
}

/// GET /api/market/movers
async fn get_movers(
    State(state): State<AppState>,
    Query(query): Query<MoversQuery>,
) -> Result<Json<ApiResponse<MoversResponse>>> {
    let timeframe = query
        .timeframe
        .as_deref()
        .and_then(|s| s.parse::<MoverTimeframe>().ok())
        .unwrap_or_default();

    let limit = query.limit.min(50).max(1);

    let (gainers, losers) = state.chart_store.get_top_movers(timeframe, limit);

    Ok(Json(ApiResponse {
        data: MoversResponse {
            timeframe: timeframe.to_string(),
            gainers,
            losers,
            timestamp: chrono::Utc::now().timestamp(),
        },
        meta: ApiMeta::simple(),
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/global", get(get_global))
        .route("/fear-greed", get(get_fear_greed))
        .route("/exchanges", get(get_exchanges))
        .route("/stats", get(get_stats))
        .route("/movers", get(get_movers))
        .route("/source-stats/:symbol", get(get_symbol_source_stats))
        .route("/confidence/:symbol", get(get_symbol_confidence))
}
