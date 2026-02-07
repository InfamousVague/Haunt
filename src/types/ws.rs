use super::{
    AggregatedPrice, GlobalMetrics, GridConfig, GridlinePosition, Order, OrderStatus, PeerStatus,
    Portfolio, Position, PriceSource, RatConfig, RatStats, RatStatus, SignalDirection, Trade,
    TradeDirection,
};
use serde::{Deserialize, Serialize};

/// Incoming WebSocket message from client.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Subscribe {
        assets: Vec<String>,
    },
    Unsubscribe {
        assets: Vec<String>,
    },
    /// Set throttle interval in milliseconds (0 = no throttling)
    SetThrottle {
        throttle_ms: u64,
    },
    /// Subscribe to peer status updates (real-time ping data)
    SubscribePeers,
    /// Unsubscribe from peer status updates
    UnsubscribePeers,
    /// Peer mesh ping (for server-to-server latency measurement)
    Ping {
        from_id: String,
        from_region: String,
        timestamp: i64,
    },
    /// Peer mesh pong response
    Pong {
        from_id: String,
        from_region: String,
        original_timestamp: i64,
    },
    /// Peer mesh authentication
    #[allow(dead_code)]
    Auth {
        id: String,
        region: String,
        timestamp: i64,
        signature: String,
    },
    /// Peer mesh identification (no auth)
    Identify {
        id: String,
        region: String,
        version: String,
    },
    /// Peer mesh sync data (server-to-server sync)
    SyncData {
        from_id: String,
        data: String,
    },
    /// Subscribe to trading updates for a portfolio
    SubscribeTrading {
        portfolio_id: String,
    },
    /// Unsubscribe from trading updates
    UnsubscribeTrading {
        portfolio_id: String,
    },
    /// Subscribe to gridline trading updates for a symbol
    SubscribeGridline {
        symbol: String,
        #[serde(default)]
        portfolio_id: Option<String>,
    },
    /// Unsubscribe from gridline trading updates
    UnsubscribeGridline {
        symbol: String,
    },
}

/// Outgoing WebSocket message to client.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
pub enum ServerMessage {
    PriceUpdate {
        data: PriceUpdateData,
    },
    MarketUpdate {
        data: MarketUpdateData,
    },
    SeedingProgress {
        data: SeedingProgressData,
    },
    SignalUpdate {
        data: SignalUpdateData,
    },
    /// Real-time peer status update with latency info
    PeerUpdate {
        data: PeerUpdateData,
    },
    Subscribed {
        assets: Vec<String>,
    },
    Unsubscribed {
        assets: Vec<String>,
    },
    ThrottleSet {
        throttle_ms: u64,
    },
    /// Confirmation of peer subscription
    PeersSubscribed,
    /// Confirmation of peer unsubscription
    PeersUnsubscribed,
    Error {
        error: String,
    },
    /// Peer mesh pong response (echoes the ping)
    Pong {
        from_id: String,
        from_region: String,
        original_timestamp: i64,
    },
    /// Peer mesh auth response
    AuthResponse {
        success: bool,
        error: Option<String>,
    },
    /// Confirmation of trading subscription
    TradingSubscribed {
        portfolio_id: String,
    },
    /// Confirmation of trading unsubscription
    TradingUnsubscribed {
        portfolio_id: String,
    },
    /// Order update (created, filled, cancelled, etc.)
    OrderUpdate {
        data: OrderUpdateData,
    },
    /// Position update (opened, modified, P&L change)
    PositionUpdate {
        data: PositionUpdateData,
    },
    /// Portfolio update (balance, margin changes)
    PortfolioUpdate {
        data: PortfolioUpdateData,
    },
    /// Trade execution notification
    TradeExecution {
        data: TradeExecutionData,
    },
    /// Margin warning (approaching liquidation)
    MarginWarning {
        data: MarginWarningData,
    },
    /// Liquidation event
    LiquidationAlert {
        data: LiquidationAlertData,
    },
    /// RAT (Random Auto Trader) status update
    RatStatusUpdate {
        data: RatStatusUpdateData,
    },
    /// Gridline trade placed notification
    GridlineTradePlaced {
        data: GridlineTradePlacedData,
    },
    /// Gridline trade resolved (won or lost)
    GridlineTradeResolved {
        data: GridlineTradeResolvedData,
    },
    /// Grid multiplier matrix update
    GridMultiplierUpdate {
        data: GridMultiplierUpdateData,
    },
    /// Grid column expired with batch results
    GridColumnExpired {
        data: GridColumnExpiredData,
    },
    /// Confirmation of gridline subscription
    GridlineSubscribed {
        symbol: String,
    },
    /// Confirmation of gridline unsubscription
    GridlineUnsubscribed {
        symbol: String,
    },
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

// =============================================================================
// Trading WebSocket Data Types
// =============================================================================

/// Order update payload.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderUpdateData {
    /// The updated order.
    pub order: Order,
    /// Type of update.
    pub update_type: OrderUpdateType,
    /// Timestamp of the update.
    pub timestamp: i64,
}

