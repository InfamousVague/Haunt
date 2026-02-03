use serde::{Deserialize, Serialize};
use super::{AggregatedPrice, GlobalMetrics, PeerStatus, PriceSource, SignalDirection, TradeDirection};

/// Incoming WebSocket message from client.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Subscribe { assets: Vec<String> },
    Unsubscribe { assets: Vec<String> },
    /// Set throttle interval in milliseconds (0 = no throttling)
    SetThrottle { throttle_ms: u64 },
    /// Subscribe to peer status updates (real-time ping data)
    SubscribePeers,
    /// Unsubscribe from peer status updates
    UnsubscribePeers,
}

/// Outgoing WebSocket message to client.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    PriceUpdate { data: PriceUpdateData },
    MarketUpdate { data: MarketUpdateData },
    SeedingProgress { data: SeedingProgressData },
    SignalUpdate { data: SignalUpdateData },
    /// Real-time peer status update with latency info
    PeerUpdate { data: PeerUpdateData },
    Subscribed { assets: Vec<String> },
    Unsubscribed { assets: Vec<String> },
    ThrottleSet { throttle_ms: u64 },
    /// Confirmation of peer subscription
    PeersSubscribed,
    /// Confirmation of peer unsubscription
    PeersUnsubscribed,
    Error { error: String },
}

/// Signal update payload.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalUpdateData {
    pub symbol: String,
    pub composite_score: i8,
    pub direction: SignalDirection,
    pub trend_score: i8,
    pub momentum_score: i8,
    pub volatility_score: i8,
    pub volume_score: i8,
    pub timestamp: i64,
}

/// Seeding progress payload for chart data updates.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedingProgressData {
    pub symbol: String,
    /// Status: "in_progress", "complete", "failed"
    pub status: String,
    /// Progress percentage (0-100)
    pub progress: u8,
    /// Total data points when complete
    #[serde(skip_serializing_if = "Option::is_none")]
    pub points: Option<u64>,
    /// Optional message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trade_direction: Option<TradeDirection>,
    pub source: PriceSource,
    pub sources: Vec<PriceSource>,
    pub timestamp: i64,
    /// Asset type: "crypto", "stock", "etf"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_type: Option<String>,
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
            trade_direction: price.trade_direction,
            source: price.source,
            sources: price.sources,
            timestamp: price.timestamp,
            asset_type: Some("crypto".to_string()), // Default to crypto for existing sources
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

/// Peer status update payload for real-time server connectivity info.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerUpdateData {
    /// This server's ID.
    pub server_id: String,
    /// This server's region.
    pub server_region: String,
    /// Status of all peer connections.
    pub peers: Vec<PeerStatus>,
    /// Timestamp of this update.
    pub timestamp: i64,
}
