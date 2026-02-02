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

/// Sort field options for listings.
#[derive(Debug, Deserialize, Clone, Copy, Default)]
#[serde(rename_all = "snake_case")]
pub enum SortField {
    #[default]
    MarketCap,
    Price,
    Volume24h,
    PercentChange1h,
    PercentChange24h,
    PercentChange7d,
    Name,
}

/// Sort direction.
#[derive(Debug, Deserialize, Clone, Copy, Default)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    Asc,
    #[default]
    Desc,
}

/// Filter options for listings.
#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum ListingFilter {
    All,
    Gainers,
    Losers,
    MostVolatile,
    TopVolume,
}

/// Asset type for filtering.
#[derive(Debug, Deserialize, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AssetType {
    #[default]
    All,
    Crypto,
    Stock,
    Etf,
    Forex,
    Commodity,
}

#[derive(Debug, Deserialize)]
pub struct ListingsQuery {
    start: Option<i32>,
    limit: Option<i32>,
    sort: Option<SortField>,
    sort_dir: Option<SortDirection>,
    filter: Option<ListingFilter>,
    asset_type: Option<AssetType>,
    /// Minimum 24h percent change (for filtering)
    min_change: Option<f64>,
    /// Maximum 24h percent change (for filtering)
    max_change: Option<f64>,
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
    let asset_type = params.asset_type.unwrap_or_default();

    // Use asset service for unified listings
    let (mut data, total) = state
        .asset_service
        .get_listings(asset_type, page, limit)
        .await
        .map_err(|e| crate::error::AppError::Internal(e))?;

    // Filter by listing filter type
    if let Some(filter) = params.filter {
        data = match filter {
            ListingFilter::All => data,
            ListingFilter::Gainers => data.into_iter().filter(|a| a.change_24h > 0.0).collect(),
            ListingFilter::Losers => data.into_iter().filter(|a| a.change_24h < 0.0).collect(),
            ListingFilter::MostVolatile => {
                // Sort by absolute change, take most volatile
                let mut volatile = data;
                volatile.sort_by(|a, b| {
                    b.change_24h.abs().partial_cmp(&a.change_24h.abs()).unwrap_or(std::cmp::Ordering::Equal)
                });
                volatile
            }
            ListingFilter::TopVolume => {
                let mut by_volume = data;
                by_volume.sort_by(|a, b| {
                    b.volume_24h.partial_cmp(&a.volume_24h).unwrap_or(std::cmp::Ordering::Equal)
                });
                by_volume
            }
        };
    }

    // Filter by percent change range
    if let Some(min) = params.min_change {
        data.retain(|a| a.change_24h >= min);
    }
    if let Some(max) = params.max_change {
        data.retain(|a| a.change_24h <= max);
    }

    // Populate sparklines from chart store if empty
    for listing in &mut data {
        if listing.sparkline.is_empty() {
            let symbol = listing.symbol.to_lowercase();
            let sparkline = state.chart_store.generate_sparkline_from_history(&symbol, 60);
            if !sparkline.is_empty() {
                listing.sparkline = sparkline;
            }
        }
    }

    // Apply sorting
    let sort_field = params.sort.unwrap_or_default();
    let sort_dir = params.sort_dir.unwrap_or_default();

    data.sort_by(|a, b| {
        let cmp = match sort_field {
            SortField::MarketCap => b.market_cap.partial_cmp(&a.market_cap),
            SortField::Price => b.price.partial_cmp(&a.price),
            SortField::Volume24h => b.volume_24h.partial_cmp(&a.volume_24h),
            SortField::PercentChange1h => b.change_1h.partial_cmp(&a.change_1h),
            SortField::PercentChange24h => b.change_24h.partial_cmp(&a.change_24h),
            SortField::PercentChange7d => b.change_7d.partial_cmp(&a.change_7d),
            SortField::Name => a.name.cmp(&b.name).into(),
        };
        let ordering = cmp.unwrap_or(std::cmp::Ordering::Equal);
        match sort_dir {
            SortDirection::Desc => ordering,
            SortDirection::Asc => ordering.reverse(),
        }
    });

