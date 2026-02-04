use crate::error::Result;
use crate::services::price_cache::{ExchangeStats, SymbolConfidence, SymbolSourceStat};
use crate::types::{FearGreedData, GlobalMetrics, MoverTimeframe, MoversResponse};
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

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
async fn get_global(State(state): State<AppState>) -> Result<Json<ApiResponse<GlobalMetrics>>> {
    let metrics = state.cmc_client.get_global_metrics().await?;
    Ok(Json(ApiResponse {
        data: metrics,
        meta: ApiMeta::simple(),
    }))
}

/// GET /api/market/fear-greed
async fn get_fear_greed(State(state): State<AppState>) -> Result<Json<ApiResponse<FearGreedData>>> {
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
async fn get_stats(State(state): State<AppState>) -> Result<Json<ApiResponse<StatsResponse>>> {
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
    /// Asset type filter: "all", "crypto", "stock"
    #[serde(default)]
    pub asset_type: Option<String>,
}

fn default_limit() -> usize {
    10
}

/// GET /api/market/movers
async fn get_movers(
    State(state): State<AppState>,
    Query(query): Query<MoversQuery>,
) -> Result<Json<ApiResponse<MoversResponse>>> {
    use crate::sources::finnhub::{ETF_SYMBOLS, STOCK_SYMBOLS};
    use std::collections::HashSet;

    let timeframe = query
        .timeframe
        .as_deref()
        .and_then(|s| s.parse::<MoverTimeframe>().ok())
        .unwrap_or_default();

    let limit = query.limit.clamp(1, 50);

    // Build symbol filter based on asset_type
    let symbol_filter: Option<HashSet<String>> = match query.asset_type.as_deref() {
        Some("crypto") => {
            // Exclude stock and ETF symbols - filter will be None (include all)
            // but we'll create an exclusion set
            None // For crypto, we don't filter - just exclude stocks in the else branch
        }
        Some("stock") => {
            let filter: HashSet<String> = STOCK_SYMBOLS.iter().map(|s| s.to_lowercase()).collect();
            Some(filter)
        }
        Some("etf") => {
            let filter: HashSet<String> = ETF_SYMBOLS.iter().map(|s| s.to_lowercase()).collect();
            Some(filter)
        }
        _ => None, // "all" or not specified - include everything
    };

    // For crypto filter, we need to exclude stocks/ETFs
    let crypto_exclusion: Option<HashSet<String>> = if query.asset_type.as_deref() == Some("crypto")
    {
        let exclusion: HashSet<String> = STOCK_SYMBOLS
            .iter()
            .chain(ETF_SYMBOLS.iter())
            .map(|s| s.to_lowercase())
            .collect();
        Some(exclusion)
    } else {
        None
    };

    let (mut gainers, mut losers) =
        state
            .chart_store
            .get_top_movers(timeframe, limit * 2, symbol_filter.as_ref());

    // If crypto filter, exclude stocks/ETFs from results
    if let Some(ref exclusion) = crypto_exclusion {
        gainers.retain(|m| !exclusion.contains(&m.symbol.to_lowercase()));
        losers.retain(|m| !exclusion.contains(&m.symbol.to_lowercase()));
    }

    // Truncate to requested limit
    gainers.truncate(limit);
    losers.truncate(limit);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::price_cache::ConfidenceFactors;
    use crate::types::PriceSource;

    // =========================================================================
    // StatsResponse Tests
    // =========================================================================

    #[test]
    fn test_stats_response_serialization() {
        let response = StatsResponse {
            total_updates: 1_000_000,
            tps: 125.5,
            uptime_secs: 3600,
            active_symbols: 500,
            online_sources: 8,
            total_sources: 12,
            exchanges: vec![],
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"totalUpdates\":1000000"));
        assert!(json.contains("\"tps\":125.5"));
        assert!(json.contains("\"uptimeSecs\":3600"));
        assert!(json.contains("\"activeSymbols\":500"));
        assert!(json.contains("\"onlineSources\":8"));
        assert!(json.contains("\"totalSources\":12"));
    }

    #[test]
    fn test_stats_response_with_exchanges() {
        let response = StatsResponse {
            total_updates: 100,
            tps: 10.0,
            uptime_secs: 60,
            active_symbols: 10,
            online_sources: 2,
            total_sources: 2,
            exchanges: vec![
                ExchangeStats {
                    source: PriceSource::Binance,
                    update_count: 60,
                    update_percent: 60.0,
                    online: true,
                    last_error: None,
                    last_update: Some(1700000000000),
                },
                ExchangeStats {
                    source: PriceSource::Coinbase,
                    update_count: 40,
                    update_percent: 40.0,
                    online: true,
                    last_error: None,
                    last_update: Some(1700000000000),
                },
            ],
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"source\":\"binance\""));
        assert!(json.contains("\"source\":\"coinbase\""));
    }

    #[test]
    fn test_stats_response_debug() {
        let response = StatsResponse {
            total_updates: 0,
            tps: 0.0,
            uptime_secs: 0,
            active_symbols: 0,
            online_sources: 0,
            total_sources: 0,
            exchanges: vec![],
        };

        let debug_str = format!("{:?}", response);
        assert!(debug_str.contains("StatsResponse"));
    }

    // =========================================================================
    // ApiResponse and ApiMeta Tests
    // =========================================================================

    #[test]
    fn test_api_response_serialization() {
        let response = ApiResponse {
            data: 42,
            meta: ApiMeta::simple(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"data\":42"));
        assert!(json.contains("\"cached\":false"));
    }

    #[test]
    fn test_api_meta_simple() {
        let meta = ApiMeta::simple();
        assert!(!meta.cached);
    }

    #[test]
    fn test_api_meta_debug() {
        let meta = ApiMeta { cached: true };
        let debug_str = format!("{:?}", meta);
        assert!(debug_str.contains("ApiMeta"));
        assert!(debug_str.contains("true"));
    }

    // =========================================================================
    // SymbolSourceStatsResponse Tests
    // =========================================================================

    #[test]
    fn test_symbol_source_stats_response_serialization() {
        let response = SymbolSourceStatsResponse {
            symbol: "btc".to_string(),
            sources: vec![SymbolSourceStat {
                source: PriceSource::Binance,
                update_count: 1000,
                update_percent: 50.0,
                online: true,
            }],
            total_updates: 1000,
            timestamp: 1700000000,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"symbol\":\"btc\""));
        assert!(json.contains("\"totalUpdates\":1000"));
        assert!(json.contains("\"source\":\"binance\""));
    }

    #[test]
    fn test_symbol_source_stats_response_empty_sources() {
        let response = SymbolSourceStatsResponse {
            symbol: "unknown".to_string(),
            sources: vec![],
            total_updates: 0,
            timestamp: 1700000000,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"sources\":[]"));
        assert!(json.contains("\"totalUpdates\":0"));
    }

    // =========================================================================
    // ConfidenceResponse Tests
    // =========================================================================

    #[test]
    fn test_confidence_response_serialization() {
        let response = ConfidenceResponse {
            symbol: "eth".to_string(),
            confidence: SymbolConfidence {
                score: 85,
                source_count: 8,
                online_sources: 7,
                total_updates: 10000,
                current_price: Some(2500.0),
                price_spread_percent: Some(0.5),
                seconds_since_update: Some(5),
                factors: ConfidenceFactors {
                    source_diversity: 25,
                    update_frequency: 20,
                    data_recency: 22,
                    price_consistency: 18,
                },
            },
            chart_data_points: 1440,
            timestamp: 1700000000,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"symbol\":\"eth\""));
        assert!(json.contains("\"chartDataPoints\":1440"));
    }

    #[test]
    fn test_confidence_response_debug() {
        let response = ConfidenceResponse {
            symbol: "test".to_string(),
            confidence: SymbolConfidence {
                score: 30,
                source_count: 1,
                online_sources: 1,
                total_updates: 10,
                current_price: Some(100.0),
                price_spread_percent: None,
                seconds_since_update: Some(60),
                factors: ConfidenceFactors {
                    source_diversity: 5,
                    update_frequency: 10,
                    data_recency: 10,
                    price_consistency: 5,
                },
            },
            chart_data_points: 0,
            timestamp: 0,
        };

        let debug_str = format!("{:?}", response);
        assert!(debug_str.contains("ConfidenceResponse"));
    }

    // =========================================================================
    // MoversQuery Tests
    // =========================================================================

    #[test]
    fn test_movers_query_default() {
        let query = MoversQuery {
            timeframe: None,
            limit: default_limit(),
            asset_type: None,
        };

        assert!(query.timeframe.is_none());
        assert_eq!(query.limit, 10);
        assert!(query.asset_type.is_none());
    }

    #[test]
    fn test_movers_query_with_values() {
        let query = MoversQuery {
            timeframe: Some("1h".to_string()),
            limit: 25,
            asset_type: Some("crypto".to_string()),
        };

        assert_eq!(query.timeframe, Some("1h".to_string()));
        assert_eq!(query.limit, 25);
        assert_eq!(query.asset_type, Some("crypto".to_string()));
    }

    #[test]
    fn test_movers_query_deserialization() {
        let json = r#"{"timeframe": "24h", "limit": 20, "asset_type": "stock"}"#;
        let query: MoversQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.timeframe, Some("24h".to_string()));
        assert_eq!(query.limit, 20);
        assert_eq!(query.asset_type, Some("stock".to_string()));
    }

    #[test]
    fn test_movers_query_deserialization_defaults() {
        let json = r#"{}"#;
        let query: MoversQuery = serde_json::from_str(json).unwrap();
        assert!(query.timeframe.is_none());
        assert_eq!(query.limit, 10); // default_limit
        assert!(query.asset_type.is_none());
    }

    #[test]
    fn test_movers_query_debug() {
        let query = MoversQuery {
            timeframe: Some("5m".to_string()),
            limit: 5,
            asset_type: Some("etf".to_string()),
        };

        let debug_str = format!("{:?}", query);
        assert!(debug_str.contains("MoversQuery"));
        assert!(debug_str.contains("5m"));
    }

    #[test]
    fn test_default_limit() {
        assert_eq!(default_limit(), 10);
    }
}
