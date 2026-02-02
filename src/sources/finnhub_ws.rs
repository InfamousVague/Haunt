//! Finnhub WebSocket client for real-time stock/ETF trades.
//!
//! Provides instant trade updates for US stocks and ETFs via WebSocket.
//! Free tier supports real-time US stock trades.

use crate::services::{ChartStore, PriceCache};
use crate::types::PriceSource;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

/// Top stocks to subscribe to.
pub const STOCK_SYMBOLS: &[&str] = &[
    "AAPL", "MSFT", "GOOGL", "AMZN", "NVDA", "TSLA", "META", "BRK.B", "JPM", "V",
    "JNJ", "UNH", "HD", "PG", "MA", "DIS", "ADBE", "CRM", "NFLX", "PYPL",
];

/// Top ETFs to subscribe to.
pub const ETF_SYMBOLS: &[&str] = &[
    "SPY", "QQQ", "VOO", "IWM", "DIA", "VTI", "ARKK", "XLF", "XLE", "GLD",
];

/// Finnhub WebSocket subscribe message.
#[derive(Debug, Serialize)]
struct SubscribeMessage {
    #[serde(rename = "type")]
    msg_type: String,
    symbol: String,
}

/// Finnhub trade data.
#[derive(Debug, Deserialize)]
struct TradeData {
    /// Symbol
    s: String,
    /// Price
    p: f64,
    /// Volume
    v: f64,
    /// Timestamp (milliseconds)
    t: i64,
    /// Trade conditions
    #[serde(default)]
    c: Option<Vec<String>>,
}

/// Finnhub WebSocket message.
#[derive(Debug, Deserialize)]
struct FinnhubMessage {
    #[serde(rename = "type")]
    msg_type: String,
    #[serde(default)]
    data: Vec<TradeData>,
}

/// Finnhub WebSocket client for real-time stock data.
#[derive(Clone)]
pub struct FinnhubWs {
    api_key: String,
    price_cache: Arc<PriceCache>,
    chart_store: Arc<ChartStore>,
    subscribed: Arc<RwLock<HashSet<String>>>,
}

impl FinnhubWs {
    /// Create a new Finnhub WebSocket client.
    pub fn new(api_key: String, price_cache: Arc<PriceCache>, chart_store: Arc<ChartStore>) -> Self {
        Self {
            api_key,
            price_cache,
            chart_store,
            subscribed: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Connect and start receiving real-time stock trades.
    pub async fn connect(&self) -> anyhow::Result<()> {
        loop {
            match self.run_connection().await {
                Ok(_) => {
                    warn!("Finnhub WebSocket disconnected, reconnecting...");
                    self.price_cache.report_source_error(PriceSource::Finnhub, "WebSocket disconnected");
                }
                Err(e) => {
                    error!("Finnhub WebSocket error: {}, reconnecting...", e);
                    self.price_cache.report_source_error(PriceSource::Finnhub, &e.to_string());
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }

    async fn run_connection(&self) -> anyhow::Result<()> {
        let url = format!("wss://ws.finnhub.io?token={}", self.api_key);
        info!("Connecting to Finnhub WebSocket");

        let (ws_stream, _) = connect_async(&url).await?;
        let (mut write, mut read) = ws_stream.split();
        info!("Connected to Finnhub WebSocket");

        // Subscribe to stocks
        for symbol in STOCK_SYMBOLS {
            let msg = SubscribeMessage {
                msg_type: "subscribe".to_string(),
                symbol: symbol.to_string(),
            };
            if let Ok(json) = serde_json::to_string(&msg) {
                write.send(Message::Text(json)).await?;
            }
        }

        // Subscribe to ETFs
        for symbol in ETF_SYMBOLS {
            let msg = SubscribeMessage {
                msg_type: "subscribe".to_string(),
                symbol: symbol.to_string(),
            };
            if let Ok(json) = serde_json::to_string(&msg) {
                write.send(Message::Text(json)).await?;
            }
        }

        info!("Subscribed to {} stocks and {} ETFs via Finnhub WebSocket",
              STOCK_SYMBOLS.len(), ETF_SYMBOLS.len());

        // Store subscribed symbols
        {
            let mut subscribed = self.subscribed.write().await;
            subscribed.extend(STOCK_SYMBOLS.iter().map(|s| s.to_string()));
            subscribed.extend(ETF_SYMBOLS.iter().map(|s| s.to_string()));
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
                            info!("Finnhub WebSocket closed");
                            break;
                        }
                        Some(Err(e)) => {
                            error!("Finnhub WebSocket read error: {}", e);
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
        let msg: FinnhubMessage = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(_) => return,
        };

        if msg.msg_type != "trade" {
            return;
        }

        for trade in msg.data {
            let symbol = trade.s.to_lowercase();
            let price = trade.p;
            let timestamp = trade.t;

            debug!("Finnhub trade: {} = ${:.2} (vol: {})", symbol, price, trade.v);

            // Update price cache
            self.price_cache.update_price(&symbol, PriceSource::Finnhub, price, None);

            // Add to chart store
            self.chart_store.add_price(&symbol, price, Some(trade.v), timestamp);
        }
    }
}