/// Type of order update.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderUpdateType {
    Created,
    PartialFill,
    Filled,
    Cancelled,
    Expired,
    Rejected,
    Modified,
}

impl From<OrderStatus> for OrderUpdateType {
    fn from(status: OrderStatus) -> Self {
        match status {
            OrderStatus::Pending => OrderUpdateType::Created,
            OrderStatus::Open => OrderUpdateType::Created,
            OrderStatus::PartiallyFilled => OrderUpdateType::PartialFill,
            OrderStatus::Filled => OrderUpdateType::Filled,
            OrderStatus::Cancelled => OrderUpdateType::Cancelled,
            OrderStatus::Expired => OrderUpdateType::Expired,
            OrderStatus::Rejected => OrderUpdateType::Rejected,
        }
    }
}

/// Position update payload.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionUpdateData {
    /// The updated position.
    pub position: Position,
    /// Type of update.
    pub update_type: PositionUpdateType,
    /// Timestamp of the update.
    pub timestamp: i64,
}

/// Type of position update.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionUpdateType {
    Opened,
    Increased,
    Decreased,
    Modified,
    Closed,
    StopLossTriggered,
    TakeProfitTriggered,
    Liquidated,
    PnlChanged,
}

/// Portfolio update payload.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioUpdateData {
    /// The updated portfolio.
    pub portfolio: Portfolio,
    /// Type of update.
    pub update_type: PortfolioUpdateType,
    /// Timestamp of the update.
    pub timestamp: i64,
}

/// Type of portfolio update.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PortfolioUpdateType {
    BalanceChanged,
    MarginChanged,
    SettingsChanged,
    Reset,
}

/// Trade execution payload.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradeExecutionData {
    /// The executed trade.
    pub trade: Trade,
    /// Related order ID.
    pub order_id: String,
    /// Related position ID if any.
    pub position_id: Option<String>,
    /// Timestamp of execution.
    pub timestamp: i64,
}

/// Margin warning payload.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarginWarningData {
    /// Portfolio ID.
    pub portfolio_id: String,
    /// Current margin level as percentage.
    pub margin_level: f64,
    /// Warning level (e.g., 50%, 25%).
    pub warning_level: f64,
    /// Positions at risk of liquidation.
    pub at_risk_positions: Vec<String>,
    /// Timestamp of warning.
    pub timestamp: i64,
}

/// Liquidation alert payload.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiquidationAlertData {
    /// Portfolio ID.
    pub portfolio_id: String,
    /// Position being liquidated.
    pub position_id: String,
    /// Symbol of the position.
    pub symbol: String,
    /// Liquidation price.
    pub liquidation_price: f64,
    /// Loss amount.
    pub loss_amount: f64,
    /// Timestamp of liquidation.
    pub timestamp: i64,
}

/// RAT (Random Auto Trader) status update payload.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RatStatusUpdateData {
    /// Portfolio ID.
    pub portfolio_id: String,
    /// Current RAT status.
    pub status: RatStatus,
    /// Runtime statistics.
    pub stats: RatStats,
    /// Current configuration.
    pub config: RatConfig,
    /// Current number of open positions.
    pub open_positions: u32,
    /// Timestamp of update.
    pub timestamp: i64,
}

// =============================================================================
// Gridline Trading WebSocket Data
// =============================================================================

