//! Tiingo WebSocket client for real-time stock data.
//!
//! Provides real-time IEX exchange data via WebSocket.
//! Free tier available at: https://www.tiingo.com/

use crate::services::{ChartStore, PriceCache};
use crate::types::PriceSource;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

const TIINGO_WS_URL: &str = "wss://api.tiingo.com/iex";

/// Stocks to subscribe to.
pub const STOCK_SYMBOLS: &[&str] = &[
    "AAPL", "MSFT", "GOOGL", "AMZN", "NVDA", "TSLA", "META", "JPM", "V",
    "JNJ", "UNH", "HD", "PG", "MA", "DIS", "ADBE", "CRM", "NFLX", "PYPL",
    "SPY", "QQQ", "VOO", "IWM", "DIA",
];

/// Tiingo subscribe message.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SubscribeMessage {
    event_name: String,
    authorization: String,
    event_data: EventData,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EventData {
    /// Threshold level: 0=trades only, 1=top of book, 2=all data
    threshold_level: u8,
    /// Tickers to subscribe to
    tickers: Vec<String>,
}

/// Tiingo WebSocket message types.
#[derive(Debug, Deserialize)]
struct TiingoMessage {
    #[serde(rename = "messageType")]
    message_type: String,
    #[serde(default)]
    data: Option<serde_json::Value>,
    #[serde(default)]
    response: Option<TiingoResponse>,
}

#[derive(Debug, Deserialize)]
struct TiingoResponse {
    code: Option<i32>,
    message: Option<String>,
}

/// Tiingo IEX data format (array-based for efficiency).
/// Format: [messageType, date, timestamp, ticker, bidSize, bidPrice, midPrice, askPrice, askSize, lastPrice, lastSize, halted]
#[derive(Debug, Deserialize)]
struct TiingoIexData(
    String,           // 0: messageType ("T" for trade, "Q" for quote)
    String,           // 1: date
    String,           // 2: timestamp
    String,           // 3: ticker
    Option<f64>,      // 4: bidSize
    Option<f64>,      // 5: bidPrice
    Option<f64>,      // 6: midPrice
    Option<f64>,      // 7: askPrice
    Option<f64>,      // 8: askSize
    Option<f64>,      // 9: lastPrice (trade price)
    Option<f64>,      // 10: lastSize (trade size)
    Option<i32>,      // 11: halted
);

/// Tiingo WebSocket client for real-time stock data.
#[derive(Clone)]
pub struct TiingoWs {
    api_key: String,
    price_cache: Arc<PriceCache>,
    chart_store: Arc<ChartStore>,
    subscribed: Arc<RwLock<HashSet<String>>>,
}

impl TiingoWs {
    /// Create a new Tiingo WebSocket client.
    pub fn new(api_key: String, price_cache: Arc<PriceCache>, chart_store: Arc<ChartStore>) -> Self {
        Self {
            api_key,
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
                    warn!("Tiingo WebSocket disconnected, reconnecting...");
                }
                Err(e) => {
                    error!("Tiingo WebSocket error: {}, reconnecting...", e);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }

    async fn run_connection(&self) -> anyhow::Result<()> {
        info!("Connecting to Tiingo WebSocket");

        let (ws_stream, _) = connect_async(TIINGO_WS_URL).await?;
        let (mut write, mut read) = ws_stream.split();
        info!("Connected to Tiingo WebSocket");

        // Subscribe with authentication
        let subscribe_msg = SubscribeMessage {
            event_name: "subscribe".to_string(),
            authorization: self.api_key.clone(),
            event_data: EventData {
                threshold_level: 0, // 0 = trades only (most efficient)
                tickers: STOCK_SYMBOLS.iter().map(|s| s.to_string()).collect(),
            },
        };

        let sub_json = serde_json::to_string(&subscribe_msg)?;
        write.send(Message::Text(sub_json)).await?;

        info!("Subscribed to {} stocks via Tiingo WebSocket", STOCK_SYMBOLS.len());

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
                            info!("Tiingo WebSocket closed");
                            break;
                        }
                        Some(Err(e)) => {
                            error!("Tiingo WebSocket read error: {}", e);
                            break;
                        }
                        None => {
                            break;
                        }
                        _ => {}
                    }
                }
                // Send heartbeat every 20 seconds
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(20)) => {
                    // Tiingo expects a heartbeat subscription to keep alive
                    let heartbeat = serde_json::json!({
                        "eventName": "heartbeat"
                    });
                    let _ = write.send(Message::Text(heartbeat.to_string())).await;
                }
            }
        }

        Ok(())
    }

    async fn handle_message(&self, text: &str) {
        let msg: TiingoMessage = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(_) => return,
        };

        match msg.message_type.as_str() {
            "A" => {
                // Array data message (trade or quote)
                if let Some(data) = msg.data {
                    if let Ok(arr) = serde_json::from_value::<Vec<serde_json::Value>>(data) {
                        self.handle_iex_data(&arr).await;
                    }
                }
            }
            "I" => {
                // Info message
                debug!("Tiingo info: {}", text);
            }
            "H" => {
                // Heartbeat
                debug!("Tiingo heartbeat");
            }
            "E" => {
                // Error
                error!("Tiingo error: {}", text);
            }
            _ => {}
        }
    }

    async fn handle_iex_data(&self, arr: &[serde_json::Value]) {
        // Format: [messageType, date, timestamp, ticker, ...]
        if arr.len() < 10 {
            return;
        }

        let msg_type = arr[0].as_str().unwrap_or("");
        let ticker = arr[3].as_str().unwrap_or("");

        if ticker.is_empty() {
            return;
        }

        let symbol = ticker.to_lowercase();
        let timestamp = chrono::Utc::now().timestamp_millis();

        match msg_type {
            "T" => {
                // Trade message
                // lastPrice is at index 9, lastSize at index 10
                if let Some(price) = arr[9].as_f64() {
                    let size = arr[10].as_f64();

                    debug!("Tiingo trade: {} = ${:.2}", ticker, price);

                    self.price_cache.update_price(&symbol, PriceSource::Tiingo, price, None);
                    self.chart_store.add_price(&symbol, price, size, timestamp);
                }
            }
            "Q" => {
                // Quote message - use mid price
                // bidPrice at 5, midPrice at 6, askPrice at 7
                if let Some(mid_price) = arr[6].as_f64() {
                    debug!("Tiingo quote: {} mid=${:.2}", ticker, mid_price);

                    self.price_cache.update_price(&symbol, PriceSource::Tiingo, mid_price, None);
                    self.chart_store.add_price(&symbol, mid_price, None, timestamp);
                }
            }
            _ => {}
        }
    }
}