    Ok(Json(ApiResponse {
        data,
        meta: ApiMeta::with_pagination(start, limit, total),
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
) -> Result<Json<ApiResponse<AssetListing>>> {
    // First try to get from CoinMarketCap (crypto)
    if let Ok(Some(asset)) = state.cmc_client.get_asset(id).await {
        // Convert Asset to AssetListing format that frontend expects
        let quote = asset.quote.as_ref();
        let symbol = asset.symbol.to_lowercase();

        // Get sparkline from chart store (168 points = 7 days of hourly data)
        let sparkline = state.chart_store.get_sparkline(&symbol, 168);

        // Get trade direction from price cache
        let trade_direction = state.coordinator.price_cache().get_trade_direction(&symbol);

        let listing = AssetListing {
            id: asset.id,
            rank: asset.rank.unwrap_or(0),
            name: asset.name,
            symbol: asset.symbol,
            image: asset.logo.unwrap_or_else(|| format!(
                "https://s2.coinmarketcap.com/static/img/coins/64x64/{}.png",
                asset.id
            )),
            price: quote.map(|q| q.price).unwrap_or(0.0),
            change_1h: quote.and_then(|q| q.percent_change_1h).unwrap_or(0.0),
            change_24h: quote.and_then(|q| q.percent_change_24h).unwrap_or(0.0),
            change_7d: quote.and_then(|q| q.percent_change_7d).unwrap_or(0.0),
            market_cap: quote.and_then(|q| q.market_cap).unwrap_or(0.0),
            volume_24h: quote.and_then(|q| q.volume_24h).unwrap_or(0.0),
            circulating_supply: quote.and_then(|q| q.circulating_supply).unwrap_or(0.0),
            max_supply: quote.and_then(|q| q.max_supply),
            sparkline,
            trade_direction,
            asset_type: "crypto".to_string(),
            exchange: None,
            sector: None,
        };

        return Ok(Json(ApiResponse {
            data: listing,
            meta: ApiMeta::simple(),
        }));
    }

    // Fall back to stocks/ETFs from asset service
    if let Some(mut listing) = state.asset_service.get_stock_or_etf_by_id(id).await {
        let symbol = listing.symbol.to_lowercase();

        // Get sparkline from chart store
        let sparkline = state.chart_store.get_sparkline(&symbol, 168);
        if !sparkline.is_empty() {
            listing.sparkline = sparkline;
        }

        // Get trade direction from price cache
        listing.trade_direction = state.coordinator.price_cache().get_trade_direction(&symbol);

        return Ok(Json(ApiResponse {
            data: listing,
            meta: ApiMeta::simple(),
        }));
    }

    Err(AppError::NotFound(format!("Asset {} not found", id)))
}

/// GET /api/crypto/:id/quotes
async fn get_quotes(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<Quote>>> {
    // First try crypto from CMC
    if let Ok(Some(asset)) = state.cmc_client.get_asset(id).await {
        let quote = asset.quote
            .ok_or_else(|| AppError::NotFound("Quote not available".to_string()))?;
        return Ok(Json(ApiResponse {
            data: quote,
            meta: ApiMeta::simple(),
        }));
    }

    // Fall back to stocks/ETFs - convert AssetListing to Quote format
    if let Some(listing) = state.asset_service.get_stock_or_etf_by_id(id).await {
        let quote = Quote {
            price: listing.price,
            volume_24h: Some(listing.volume_24h),
            volume_change_24h: None,
            percent_change_1h: Some(listing.change_1h),
            percent_change_24h: Some(listing.change_24h),
            percent_change_7d: Some(listing.change_7d),
            percent_change_30d: None,
            market_cap: Some(listing.market_cap),
            market_cap_dominance: None,
            fully_diluted_market_cap: None,
            circulating_supply: Some(listing.circulating_supply),
            total_supply: None,
            max_supply: listing.max_supply,
            last_updated: None,
        };
        return Ok(Json(ApiResponse {
            data: quote,
            meta: ApiMeta::simple(),
        }));
    }

    Err(AppError::NotFound(format!("Asset {} not found", id)))
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

    // Get the asset to find the symbol - try crypto first, then stocks/ETFs
    let symbol = if let Ok(Some(asset)) = state.cmc_client.get_asset(id).await {
        asset.symbol.to_lowercase()
    } else if let Some(listing) = state.asset_service.get_stock_or_etf_by_id(id).await {
        listing.symbol.to_lowercase()
    } else {
        return Err(AppError::NotFound(format!("Asset {} not found", id)));
    };

    // Get current chart data
    let data = state.chart_store.get_chart(&symbol, range);

    // Get current seeding status
    let status = state.historical_service.get_seed_status(&symbol);

    // Auto-trigger seeding if data is empty and not already seeding/seeded
    let should_auto_seed = data.is_empty()
        && (status == crate::services::SeedStatus::NotSeeded || status == crate::services::SeedStatus::Failed);

    if should_auto_seed {
        tracing::info!("Auto-triggering historical data seed for {} (empty chart data)", symbol);
        let service = state.historical_service.clone();
        let symbol_clone = symbol.clone();
        tokio::spawn(async move {
            service.seed_historical_data(symbol_clone).await;
        });
    } else if state.historical_service.should_seed(&symbol, range_str).await {
        // Regular seeding check for inadequate data
        tracing::info!("Triggering historical data seed for {} (range: {})", symbol, range_str);
        let service = state.historical_service.clone();
        let symbol_clone = symbol.clone();
        tokio::spawn(async move {
            service.seed_historical_data(symbol_clone).await;
        });
    }

    // Get updated status after potential triggering
    let status = if should_auto_seed {
        crate::services::SeedStatus::Seeding
    } else {
        state.historical_service.get_seed_status(&symbol)
    };

    // Convert status to string for frontend
    let seeding_status = match status {
        crate::services::SeedStatus::NotSeeded => "not_started",
        crate::services::SeedStatus::Seeding => "in_progress",
        crate::services::SeedStatus::Seeded => "complete",
        crate::services::SeedStatus::Failed => "failed",
    };

    let seeding = status == crate::services::SeedStatus::Seeding;

    // Calculate data completeness
    let expected_points = match range {
        ChartRange::OneHour => 60,      // 1-minute buckets for 1 hour
        ChartRange::FourHours => 48,    // 5-minute buckets for 4 hours
        ChartRange::OneDay => 288,      // 5-minute buckets for 24 hours
        ChartRange::OneWeek => 168,     // 1-hour buckets for 7 days
        ChartRange::OneMonth => 720,    // 1-hour buckets for 30 days
    };

    let data_completeness = if expected_points > 0 {
        ((data.len() as f64 / expected_points as f64) * 100.0).min(100.0) as u8
    } else {
        0
    };

    // Get actual seeding progress if available
    let seeding_progress = state.historical_service.get_seed_progress(&symbol)
        .map(|p| p.progress)
        .or_else(|| {
            // Fallback based on status
            match status {
                crate::services::SeedStatus::NotSeeded => Some(0),
                crate::services::SeedStatus::Seeding => Some(50),
                crate::services::SeedStatus::Seeded => Some(100),
                crate::services::SeedStatus::Failed => Some(0),
            }
        });

    Ok(Json(ApiResponse {
        data: ChartData {
            symbol,
            range: range_str.to_string(),
            data,
            seeding: Some(seeding),
            seeding_status: Some(seeding_status.to_string()),
            seeding_progress,
            data_completeness: Some(data_completeness),
            expected_points: Some(expected_points),
        },
        meta: ApiMeta::simple(),
    }))
}

/// Request body for seeding a single symbol.
#[derive(Debug, Deserialize)]
pub struct SeedRequest {
    symbol: String,
}

/// Request body for batch seeding.
#[derive(Debug, Deserialize)]
pub struct BatchSeedRequest {
    symbols: Vec<String>,
}

/// Response for seed status.
#[derive(Debug, Serialize)]
pub struct SeedResponse {
    symbol: String,
    status: String,
    message: String,
}

/// POST /api/crypto/seed - Trigger historical data seeding for a symbol
async fn seed_symbol(
    State(state): State<AppState>,
    Json(req): Json<SeedRequest>,
) -> Result<Json<ApiResponse<SeedResponse>>> {
    let symbol = req.symbol.to_lowercase();

    let status = state.historical_service.get_seed_status(&symbol);
    let status_str = match status {
        crate::services::SeedStatus::NotSeeded => "not_seeded",
        crate::services::SeedStatus::Seeding => "seeding",
        crate::services::SeedStatus::Seeded => "seeded",
        crate::services::SeedStatus::Failed => "failed",
    };

    // Only start seeding if not already seeding or seeded
    if status == crate::services::SeedStatus::NotSeeded || status == crate::services::SeedStatus::Failed {
        let service = state.historical_service.clone();
        let symbol_clone = symbol.clone();
        tokio::spawn(async move {
            service.seed_historical_data(symbol_clone).await;
        });

        Ok(Json(ApiResponse {
            data: SeedResponse {
                symbol,
                status: "seeding".to_string(),
                message: "Historical data seeding started".to_string(),
            },
            meta: ApiMeta::simple(),
        }))
    } else {
        Ok(Json(ApiResponse {
            data: SeedResponse {
                symbol,
                status: status_str.to_string(),
                message: format!("Seeding already {}", status_str),
            },
            meta: ApiMeta::simple(),
        }))
    }
}

/// POST /api/crypto/seed/batch - Trigger historical data seeding for multiple symbols
async fn seed_batch(
    State(state): State<AppState>,
    Json(req): Json<BatchSeedRequest>,
) -> Result<Json<ApiResponse<Vec<SeedResponse>>>> {
    let mut responses = Vec::new();

    for symbol in req.symbols {
        let symbol_lower = symbol.to_lowercase();
        let status = state.historical_service.get_seed_status(&symbol_lower);

        if status == crate::services::SeedStatus::NotSeeded || status == crate::services::SeedStatus::Failed {
            let service = state.historical_service.clone();
            let symbol_clone = symbol_lower.clone();
            tokio::spawn(async move {
                service.seed_historical_data(symbol_clone).await;
            });

            responses.push(SeedResponse {
                symbol: symbol_lower,
                status: "seeding".to_string(),
                message: "Seeding started".to_string(),
            });
        } else {
            let status_str = match status {
                crate::services::SeedStatus::Seeding => "seeding",
                crate::services::SeedStatus::Seeded => "seeded",
                _ => "unknown",
            };
            responses.push(SeedResponse {
                symbol: symbol_lower,
                status: status_str.to_string(),
                message: format!("Already {}", status_str),
            });
        }
    }

    Ok(Json(ApiResponse {
        data: responses,
        meta: ApiMeta::simple(),
    }))
}

/// GET /api/crypto/seed/status - Get seeding status for all known symbols
async fn seed_status(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<SeedResponse>>>> {
    // Get status for common symbols
    let symbols = vec![
        "btc", "eth", "bnb", "xrp", "ada", "doge", "sol", "dot", "matic", "ltc",
        "shib", "trx", "avax", "link", "atom", "uni", "xlm", "etc", "bch", "fil",
    ];

    let responses: Vec<SeedResponse> = symbols
        .into_iter()
        .map(|s| {
            let status = state.historical_service.get_seed_status(s);
            let status_str = match status {
                crate::services::SeedStatus::NotSeeded => "not_seeded",
                crate::services::SeedStatus::Seeding => "seeding",
                crate::services::SeedStatus::Seeded => "seeded",
                crate::services::SeedStatus::Failed => "failed",
            };
            SeedResponse {
                symbol: s.to_string(),
                status: status_str.to_string(),
                message: String::new(),
            }
        })
        .collect();

    Ok(Json(ApiResponse {
        data: responses,
        meta: ApiMeta::simple(),
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/listings", get(get_listings))
        .route("/search", get(search))
        .route("/seed", axum::routing::post(seed_symbol))
        .route("/seed/batch", axum::routing::post(seed_batch))
        .route("/seed/status", get(seed_status))
        .route("/:id", get(get_asset))
        .route("/:id/quotes", get(get_quotes))
        .route("/:id/chart", get(get_chart))
}
