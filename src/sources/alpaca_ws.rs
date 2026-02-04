//! Alpaca WebSocket client for real-time stock/ETF data.
//!
//! Provides real-time IEX exchange data via WebSocket.
//! Requires Alpaca API key and secret (free with paper trading account).
//! Sign up at: https://alpaca.markets/

// Some structs are used only for deserialization/tests
#![allow(dead_code)]

use crate::services::{ChartStore, PriceCache};
use crate::types::PriceSource;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

// IEX feed (free) - use SIP for paid real-time data
const ALPACA_WS_URL: &str = "wss://stream.data.alpaca.markets/v2/iex";

/// Stocks to subscribe to (limited for free tier - max ~10 symbols).
pub const STOCK_SYMBOLS: &[&str] = &[
    "AAPL", "MSFT", "GOOGL", "AMZN", "NVDA", "TSLA", "META", "SPY", "QQQ",
];

/// Alpaca authentication message.
#[derive(Debug, Serialize)]
struct AuthMessage {
    action: String,
    key: String,
    secret: String,
}

/// Alpaca subscribe message.
#[derive(Debug, Serialize)]
struct SubscribeMessage {
    action: String,
    trades: Vec<String>,
    quotes: Vec<String>,
}

/// Alpaca trade data.
#[derive(Debug, Deserialize)]
struct AlpacaTrade {
    /// Symbol
    #[serde(rename = "S")]
    symbol: String,
    /// Price
    p: f64,
    /// Size/volume
    s: u64,
    /// Timestamp
    t: String,
    /// Trade ID
    #[serde(default)]
    i: Option<u64>,
    /// Exchange
    #[serde(default)]
    x: Option<String>,
}

/// Alpaca quote data.
#[derive(Debug, Deserialize)]
struct AlpacaQuote {
    /// Symbol
    #[serde(rename = "S")]
    symbol: String,
    /// Bid price
    bp: f64,
    /// Ask price
    ap: f64,
    /// Bid size
    bs: u64,
    /// Ask size
    #[serde(rename = "as")]
    ask_size: u64,
    /// Timestamp
    t: String,
}

/// Alpaca WebSocket message wrapper.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum AlpacaMessage {
    Control(Vec<ControlMessage>),
    Trade(Vec<TradeMessage>),
    Quote(Vec<QuoteMessage>),
}

