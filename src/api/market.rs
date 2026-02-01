use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use crate::error::Result;
use crate::types::{FearGreedData, GlobalMetrics};
use crate::AppState;

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

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/global", get(get_global))
        .route("/fear-greed", get(get_fear_greed))
}