/// Gridline trade placed notification data.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GridlineTradePlacedData {
    /// The placed gridline position
    pub position: GridlinePosition,
    /// Timestamp of placement
    pub timestamp: i64,
}

/// Gridline trade resolved notification data.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GridlineTradeResolvedData {
    /// The resolved gridline position
    pub position: GridlinePosition,
    /// Whether the position was won
    pub won: bool,
    /// Payout amount (if won, None if lost)
    pub payout: Option<f64>,
    /// Net P&L
    pub pnl: f64,
    /// Timestamp of resolution
    pub timestamp: i64,
}

/// Grid multiplier matrix update data.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GridMultiplierUpdateData {
    /// Trading symbol
    pub symbol: String,
    /// Multiplier matrix [row][col]
    pub multipliers: Vec<Vec<f64>>,
    /// Current grid configuration
    pub config: GridConfig,
    /// Current price of the asset
    pub current_price: f64,
    /// Timestamp of update
    pub timestamp: i64,
}

/// Grid column expired data with batch resolution results.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GridColumnExpiredData {
    /// Trading symbol
    pub symbol: String,
    /// Column index that expired
    pub col_index: i32,
    /// Column end timestamp
    pub time_end: i64,
    /// All resolution results for positions in this column
    pub results: Vec<GridlineTradeResolvedData>,
    /// Timestamp of expiration
    pub timestamp: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PeerConnectionStatus;

    // =========================================================================
    // ClientMessage Tests
    // =========================================================================

    #[test]
    fn test_client_message_subscribe_deserialization() {
        let json = r#"{"type":"subscribe","assets":["BTC","ETH"]}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();

        if let ClientMessage::Subscribe { assets } = msg {
            assert_eq!(assets, vec!["BTC", "ETH"]);
        } else {
            panic!("Expected Subscribe message");
        }
    }

    #[test]
    fn test_client_message_unsubscribe_deserialization() {
        let json = r#"{"type":"unsubscribe","assets":["BTC"]}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();

        if let ClientMessage::Unsubscribe { assets } = msg {
            assert_eq!(assets, vec!["BTC"]);
        } else {
            panic!("Expected Unsubscribe message");
        }
    }

    #[test]
    fn test_client_message_set_throttle_deserialization() {
        let json = r#"{"type":"set_throttle","throttle_ms":500}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();

        if let ClientMessage::SetThrottle { throttle_ms } = msg {
            assert_eq!(throttle_ms, 500);
        } else {
            panic!("Expected SetThrottle message");
        }
    }

    #[test]
    fn test_client_message_subscribe_peers_deserialization() {
        let json = r#"{"type":"subscribe_peers"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, ClientMessage::SubscribePeers));
    }

    #[test]
    fn test_client_message_ping_deserialization() {
        let json = r#"{"type":"ping","from_id":"server1","from_region":"US East","timestamp":1704067200000}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();

        if let ClientMessage::Ping {
            from_id,
            from_region,
            timestamp,
        } = msg
        {
            assert_eq!(from_id, "server1");
            assert_eq!(from_region, "US East");
            assert_eq!(timestamp, 1704067200000);
        } else {
            panic!("Expected Ping message");
        }
    }

    #[test]
    fn test_client_message_auth_deserialization() {
        let json = r#"{"type":"auth","id":"server1","region":"US East","timestamp":1704067200000,"signature":"abc123"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();

        if let ClientMessage::Auth {
            id,
            region,
            timestamp,
            signature,
        } = msg
        {
            assert_eq!(id, "server1");
            assert_eq!(region, "US East");
            assert_eq!(signature, "abc123");
            assert_eq!(timestamp, 1704067200000);
        } else {
            panic!("Expected Auth message");
        }
    }

    // =========================================================================
    // ServerMessage Tests
    // =========================================================================

    #[test]
    fn test_server_message_subscribed_serialization() {
        let msg = ServerMessage::Subscribed {
            assets: vec!["BTC".to_string(), "ETH".to_string()],
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"subscribed\""));
        assert!(json.contains("\"assets\":[\"BTC\",\"ETH\"]"));
    }

    #[test]
    fn test_server_message_throttle_set_serialization() {
        let msg = ServerMessage::ThrottleSet { throttle_ms: 100 };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"throttle_set\""));
        assert!(json.contains("\"throttle_ms\":100"));
    }

    #[test]
    fn test_server_message_error_serialization() {
        let msg = ServerMessage::Error {
            error: "Invalid asset".to_string(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"error\""));
        assert!(json.contains("\"error\":\"Invalid asset\""));
    }

    #[test]
    fn test_server_message_auth_response_serialization() {
        let msg = ServerMessage::AuthResponse {
            success: true,
            error: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"auth_response\""));
        assert!(json.contains("\"success\":true"));
    }

    #[test]
    fn test_server_message_auth_response_with_error() {
        let msg = ServerMessage::AuthResponse {
            success: false,
            error: Some("Invalid signature".to_string()),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("\"error\":\"Invalid signature\""));
    }

    // =========================================================================
    // SignalUpdateData Tests
    // =========================================================================

    #[test]
    fn test_signal_update_data_creation() {
        let data = SignalUpdateData {
            symbol: "BTC".to_string(),
            composite_score: 75,
            direction: SignalDirection::StrongBuy,
            trend_score: 80,
            momentum_score: 70,
            volatility_score: 60,
            volume_score: 90,
            timestamp: 1704067200000,
        };

        assert_eq!(data.symbol, "BTC");
        assert_eq!(data.composite_score, 75);
        assert_eq!(data.direction, SignalDirection::StrongBuy);
    }

    #[test]
    fn test_signal_update_data_serialization() {
        let data = SignalUpdateData {
            symbol: "ETH".to_string(),
            composite_score: -50,
            direction: SignalDirection::Sell,
            trend_score: -60,
            momentum_score: -40,
            volatility_score: 30,
            volume_score: -50,
            timestamp: 1704067200000,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"compositeScore\":-50"));
        assert!(json.contains("\"direction\":\"sell\""));
    }

    // =========================================================================
    // SeedingProgressData Tests
    // =========================================================================

    #[test]
    fn test_seeding_progress_in_progress() {
        let data = SeedingProgressData {
            symbol: "BTC".to_string(),
            status: "in_progress".to_string(),
            progress: 50,
            points: None,
            message: Some("Fetching data...".to_string()),
        };

        assert_eq!(data.progress, 50);
        assert!(data.points.is_none());
    }

    #[test]
    fn test_seeding_progress_complete() {
        let data = SeedingProgressData {
            symbol: "BTC".to_string(),
            status: "complete".to_string(),
            progress: 100,
            points: Some(8760),
            message: None,
        };

        assert_eq!(data.status, "complete");
        assert_eq!(data.points, Some(8760));
    }

    #[test]
    fn test_seeding_progress_serialization() {
        let data = SeedingProgressData {
            symbol: "ETH".to_string(),
            status: "complete".to_string(),
            progress: 100,
            points: Some(1000),
            message: None,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"symbol\":\"ETH\""));
        assert!(json.contains("\"progress\":100"));
        assert!(json.contains("\"points\":1000"));
        assert!(!json.contains("message")); // None should be omitted
    }

    // =========================================================================
    // PriceUpdateData Tests
    // =========================================================================

    #[test]
    fn test_price_update_data_creation() {
        let data = PriceUpdateData {
            id: "btc".to_string(),
            symbol: "BTC".to_string(),
            price: 50000.0,
            previous_price: Some(49000.0),
            change_24h: Some(2.04),
            volume_24h: Some(30_000_000_000.0),
            trade_direction: Some(TradeDirection::Up),
            source: PriceSource::Coinbase,
            sources: vec![PriceSource::Coinbase, PriceSource::Binance],
            timestamp: 1704067200000,
            asset_type: Some("crypto".to_string()),
        };

        assert_eq!(data.price, 50000.0);
        assert_eq!(data.sources.len(), 2);
    }

    #[test]
    fn test_price_update_data_from_aggregated_price() {
        let aggregated = AggregatedPrice {
            id: "btc".to_string(),
            symbol: "BTC".to_string(),
            price: 50000.0,
            previous_price: Some(49000.0),
            change_24h: Some(2.0),
            volume_24h: Some(30_000_000_000.0),
            trade_direction: Some(TradeDirection::Up),
            source: PriceSource::Coinbase,
            sources: vec![PriceSource::Coinbase],
            timestamp: 1704067200000,
        };

        let update_data: PriceUpdateData = aggregated.into();

        assert_eq!(update_data.id, "btc");
        assert_eq!(update_data.price, 50000.0);
        assert_eq!(update_data.asset_type, Some("crypto".to_string()));
    }

    #[test]
    fn test_price_update_data_serialization() {
        let data = PriceUpdateData {
            id: "eth".to_string(),
            symbol: "ETH".to_string(),
            price: 3000.0,
            previous_price: None,
            change_24h: None,
            volume_24h: None,
            trade_direction: None,
            source: PriceSource::Binance,
            sources: vec![PriceSource::Binance],
            timestamp: 1704067200000,
            asset_type: None,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"symbol\":\"ETH\""));
        assert!(json.contains("\"source\":\"binance\""));
        assert!(!json.contains("previousPrice")); // None omitted
        assert!(!json.contains("tradeDirection")); // None omitted
    }

    // =========================================================================
    // MarketUpdateData Tests
    // =========================================================================

    #[test]
    fn test_market_update_data_creation() {
        let data = MarketUpdateData {
            total_market_cap: 2_500_000_000_000.0,
            total_volume_24h: 100_000_000_000.0,
            btc_dominance: 52.5,
            timestamp: 1704067200000,
        };

        assert_eq!(data.total_market_cap, 2_500_000_000_000.0);
        assert_eq!(data.btc_dominance, 52.5);
    }

    #[test]
    fn test_market_update_data_from_global_metrics() {
        let metrics = GlobalMetrics {
            total_market_cap: 2_000_000_000_000.0,
            total_volume_24h: 80_000_000_000.0,
            btc_dominance: 50.0,
            eth_dominance: 18.0,
            active_cryptocurrencies: 10000,
            active_exchanges: 500,
            market_cap_change_24h: 2.0,
            volume_change_24h: -1.0,
            defi_volume_24h: None,
            defi_market_cap: None,
            stablecoin_volume_24h: None,
            stablecoin_market_cap: None,
            last_updated: "2024-01-01".to_string(),
        };

        let update_data: MarketUpdateData = metrics.into();

        assert_eq!(update_data.total_market_cap, 2_000_000_000_000.0);
        assert_eq!(update_data.btc_dominance, 50.0);
    }

    #[test]
    fn test_market_update_data_serialization() {
        let data = MarketUpdateData {
            total_market_cap: 1_000_000_000_000.0,
            total_volume_24h: 50_000_000_000.0,
            btc_dominance: 48.0,
            timestamp: 1704067200000,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"totalMarketCap\":"));
        assert!(json.contains("\"btcDominance\":48"));
    }

    // =========================================================================
    // PeerUpdateData Tests
    // =========================================================================

    #[test]
    fn test_peer_update_data_creation() {
        let data = PeerUpdateData {
            server_id: "us-east".to_string(),
            server_region: "US East".to_string(),
            peers: vec![PeerStatus {
                id: "eu-west".to_string(),
                region: "EU West".to_string(),
                status: PeerConnectionStatus::Connected,
                latency_ms: Some(85.0),
                avg_latency_ms: Some(80.0),
                min_latency_ms: Some(70.0),
                max_latency_ms: Some(100.0),
                ping_count: 1000,
                failed_pings: 5,
                uptime_percent: 99.5,
                last_ping_at: Some(1704067200000),
                last_attempt_at: Some(1704067190000),
            }],
            timestamp: 1704067200000,
        };

        assert_eq!(data.server_id, "us-east");
        assert_eq!(data.peers.len(), 1);
        assert_eq!(data.peers[0].id, "eu-west");
    }

    #[test]
    fn test_peer_update_data_serialization() {
        let data = PeerUpdateData {
            server_id: "test".to_string(),
            server_region: "Test".to_string(),
            peers: vec![],
            timestamp: 1704067200000,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"serverId\":\"test\""));
        assert!(json.contains("\"serverRegion\":\"Test\""));
        assert!(json.contains("\"peers\":[]"));
    }

    // =========================================================================
    // Trading WebSocket Message Tests
    // =========================================================================

    #[test]
    fn test_client_subscribe_trading_deserialization() {
        let json = r#"{"type":"subscribe_trading","portfolio_id":"port-123"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();

        if let ClientMessage::SubscribeTrading { portfolio_id } = msg {
            assert_eq!(portfolio_id, "port-123");
        } else {
            panic!("Expected SubscribeTrading message");
        }
    }

    #[test]
    fn test_client_unsubscribe_trading_deserialization() {
        let json = r#"{"type":"unsubscribe_trading","portfolio_id":"port-456"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();

        if let ClientMessage::UnsubscribeTrading { portfolio_id } = msg {
            assert_eq!(portfolio_id, "port-456");
        } else {
            panic!("Expected UnsubscribeTrading message");
        }
    }

    #[test]
    fn test_server_trading_subscribed_serialization() {
        let msg = ServerMessage::TradingSubscribed {
            portfolio_id: "port-abc".to_string(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"trading_subscribed\""));
        assert!(json.contains("\"portfolio_id\":\"port-abc\""));
    }

    #[test]
    fn test_order_update_type_serialization() {
        let created = OrderUpdateType::Created;
        let filled = OrderUpdateType::Filled;
        let cancelled = OrderUpdateType::Cancelled;

        assert_eq!(serde_json::to_string(&created).unwrap(), "\"created\"");
        assert_eq!(serde_json::to_string(&filled).unwrap(), "\"filled\"");
        assert_eq!(serde_json::to_string(&cancelled).unwrap(), "\"cancelled\"");
    }

    #[test]
    fn test_position_update_type_serialization() {
        let opened = PositionUpdateType::Opened;
        let closed = PositionUpdateType::Closed;
        let liquidated = PositionUpdateType::Liquidated;

        assert_eq!(serde_json::to_string(&opened).unwrap(), "\"opened\"");
        assert_eq!(serde_json::to_string(&closed).unwrap(), "\"closed\"");
        assert_eq!(serde_json::to_string(&liquidated).unwrap(), "\"liquidated\"");
    }

    #[test]
    fn test_portfolio_update_type_serialization() {
        let balance = PortfolioUpdateType::BalanceChanged;
        let reset = PortfolioUpdateType::Reset;

        assert_eq!(serde_json::to_string(&balance).unwrap(), "\"balance_changed\"");
        assert_eq!(serde_json::to_string(&reset).unwrap(), "\"reset\"");
    }

    #[test]
    fn test_margin_warning_data_serialization() {
        let data = MarginWarningData {
            portfolio_id: "port-123".to_string(),
            margin_level: 25.5,
            warning_level: 25.0,
            at_risk_positions: vec!["pos-1".to_string(), "pos-2".to_string()],
            timestamp: 1704067200000,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"portfolioId\":\"port-123\""));
        assert!(json.contains("\"marginLevel\":25.5"));
        assert!(json.contains("\"warningLevel\":25.0"));
        assert!(json.contains("\"atRiskPositions\":[\"pos-1\",\"pos-2\"]"));
    }

    #[test]
    fn test_liquidation_alert_data_serialization() {
        let data = LiquidationAlertData {
            portfolio_id: "port-123".to_string(),
            position_id: "pos-456".to_string(),
            symbol: "BTC".to_string(),
            liquidation_price: 45000.0,
            loss_amount: 1500.0,
            timestamp: 1704067200000,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"portfolioId\":\"port-123\""));
        assert!(json.contains("\"positionId\":\"pos-456\""));
        assert!(json.contains("\"symbol\":\"BTC\""));
        assert!(json.contains("\"liquidationPrice\":45000"));
        assert!(json.contains("\"lossAmount\":1500"));
    }

    #[test]
    fn test_order_status_to_update_type() {
        assert!(matches!(
            OrderUpdateType::from(OrderStatus::Pending),
            OrderUpdateType::Created
        ));
        assert!(matches!(
            OrderUpdateType::from(OrderStatus::Filled),
            OrderUpdateType::Filled
        ));
        assert!(matches!(
            OrderUpdateType::from(OrderStatus::Cancelled),
            OrderUpdateType::Cancelled
        ));
        assert!(matches!(
            OrderUpdateType::from(OrderStatus::PartiallyFilled),
            OrderUpdateType::PartialFill
        ));
    }
}