#[derive(Debug, Deserialize)]
struct ControlMessage {
    #[serde(rename = "T")]
    msg_type: String,
    #[serde(default)]
    msg: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TradeMessage {
    #[serde(rename = "T")]
    msg_type: String,
    #[serde(rename = "S")]
    symbol: String,
    p: f64,
    s: u64,
    t: String,
}

#[derive(Debug, Deserialize)]
struct QuoteMessage {
    #[serde(rename = "T")]
    msg_type: String,
    #[serde(rename = "S")]
    symbol: String,
    bp: f64,
    ap: f64,
    bs: u64,
    #[serde(rename = "as")]
    ask_size: u64,
    t: String,
}

/// Alpaca WebSocket client for real-time stock data.
#[derive(Clone)]
pub struct AlpacaWs {
    api_key: String,
    api_secret: String,
    price_cache: Arc<PriceCache>,
    chart_store: Arc<ChartStore>,
    subscribed: Arc<RwLock<HashSet<String>>>,
}

impl AlpacaWs {
    /// Create a new Alpaca WebSocket client.
    pub fn new(
        api_key: String,
        api_secret: String,
        price_cache: Arc<PriceCache>,
        chart_store: Arc<ChartStore>,
    ) -> Self {
        Self {
            api_key,
            api_secret,
            price_cache,
            chart_store,
            subscribed: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Connect and start receiving real-time stock data.
    pub async fn connect(&self) -> anyhow::Result<()> {
        loop {
            match self.run_connection().await {
                Ok(_) => {
                    warn!("Alpaca WebSocket disconnected, reconnecting...");
                }
                Err(e) => {
                    error!("Alpaca WebSocket error: {}, reconnecting...", e);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }

    async fn run_connection(&self) -> anyhow::Result<()> {
        info!("Connecting to Alpaca WebSocket (IEX)");

        let (ws_stream, _) = connect_async(ALPACA_WS_URL).await?;
        let (mut write, mut read) = ws_stream.split();
        info!("Connected to Alpaca WebSocket");

        // Wait for welcome message
        if let Some(Ok(Message::Text(text))) = read.next().await {
            debug!("Alpaca welcome: {}", text);
        }

        // Authenticate
        let auth_msg = AuthMessage {
            action: "auth".to_string(),
            key: self.api_key.clone(),
            secret: self.api_secret.clone(),
        };
        let auth_json = serde_json::to_string(&auth_msg)?;
        write.send(Message::Text(auth_json)).await?;

        // Wait for auth response
        if let Some(Ok(Message::Text(text))) = read.next().await {
            debug!("Alpaca auth response: {}", text);
            if text.contains("\"error\"") || text.contains("auth failed") {
                return Err(anyhow::anyhow!("Alpaca authentication failed: {}", text));
            }
        }

        info!("Alpaca WebSocket authenticated");

        // Subscribe to trades and quotes
        let subscribe_msg = SubscribeMessage {
            action: "subscribe".to_string(),
            trades: STOCK_SYMBOLS.iter().map(|s| s.to_string()).collect(),
            quotes: STOCK_SYMBOLS.iter().map(|s| s.to_string()).collect(),
        };
        let sub_json = serde_json::to_string(&subscribe_msg)?;
        write.send(Message::Text(sub_json)).await?;

        info!(
            "Subscribed to {} stocks via Alpaca WebSocket",
            STOCK_SYMBOLS.len()
        );

        // Store subscribed symbols
        {
            let mut subscribed = self.subscribed.write().await;
            subscribed.extend(STOCK_SYMBOLS.iter().map(|s| s.to_string()));
        }

        loop {
            tokio::select! {
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            self.handle_message(&text).await;
                        }
                        Some(Ok(Message::Ping(data))) => {
                            let _ = write.send(Message::Pong(data)).await;
                        }
                        Some(Ok(Message::Close(_))) => {
                            info!("Alpaca WebSocket closed");
                            break;
                        }
                        Some(Err(e)) => {
                            error!("Alpaca WebSocket read error: {}", e);
                            break;
                        }
                        None => {
                            break;
                        }
                        _ => {}
                    }
                }
                // Send ping every 30 seconds to keep connection alive
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                    let _ = write.send(Message::Ping(vec![])).await;
                }
            }
        }

        Ok(())
    }

    async fn handle_message(&self, text: &str) {
        // Alpaca sends arrays of messages
        let messages: Vec<serde_json::Value> = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(_) => return,
        };

        for msg in messages {
            let msg_type = msg.get("T").and_then(|t| t.as_str()).unwrap_or("");

            match msg_type {
                "t" => {
                    // Trade message
                    if let (Some(symbol), Some(price), Some(size)) = (
                        msg.get("S").and_then(|s| s.as_str()),
                        msg.get("p").and_then(|p| p.as_f64()),
                        msg.get("s").and_then(|s| s.as_u64()),
                    ) {
                        let symbol_lower = symbol.to_lowercase();
                        let timestamp = chrono::Utc::now().timestamp_millis();

                        debug!("Alpaca trade: {} = ${:.2} (size: {})", symbol, price, size);

                        // Update price cache (use Finnhub source since we don't have Alpaca variant)
                        self.price_cache.update_price(
                            &symbol_lower,
                            PriceSource::Alpaca,
                            price,
                            None,
                        );

                        // Add to chart store
                        self.chart_store.add_price(
                            &symbol_lower,
                            price,
                            Some(size as f64),
                            timestamp,
                        );
                    }
                }
                "q" => {
                    // Quote message - use mid price
                    if let (Some(symbol), Some(bid), Some(ask)) = (
                        msg.get("S").and_then(|s| s.as_str()),
                        msg.get("bp").and_then(|p| p.as_f64()),
                        msg.get("ap").and_then(|p| p.as_f64()),
                    ) {
                        let mid_price = (bid + ask) / 2.0;
                        let symbol_lower = symbol.to_lowercase();
                        let timestamp = chrono::Utc::now().timestamp_millis();

                        debug!(
                            "Alpaca quote: {} bid=${:.2} ask=${:.2} mid=${:.2}",
                            symbol, bid, ask, mid_price
                        );

                        // Update with mid price from quotes
                        self.price_cache.update_price(
                            &symbol_lower,
                            PriceSource::Alpaca,
                            mid_price,
                            None,
                        );
                        self.chart_store
                            .add_price(&symbol_lower, mid_price, None, timestamp);
                    }
                }
                "success" | "subscription" => {
                    debug!("Alpaca control message: {}", text);
                }
                "error" => {
                    error!("Alpaca error: {}", text);
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // STOCK_SYMBOLS Tests
    // =========================================================================

    #[test]
    fn test_stock_symbols_contains_major_stocks() {
        assert!(STOCK_SYMBOLS.contains(&"AAPL"));
        assert!(STOCK_SYMBOLS.contains(&"MSFT"));
        assert!(STOCK_SYMBOLS.contains(&"GOOGL"));
        assert!(STOCK_SYMBOLS.contains(&"NVDA"));
    }

    #[test]
    fn test_stock_symbols_contains_etfs() {
        assert!(STOCK_SYMBOLS.contains(&"SPY"));
        assert!(STOCK_SYMBOLS.contains(&"QQQ"));
    }

    #[test]
    fn test_stock_symbols_count() {
        assert!(STOCK_SYMBOLS.len() >= 9);
    }

    // =========================================================================
    // AuthMessage Tests
    // =========================================================================

    #[test]
    fn test_auth_message_serialization() {
        let msg = AuthMessage {
            action: "auth".to_string(),
            key: "test_key".to_string(),
            secret: "test_secret".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"action\":\"auth\""));
        assert!(json.contains("\"key\":\"test_key\""));
        assert!(json.contains("\"secret\":\"test_secret\""));
    }

    // =========================================================================
    // SubscribeMessage Tests
    // =========================================================================

    #[test]
    fn test_subscribe_message_serialization() {
        let msg = SubscribeMessage {
            action: "subscribe".to_string(),
            trades: vec!["AAPL".to_string(), "MSFT".to_string()],
            quotes: vec!["AAPL".to_string()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"action\":\"subscribe\""));
        assert!(json.contains("AAPL"));
        assert!(json.contains("MSFT"));
    }

    // =========================================================================
    // AlpacaTrade Tests
    // =========================================================================

    #[test]
    fn test_alpaca_trade_deserialization() {
        let json = r#"{
            "S": "AAPL",
            "p": 153.25,
            "s": 100,
            "t": "2024-01-15T10:30:00Z"
        }"#;
        let trade: AlpacaTrade = serde_json::from_str(json).unwrap();
        assert_eq!(trade.symbol, "AAPL");
        assert_eq!(trade.p, 153.25);
        assert_eq!(trade.s, 100);
    }

    #[test]
    fn test_alpaca_trade_with_optional_fields() {
        let json = r#"{
            "S": "MSFT",
            "p": 380.50,
            "s": 50,
            "t": "2024-01-15T10:30:00Z",
            "i": 12345,
            "x": "V"
        }"#;
        let trade: AlpacaTrade = serde_json::from_str(json).unwrap();
        assert_eq!(trade.i, Some(12345));
        assert_eq!(trade.x, Some("V".to_string()));
    }

    // =========================================================================
    // AlpacaQuote Tests
    // =========================================================================

    #[test]
    fn test_alpaca_quote_deserialization() {
        let json = r#"{
            "S": "AAPL",
            "bp": 153.00,
            "ap": 153.50,
            "bs": 100,
            "as": 200,
            "t": "2024-01-15T10:30:00Z"
        }"#;
        let quote: AlpacaQuote = serde_json::from_str(json).unwrap();
        assert_eq!(quote.symbol, "AAPL");
        assert_eq!(quote.bp, 153.00);
        assert_eq!(quote.ap, 153.50);
        assert_eq!(quote.bs, 100);
        assert_eq!(quote.ask_size, 200);
    }

