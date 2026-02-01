use serde::{Deserialize, Serialize};

/// A cryptocurrency asset with full metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: i64,
    pub name: String,
    pub symbol: String,
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_added: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urls: Option<AssetUrls>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote: Option<Quote>,
}

/// URLs associated with an asset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetUrls {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explorer: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_code: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twitter: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reddit: Option<Vec<String>>,
}

/// Quote data for an asset.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    pub price: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_24h: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_change_24h: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_cap: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_cap_dominance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent_change_1h: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent_change_24h: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent_change_7d: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent_change_30d: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fully_diluted_market_cap: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub circulating_supply: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_supply: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_supply: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
}

/// A simplified asset listing for paginated results.
/// This matches the frontend's expected Asset type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetListing {
    pub id: i64,
    pub rank: i32,
    pub name: String,
    pub symbol: String,
    pub image: String,
    pub price: f64,
    pub change_1h: f64,
    pub change_24h: f64,
    pub change_7d: f64,
    pub market_cap: f64,
    pub volume_24h: f64,
    pub circulating_supply: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_supply: Option<f64>,
    pub sparkline: Vec<f64>,
}

/// Paginated response wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub page: i32,
    pub limit: i32,
    pub total: i32,
    #[serde(rename = "hasMore")]
    pub has_more: bool,
}

/// Search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub id: i64,
    pub name: String,
    pub symbol: String,
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmc_rank: Option<i32>,
}
