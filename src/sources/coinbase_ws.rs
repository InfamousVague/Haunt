use crate::services::{ChartStore, PriceCache};
use crate::types::PriceSource;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

const COINBASE_WS_URL: &str = "wss://ws-feed.exchange.coinbase.com";

/// Coinbase WebSocket subscription message.
#[derive(Debug, Serialize)]
struct SubscribeMessage {
    #[serde(rename = "type")]
    msg_type: String,
    product_ids: Vec<String>,
    channels: Vec<String>,
}

/// Coinbase ticker message.
#[derive(Debug, Deserialize)]
struct TickerMessage {
    #[serde(rename = "type")]
    msg_type: String,
    product_id: Option<String>,
    price: Option<String>,
    volume_24h: Option<String>,
}

/// Coinbase WebSocket client.
#[derive(Clone)]
pub struct CoinbaseWs {
    price_cache: Arc<PriceCache>,
    chart_store: Arc<ChartStore>,
    subscribed: Arc<RwLock<HashSet<String>>>,
    pending_subscribe: Arc<RwLock<Vec<String>>>,
    pending_unsubscribe: Arc<RwLock<Vec<String>>>,
}

impl CoinbaseWs {
    /// Create a new Coinbase WebSocket client.
    pub fn new(price_cache: Arc<PriceCache>, chart_store: Arc<ChartStore>) -> Self {
        Self {
            price_cache,
            chart_store,
            subscribed: Arc::new(RwLock::new(HashSet::new())),
            pending_subscribe: Arc::new(RwLock::new(Vec::new())),
            pending_unsubscribe: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Subscribe to symbols.
    pub async fn subscribe(&self, symbols: &[String]) {
        let product_ids: Vec<String> = symbols
            .iter()
            .map(|s| format!("{}-USD", s.to_uppercase()))
            .collect();

        let mut pending = self.pending_subscribe.write().await;
        pending.extend(product_ids);
    }

    /// Unsubscribe from symbols.
    pub async fn unsubscribe(&self, symbols: &[String]) {
        let product_ids: Vec<String> = symbols
            .iter()
            .map(|s| format!("{}-USD", s.to_uppercase()))
            .collect();

        let mut pending = self.pending_unsubscribe.write().await;
        pending.extend(product_ids);
    }

    /// Connect and start receiving price updates.
    pub async fn connect(&self) -> anyhow::Result<()> {
        loop {
            match self.run_connection().await {
                Ok(_) => {
                    warn!("Coinbase WebSocket disconnected, reconnecting...");
                    self.price_cache
                        .report_source_error(PriceSource::Coinbase, "WebSocket disconnected");
                }
                Err(e) => {
                    error!("Coinbase WebSocket error: {}, reconnecting...", e);
                    self.price_cache
                        .report_source_error(PriceSource::Coinbase, &e.to_string());
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }

    async fn run_connection(&self) -> anyhow::Result<()> {
        info!("Connecting to Coinbase WebSocket");
        let (ws_stream, _) = connect_async(COINBASE_WS_URL).await?;
        let (mut write, mut read) = ws_stream.split();
        info!("Connected to Coinbase WebSocket");

        // Subscribe to initial symbols
        let initial_symbols = vec![
            "BTC-USD",
            "ETH-USD",
            "SOL-USD",
            "XRP-USD",
            "DOGE-USD",
            "ADA-USD",
            "AVAX-USD",
            "DOT-USD",
            "LINK-USD",
            "MATIC-USD",
            "LTC-USD",
            "ATOM-USD",
            "UNI-USD",
            "XLM-USD",
            "BCH-USD",
        ];

        let subscribe_msg = SubscribeMessage {
            msg_type: "subscribe".to_string(),
            product_ids: initial_symbols.iter().map(|s| s.to_string()).collect(),
            channels: vec!["ticker".to_string()],
        };

        let msg_json = serde_json::to_string(&subscribe_msg)?;
        write.send(Message::Text(msg_json)).await?;

        {
            let mut subscribed = self.subscribed.write().await;
            subscribed.extend(initial_symbols.iter().map(|s| s.to_string()));
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
                            info!("Coinbase WebSocket closed");
                            break;
                        }
                        Some(Err(e)) => {
                            error!("Coinbase WebSocket read error: {}", e);
                            break;
                        }
                        None => {
                            break;
                        }
                        _ => {}
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    // Process pending subscriptions
                    let to_subscribe: Vec<String> = {
                        let mut pending = self.pending_subscribe.write().await;
                        pending.drain(..).collect()
                    };

                    if !to_subscribe.is_empty() {
                        let msg = SubscribeMessage {
                            msg_type: "subscribe".to_string(),
                            product_ids: to_subscribe.clone(),
                            channels: vec!["ticker".to_string()],
                        };
                        if let Ok(json) = serde_json::to_string(&msg) {
                            let _ = write.send(Message::Text(json)).await;
                            let mut subscribed = self.subscribed.write().await;
                            subscribed.extend(to_subscribe);
                        }
                    }

                    // Process pending unsubscriptions
                    let to_unsubscribe: Vec<String> = {
                        let mut pending = self.pending_unsubscribe.write().await;
                        pending.drain(..).collect()
                    };

                    if !to_unsubscribe.is_empty() {
                        let msg = SubscribeMessage {
                            msg_type: "unsubscribe".to_string(),
                            product_ids: to_unsubscribe.clone(),
                            channels: vec!["ticker".to_string()],
                        };
                        if let Ok(json) = serde_json::to_string(&msg) {
                            let _ = write.send(Message::Text(json)).await;
                            let mut subscribed = self.subscribed.write().await;
                            for id in &to_unsubscribe {
                                subscribed.remove(id);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_message(&self, text: &str) {
        let msg: TickerMessage = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(_) => return,
        };

        if msg.msg_type != "ticker" {
            return;
        }

        let product_id = match msg.product_id {
            Some(id) => id,
            None => return,
        };

        let price: f64 = match msg.price.and_then(|p| p.parse().ok()) {
            Some(p) => p,
            None => return,
        };

        let volume_24h: Option<f64> = msg.volume_24h.and_then(|v| v.parse().ok());

        // Extract symbol from product_id (e.g., "BTC-USD" -> "btc")
        let symbol = product_id
            .split('-')
            .next()
            .unwrap_or(&product_id)
            .to_lowercase();

        debug!("Coinbase price update: {} = ${}", symbol, price);

        let timestamp = chrono::Utc::now().timestamp_millis();

        self.price_cache
            .update_price(&symbol, PriceSource::Coinbase, price, volume_24h);
        self.chart_store
            .add_price(&symbol, price, volume_24h, timestamp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // SubscribeMessage Tests
    // =========================================================================

    #[test]
    fn test_subscribe_message_serialization() {
        let msg = SubscribeMessage {
            msg_type: "subscribe".to_string(),
            product_ids: vec!["BTC-USD".to_string(), "ETH-USD".to_string()],
            channels: vec!["ticker".to_string()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"subscribe\""));
        assert!(json.contains("BTC-USD"));
        assert!(json.contains("ticker"));
    }

    #[test]
    fn test_subscribe_message_unsubscribe() {
        let msg = SubscribeMessage {
            msg_type: "unsubscribe".to_string(),
            product_ids: vec!["SOL-USD".to_string()],
            channels: vec!["ticker".to_string()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"unsubscribe\""));
    }

    // =========================================================================
    // TickerMessage Tests
    // =========================================================================

    #[test]
    fn test_ticker_message_deserialization() {
        let json = r#"{
            "type": "ticker",
            "product_id": "BTC-USD",
            "price": "43500.50",
            "volume_24h": "15000.5"
        }"#;
        let ticker: TickerMessage = serde_json::from_str(json).unwrap();
        assert_eq!(ticker.msg_type, "ticker");
        assert_eq!(ticker.product_id, Some("BTC-USD".to_string()));
        assert_eq!(ticker.price, Some("43500.50".to_string()));
        assert_eq!(ticker.volume_24h, Some("15000.5".to_string()));
    }

    #[test]
    fn test_ticker_message_minimal() {
        let json = r#"{"type": "subscriptions"}"#;
        let ticker: TickerMessage = serde_json::from_str(json).unwrap();
        assert_eq!(ticker.msg_type, "subscriptions");
        assert!(ticker.product_id.is_none());
        assert!(ticker.price.is_none());
    }

    #[test]
    fn test_ticker_message_price_parsing() {
        let json = r#"{
            "type": "ticker",
            "product_id": "ETH-USD",
            "price": "2500.00"
        }"#;
        let ticker: TickerMessage = serde_json::from_str(json).unwrap();
        let price: f64 = ticker.price.unwrap().parse().unwrap();
        assert_eq!(price, 2500.0);
    }

    // =========================================================================
    // Symbol Extraction Tests
    // =========================================================================

    #[test]
    fn test_symbol_extraction_from_product_id() {
        let product_id = "BTC-USD";
        let symbol = product_id
            .split('-')
            .next()
            .unwrap_or(product_id)
            .to_lowercase();
        assert_eq!(symbol, "btc");
    }

    #[test]
    fn test_symbol_extraction_various() {
        let test_cases = vec![("ETH-USD", "eth"), ("SOL-USD", "sol"), ("DOGE-USD", "doge")];
        for (product_id, expected) in test_cases {
            let symbol = product_id
                .split('-')
                .next()
                .unwrap_or(product_id)
                .to_lowercase();
            assert_eq!(symbol, expected);
        }
    }

    // =========================================================================
    // Constants Tests
    // =========================================================================

    #[test]
    fn test_coinbase_ws_url() {
        assert!(COINBASE_WS_URL.starts_with("wss://"));
        assert!(COINBASE_WS_URL.contains("coinbase"));
    }
}