    #[test]
    fn test_alpaca_quote_mid_price() {
        let json = r#"{
            "S": "TSLA",
            "bp": 200.00,
            "ap": 200.50,
            "bs": 100,
            "as": 150,
            "t": "2024-01-15T10:30:00Z"
        }"#;
        let quote: AlpacaQuote = serde_json::from_str(json).unwrap();
        let mid_price = (quote.bp + quote.ap) / 2.0;
        assert_eq!(mid_price, 200.25);
    }

    // =========================================================================
    // TradeMessage Tests
    // =========================================================================

    #[test]
    fn test_trade_message_deserialization() {
        let json = r#"{
            "T": "t",
            "S": "NVDA",
            "p": 500.00,
            "s": 25,
            "t": "2024-01-15T10:30:00Z"
        }"#;
        let msg: TradeMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.msg_type, "t");
        assert_eq!(msg.symbol, "NVDA");
        assert_eq!(msg.p, 500.00);
    }

    // =========================================================================
    // QuoteMessage Tests
    // =========================================================================

    #[test]
    fn test_quote_message_deserialization() {
        let json = r#"{
            "T": "q",
            "S": "META",
            "bp": 350.00,
            "ap": 350.50,
            "bs": 100,
            "as": 100,
            "t": "2024-01-15T10:30:00Z"
        }"#;
        let msg: QuoteMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.msg_type, "q");
        assert_eq!(msg.symbol, "META");
    }

    // =========================================================================
    // ControlMessage Tests
    // =========================================================================

    #[test]
    fn test_control_message_deserialization() {
        let json = r#"{"T": "success", "msg": "authenticated"}"#;
        let msg: ControlMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.msg_type, "success");
        assert_eq!(msg.msg, Some("authenticated".to_string()));
    }

    // =========================================================================
    // Constants Tests
    // =========================================================================

    #[test]
    fn test_alpaca_ws_url() {
        assert!(ALPACA_WS_URL.starts_with("wss://"));
        assert!(ALPACA_WS_URL.contains("alpaca"));
    }
}
