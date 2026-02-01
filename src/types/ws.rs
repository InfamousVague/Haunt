use serde::{Deserialize, Serialize};
use super::{AggregatedPrice, GlobalMetrics, PriceSource};

/// Incoming WebSocket message from client.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ClientMessage {
    Subscribe { assets: Vec<String> },
    Unsubscribe { assets: Vec<String> },
}

/// Outgoing WebSocket message to client.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    PriceUpdate { data: PriceUpdateData },
    MarketUpdate { data: MarketUpdateData },
    Subscribed { assets: Vec<String> },
    Unsubscribed { assets: Vec<String> },
    Error { error: String },
}

/// Price update payload.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceUpdateData {
    pub id: String,
    pub symbol: String,
    pub price: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_24h: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_24h: Option<f64>,
    pub source: PriceSource,
    pub sources: Vec<PriceSource>,
    pub timestamp: i64,
}

impl From<AggregatedPrice> for PriceUpdateData {
    fn from(price: AggregatedPrice) -> Self {
        Self {
            id: price.id,
            symbol: price.symbol,
            price: price.price,
            previous_price: price.previous_price,
            change_24h: price.change_24h,
            volume_24h: price.volume_24h,
            source: price.source,
            sources: price.sources,
            timestamp: price.timestamp,
        }
    }
}

/// Market update payload.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketUpdateData {
    pub total_market_cap: f64,
    pub total_volume_24h: f64,
    pub btc_dominance: f64,
    pub timestamp: i64,
}

impl From<GlobalMetrics> for MarketUpdateData {
    fn from(metrics: GlobalMetrics) -> Self {
        Self {
            total_market_cap: metrics.total_market_cap,
            total_volume_24h: metrics.total_volume_24h,
            btc_dominance: metrics.btc_dominance,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}
