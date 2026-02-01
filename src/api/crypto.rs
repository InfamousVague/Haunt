use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use crate::error::{AppError, Result};
use crate::types::{Asset, AssetListing, ChartData, ChartRange, Quote};
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
}

impl ApiMeta {
    fn simple() -> Self {
        Self {
            cached: false,
            total: None,
            start: None,
            limit: None,
            query: None,
        }
    }

    fn with_pagination(start: i32, limit: i32, total: i32) -> Self {
        Self {
            cached: false,
            total: Some(total),
            start: Some(start),
            limit: Some(limit),
            query: None,
        }
    }

    fn with_query(query: String, limit: i32) -> Self {
        Self {
            cached: false,
            total: None,
            start: None,
            limit: Some(limit),
            query: Some(query),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ListingsQuery {
    start: Option<i32>,
    limit: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    q: String,
    limit: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct ChartQuery {
    range: Option<String>,
}

/// GET /api/crypto/listings
async fn get_listings(
    State(state): State<AppState>,
    Query(params): Query<ListingsQuery>,
) -> Result<Json<ApiResponse<Vec<AssetListing>>>> {
    let start = params.start.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(20).min(100).max(1);
    let page = ((start - 1) / limit) + 1;

    let listings = state.cmc_client.get_listings(page, limit).await?;

    Ok(Json(ApiResponse {
        data: listings.data,
        meta: ApiMeta::with_pagination(start, limit, listings.total),
    }))
}

/// GET /api/crypto/search
async fn search(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<ApiResponse<Vec<AssetListing>>>> {
    let limit = params.limit.unwrap_or(10).min(50).max(1);
    let query = params.q.clone();

    let results = state.cmc_client.search(&params.q, limit).await?;

    Ok(Json(ApiResponse {
        data: results,
        meta: ApiMeta::with_query(query, limit),
    }))
}

/// GET /api/crypto/:id
async fn get_asset(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<Asset>>> {
    let asset = state.cmc_client.get_asset(id).await?
        .ok_or_else(|| AppError::NotFound(format!("Asset {} not found", id)))?;

    Ok(Json(ApiResponse {
        data: asset,
        meta: ApiMeta::simple(),
    }))
}

/// GET /api/crypto/:id/quotes
async fn get_quotes(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<Quote>>> {
    let asset = state.cmc_client.get_asset(id).await?
        .ok_or_else(|| AppError::NotFound(format!("Asset {} not found", id)))?;

    let quote = asset.quote
        .ok_or_else(|| AppError::NotFound("Quote not available".to_string()))?;

    Ok(Json(ApiResponse {
        data: quote,
        meta: ApiMeta::simple(),
    }))
}

/// GET /api/crypto/:id/chart
async fn get_chart(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(params): Query<ChartQuery>,
) -> Result<Json<ApiResponse<ChartData>>> {
    let range_str = params.range.as_deref().unwrap_or("1d");
    let range = ChartRange::from_str(range_str)
        .ok_or_else(|| AppError::BadRequest(format!("Invalid range: {}", range_str)))?;

    // Get the asset to find the symbol
    let asset = state.cmc_client.get_asset(id).await?
        .ok_or_else(|| AppError::NotFound(format!("Asset {} not found", id)))?;

    let symbol = asset.symbol.to_lowercase();

    let data = state.chart_store.get_chart(&symbol, range);

    Ok(Json(ApiResponse {
        data: ChartData {
            symbol,
            range: range_str.to_string(),
            data,
        },
        meta: ApiMeta::simple(),
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/listings", get(get_listings))
        .route("/search", get(search))
        .route("/:id", get(get_asset))
        .route("/:id/quotes", get(get_quotes))
        .route("/:id/chart", get(get_chart))
}
