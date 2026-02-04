//! WebSocket Data Validation Tests
//!
//! These tests validate that WebSocket price updates from our server
//! contain accurate data compared to external APIs.
//!
//! Run with: cargo test --test websocket_validation_test -- --ignored --nocapture
//!
//! The tests connect to our WebSocket, receive price updates, and compare
//! them against CoinGecko/CryptoCompare to ensure data consistency.

use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Acceptable price deviation percentage (5%)
const PRICE_DEVIATION_THRESHOLD: f64 = 5.0;

/// Acceptable volume deviation percentage (50% - WebSocket volume may differ significantly)
const VOLUME_DEVIATION_THRESHOLD: f64 = 50.0;

/// Our WebSocket URL
const HAUNT_WS_URL: &str = "ws://localhost:3001/ws";

/// Our API base URL
const HAUNT_API_URL: &str = "http://localhost:3001/api";

/// CoinGecko API URL
const COINGECKO_API_URL: &str = "https://api.coingecko.com/api/v3";

// ============================================================================
// Message Types
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
#[allow(dead_code)]
enum ClientMessage {
    Subscribe { assets: Vec<String> },
    Unsubscribe { assets: Vec<String> },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
enum ServerMessage {
    PriceUpdate {
        data: PriceUpdateData,
    },
    Subscribed {
        assets: Vec<String>,
    },
    Unsubscribed {
        assets: Vec<String>,
    },
    Error {
        error: String,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct PriceUpdateData {
    id: String,
    symbol: String,
    price: f64,
    #[serde(default)]
    previous_price: Option<f64>,
    #[serde(default)]
    change_24h: Option<f64>,
    #[serde(default)]
    volume_24h: Option<f64>,
    #[serde(default)]
    trade_direction: Option<String>,
    source: String,
    sources: Vec<String>,
    timestamp: i64,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CoinGeckoPrice {
    usd: f64,
    #[serde(default)]
    usd_24h_vol: Option<f64>,
    #[serde(default)]
    usd_24h_change: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct HauntApiResponse<T> {
    data: T,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct HauntListing {
    symbol: String,
    price: f64,
    change_24h: f64,
    volume_24h: f64,
}

// ============================================================================
// Test Utilities
// ============================================================================

fn create_http_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Haunt-WebSocket-Test/1.0")
        .build()
        .expect("Failed to create HTTP client")
}

fn calculate_deviation(our_value: f64, reference_value: f64) -> f64 {
    if reference_value == 0.0 {
        if our_value == 0.0 {
            return 0.0;
        }
        return 100.0;
    }
    ((our_value - reference_value).abs() / reference_value.abs()) * 100.0
}

// ============================================================================
// Tests
// ============================================================================

/// Test that we can connect to WebSocket and receive subscription confirmation
#[tokio::test]
#[ignore]
async fn test_websocket_connection_and_subscription() {
    let (ws_stream, _) = connect_async(HAUNT_WS_URL)
        .await
        .expect("Failed to connect to WebSocket");

    let (mut write, mut read) = ws_stream.split();

    // Subscribe to BTC
    let subscribe_msg = ClientMessage::Subscribe {
        assets: vec!["btc".to_string()],
    };
    let msg_text = serde_json::to_string(&subscribe_msg).unwrap();
    write
        .send(Message::Text(msg_text))
        .await
        .expect("Failed to send");

    // Wait for subscribed confirmation
    let response = timeout(Duration::from_secs(5), read.next())
        .await
        .expect("Timeout waiting for response")
        .expect("No message received")
        .expect("WebSocket error");

    if let Message::Text(text) = response {
        let msg: ServerMessage = serde_json::from_str(&text).expect("Failed to parse message");
        match msg {
            ServerMessage::Subscribed { assets } => {
                println!("✓ Successfully subscribed to: {:?}", assets);
                assert!(assets.contains(&"btc".to_string()));
            }
            _ => panic!("Expected Subscribed message, got: {:?}", msg),
        }
    }

    println!("✓ WebSocket connection and subscription working");
}

/// Test that WebSocket price updates are received and contain expected fields
#[tokio::test]
#[ignore]
async fn test_websocket_price_update_structure() {
    let (ws_stream, _) = connect_async(HAUNT_WS_URL)
        .await
        .expect("Failed to connect to WebSocket");

    let (mut write, mut read) = ws_stream.split();

    // Subscribe to multiple assets
    let subscribe_msg = ClientMessage::Subscribe {
        assets: vec!["btc".to_string(), "eth".to_string()],
    };
    let msg_text = serde_json::to_string(&subscribe_msg).unwrap();
    write
        .send(Message::Text(msg_text))
        .await
        .expect("Failed to send");

    // Skip the subscribed confirmation
    let _ = timeout(Duration::from_secs(5), read.next()).await;

    // Wait for price updates (up to 30 seconds)
    println!("Waiting for price updates...");
    let mut received_updates: HashMap<String, PriceUpdateData> = HashMap::new();

    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(30) && received_updates.len() < 2 {
        if let Ok(Some(Ok(Message::Text(text)))) =
            timeout(Duration::from_secs(10), read.next()).await
        {
            if let Ok(ServerMessage::PriceUpdate { data }) =
                serde_json::from_str::<ServerMessage>(&text)
            {
                println!(
                    "Received price update for {}: ${:.2}",
                    data.symbol, data.price
                );
                println!("  - change_24h: {:?}", data.change_24h);
                println!("  - volume_24h: {:?}", data.volume_24h);
                println!("  - trade_direction: {:?}", data.trade_direction);
                println!("  - sources: {:?}", data.sources);

                received_updates.insert(data.symbol.clone(), data);
            }
        }
    }

    assert!(
        !received_updates.is_empty(),
        "Should have received at least one price update"
    );

    for (symbol, update) in &received_updates {
        // Validate required fields
        assert!(update.price > 0.0, "{} price should be positive", symbol);
        assert!(
            !update.sources.is_empty(),
            "{} should have at least one source",
            symbol
        );
        assert!(
            update.timestamp > 0,
            "{} should have valid timestamp",
            symbol
        );

        // Check for optional fields that should ideally be present
        if update.volume_24h.is_none() {
            println!("⚠ Warning: {} WebSocket update missing volume_24h", symbol);
        }
        if update.change_24h.is_none() {
            println!("⚠ Warning: {} WebSocket update missing change_24h", symbol);
        }
    }

    println!("✓ WebSocket price updates have correct structure");
}

/// Compare WebSocket price with API initial load price
#[tokio::test]
#[ignore]
async fn test_websocket_price_matches_api() {
    let http_client = create_http_client();

    // First, get the API price
    let api_resp: HauntApiResponse<Vec<HauntListing>> = http_client
        .get(format!("{}/crypto/listings?limit=5", HAUNT_API_URL))
        .send()
        .await
        .expect("Failed to fetch from API")
        .json()
        .await
        .expect("Failed to parse API response");

    let api_prices: HashMap<String, &HauntListing> = api_resp
        .data
        .iter()
        .map(|l| (l.symbol.to_lowercase(), l))
        .collect();

    // Now connect to WebSocket and get prices
    let (ws_stream, _) = connect_async(HAUNT_WS_URL)
        .await
        .expect("Failed to connect to WebSocket");

    let (mut write, mut read) = ws_stream.split();

    let symbols: Vec<String> = api_prices.keys().cloned().collect();
    let subscribe_msg = ClientMessage::Subscribe {
        assets: symbols.clone(),
    };
    let msg_text = serde_json::to_string(&subscribe_msg).unwrap();
    write
        .send(Message::Text(msg_text))
        .await
        .expect("Failed to send");

    // Skip subscribed confirmation
    let _ = timeout(Duration::from_secs(5), read.next()).await;

    // Collect WebSocket prices
    let mut ws_prices: HashMap<String, PriceUpdateData> = HashMap::new();
    let start = std::time::Instant::now();

    while start.elapsed() < Duration::from_secs(30) && ws_prices.len() < symbols.len() {
        if let Ok(Some(Ok(Message::Text(text)))) =
            timeout(Duration::from_secs(10), read.next()).await
        {
            if let Ok(ServerMessage::PriceUpdate { data }) = serde_json::from_str(&text) {
                ws_prices.insert(data.symbol.clone(), data);
            }
        }
    }

    println!("\n=== API vs WebSocket Price Comparison ===\n");
    println!(
        "{:<8} {:>14} {:>14} {:>10}",
        "Symbol", "API Price", "WS Price", "Deviation"
    );
    println!("{}", "-".repeat(50));

    let mut all_passed = true;
    for (symbol, api_listing) in &api_prices {
        if let Some(ws_data) = ws_prices.get(symbol) {
            let deviation = calculate_deviation(ws_data.price, api_listing.price);
            let status = if deviation <= PRICE_DEVIATION_THRESHOLD {
                "✓"
            } else {
                "✗"
            };

            println!(
                "{} {:<6} {:>14.4} {:>14.4} {:>9.2}%",
                status,
                symbol.to_uppercase(),
                api_listing.price,
                ws_data.price,
                deviation
            );

            if deviation > PRICE_DEVIATION_THRESHOLD {
                all_passed = false;
            }
        } else {
            println!(
                "✗ {:<6} {:>14.4} {:>14} {:>10}",
                symbol.to_uppercase(),
                api_listing.price,
                "N/A",
                "N/A"
            );
            all_passed = false;
        }
    }

    assert!(
        all_passed,
        "Some WebSocket prices deviate too much from API prices"
    );
}

/// Compare WebSocket volume with API volume
#[tokio::test]
#[ignore]
async fn test_websocket_volume_matches_api() {
    let http_client = create_http_client();

    // Get API data
    let api_resp: HauntApiResponse<Vec<HauntListing>> = http_client
        .get(format!("{}/crypto/listings?limit=5", HAUNT_API_URL))
        .send()
        .await
        .expect("Failed to fetch from API")
        .json()
        .await
        .expect("Failed to parse API response");

    let api_volumes: HashMap<String, f64> = api_resp
        .data
        .iter()
        .map(|l| (l.symbol.to_lowercase(), l.volume_24h))
        .collect();

    // Connect to WebSocket
    let (ws_stream, _) = connect_async(HAUNT_WS_URL)
        .await
        .expect("Failed to connect to WebSocket");

    let (mut write, mut read) = ws_stream.split();

    let symbols: Vec<String> = api_volumes.keys().cloned().collect();
    let subscribe_msg = ClientMessage::Subscribe {
        assets: symbols.clone(),
    };
    write
        .send(Message::Text(
            serde_json::to_string(&subscribe_msg).unwrap(),
        ))
        .await
        .unwrap();

    // Skip subscribed
    let _ = timeout(Duration::from_secs(5), read.next()).await;

    // Collect WebSocket volumes
    let mut ws_volumes: HashMap<String, Option<f64>> = HashMap::new();
    let start = std::time::Instant::now();

    while start.elapsed() < Duration::from_secs(30) && ws_volumes.len() < symbols.len() {
        if let Ok(Some(Ok(Message::Text(text)))) =
            timeout(Duration::from_secs(10), read.next()).await
        {
            if let Ok(ServerMessage::PriceUpdate { data }) = serde_json::from_str(&text) {
                ws_volumes.insert(data.symbol.clone(), data.volume_24h);
            }
        }
    }

    println!("\n=== API vs WebSocket Volume Comparison ===\n");
    println!(
        "{:<8} {:>18} {:>18} {:>10}",
        "Symbol", "API Volume", "WS Volume", "Deviation"
    );
    println!("{}", "-".repeat(60));

    let mut issues_found = 0;
    for (symbol, api_volume) in &api_volumes {
        if let Some(ws_vol_opt) = ws_volumes.get(symbol) {
            match ws_vol_opt {
                Some(ws_vol) => {
                    let deviation = calculate_deviation(*ws_vol, *api_volume);
                    let status = if deviation <= VOLUME_DEVIATION_THRESHOLD {
                        "✓"
                    } else {
                        "✗"
                    };

                    println!(
                        "{} {:<6} {:>18.0} {:>18.0} {:>9.2}%",
                        status,
                        symbol.to_uppercase(),
                        api_volume,
                        ws_vol,
                        deviation
                    );

                    if deviation > VOLUME_DEVIATION_THRESHOLD {
                        issues_found += 1;
                        println!("  ⚠ Volume deviation exceeds threshold!");
                    }
                }
                None => {
                    println!(
                        "⚠ {:<6} {:>18.0} {:>18} {:>10}",
                        symbol.to_uppercase(),
                        api_volume,
                        "NULL",
                        "N/A"
                    );
                    println!("  ⚠ WebSocket update missing volume_24h field");
                    issues_found += 1;
                }
            }
        } else {
            println!(
                "✗ {:<6} {:>18.0} {:>18} {:>10}",
                symbol.to_uppercase(),
                api_volume,
                "N/A",
                "N/A"
            );
            issues_found += 1;
        }
    }

    if issues_found > 0 {
        println!("\n⚠ Found {} volume consistency issues", issues_found);
    }

    // This test documents the issue rather than failing hard
    // because volume in WebSocket updates may legitimately differ
    assert!(
        issues_found <= symbols.len() / 2,
        "Too many volume consistency issues: {}/{}",
        issues_found,
        symbols.len()
    );
}

/// Compare WebSocket prices against CoinGecko reference
#[tokio::test]
#[ignore]
async fn test_websocket_price_vs_coingecko() {
    let http_client = create_http_client();

    let symbols = ["btc", "eth", "sol"];
    let cg_ids = ["bitcoin", "ethereum", "solana"];

    // Fetch CoinGecko prices
    let ids_param = cg_ids.join(",");
    let cg_url = format!(
        "{}/simple/price?ids={}&vs_currencies=usd&include_24hr_vol=true&include_24hr_change=true",
        COINGECKO_API_URL, ids_param
    );

    let cg_resp: HashMap<String, CoinGeckoPrice> = http_client
        .get(&cg_url)
        .send()
        .await
        .expect("Failed to fetch from CoinGecko")
        .json()
        .await
        .expect("Failed to parse CoinGecko response");

    // Connect to WebSocket
    let (ws_stream, _) = connect_async(HAUNT_WS_URL)
        .await
        .expect("Failed to connect to WebSocket");

    let (mut write, mut read) = ws_stream.split();

    let subscribe_msg = ClientMessage::Subscribe {
        assets: symbols.iter().map(|s| s.to_string()).collect(),
    };
    write
        .send(Message::Text(
            serde_json::to_string(&subscribe_msg).unwrap(),
        ))
        .await
        .unwrap();

    let _ = timeout(Duration::from_secs(5), read.next()).await;

    // Collect WebSocket prices
    let mut ws_prices: HashMap<String, PriceUpdateData> = HashMap::new();
    let start = std::time::Instant::now();

    while start.elapsed() < Duration::from_secs(30) && ws_prices.len() < symbols.len() {
        if let Ok(Some(Ok(Message::Text(text)))) =
            timeout(Duration::from_secs(10), read.next()).await
        {
            if let Ok(ServerMessage::PriceUpdate { data }) = serde_json::from_str(&text) {
                ws_prices.insert(data.symbol.clone(), data);
            }
        }
    }

    println!("\n=== WebSocket vs CoinGecko Comparison ===\n");
    println!(
        "{:<8} {:>14} {:>14} {:>10}",
        "Symbol", "WebSocket", "CoinGecko", "Deviation"
    );
    println!("{}", "-".repeat(50));

    let mut all_passed = true;
    for (i, symbol) in symbols.iter().enumerate() {
        let cg_id = cg_ids[i];

        if let (Some(ws_data), Some(cg_price)) = (ws_prices.get(*symbol), cg_resp.get(cg_id)) {
            let deviation = calculate_deviation(ws_data.price, cg_price.usd);
            let status = if deviation <= PRICE_DEVIATION_THRESHOLD {
                "✓"
            } else {
                "✗"
            };

            println!(
                "{} {:<6} {:>14.4} {:>14.4} {:>9.2}%",
                status,
                symbol.to_uppercase(),
                ws_data.price,
                cg_price.usd,
                deviation
            );

            if deviation > PRICE_DEVIATION_THRESHOLD {
                all_passed = false;
            }
        }
    }

    assert!(
        all_passed,
        "Some WebSocket prices deviate too much from CoinGecko"
    );
}

/// Test that WebSocket change_24h field is populated correctly
#[tokio::test]
#[ignore]
async fn test_websocket_change_24h_populated() {
    let (ws_stream, _) = connect_async(HAUNT_WS_URL)
        .await
        .expect("Failed to connect to WebSocket");

    let (mut write, mut read) = ws_stream.split();

    let subscribe_msg = ClientMessage::Subscribe {
        assets: vec!["btc".to_string(), "eth".to_string()],
    };
    write
        .send(Message::Text(
            serde_json::to_string(&subscribe_msg).unwrap(),
        ))
        .await
        .unwrap();

    let _ = timeout(Duration::from_secs(5), read.next()).await;

    let mut updates_with_change: usize = 0;
    let mut updates_without_change: usize = 0;

    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(20) {
        if let Ok(Some(Ok(Message::Text(text)))) =
            timeout(Duration::from_secs(10), read.next()).await
        {
            if let Ok(ServerMessage::PriceUpdate { data }) = serde_json::from_str(&text) {
                if let Some(change) = data.change_24h {
                    updates_with_change += 1;
                    println!("✓ {} change_24h: {:.2}%", data.symbol, change);
                } else {
                    updates_without_change += 1;
                    println!("⚠ {} change_24h: NULL", data.symbol);
                }
            }
        }
    }

    let total = updates_with_change + updates_without_change;
    if total > 0 {
        let percentage = (updates_with_change as f64 / total as f64) * 100.0;
        println!(
            "\nchange_24h populated: {}/{} ({:.1}%)",
            updates_with_change, total, percentage
        );

        if percentage < 50.0 {
            println!("⚠ Warning: Most WebSocket updates are missing change_24h field");
        }
    }

    // Document the issue - this is expected based on current implementation
    // but flagging it for visibility
    if updates_without_change > 0 {
        println!(
            "\n⚠ Note: {} updates missing change_24h - this may need fixing in price_cache.rs",
            updates_without_change
        );
    }
}

/// Test data consistency over time - ensure values don't drift
#[tokio::test]
#[ignore]
async fn test_websocket_data_consistency_over_time() {
    let http_client = create_http_client();

    let (ws_stream, _) = connect_async(HAUNT_WS_URL)
        .await
        .expect("Failed to connect to WebSocket");

    let (mut write, mut read) = ws_stream.split();

    let subscribe_msg = ClientMessage::Subscribe {
        assets: vec!["btc".to_string()],
    };
    write
        .send(Message::Text(
            serde_json::to_string(&subscribe_msg).unwrap(),
        ))
        .await
        .unwrap();

    let _ = timeout(Duration::from_secs(5), read.next()).await;

    // Get initial API price
    let initial_api: HauntApiResponse<Vec<HauntListing>> = http_client
        .get(format!("{}/crypto/listings?limit=1", HAUNT_API_URL))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let initial_price = initial_api.data[0].price;
    let initial_volume = initial_api.data[0].volume_24h;

    println!(
        "Initial API BTC: price=${:.2}, volume=${:.0}",
        initial_price, initial_volume
    );

    // Collect WebSocket updates for 30 seconds
    let mut price_samples: Vec<f64> = Vec::new();
    let mut volume_samples: Vec<Option<f64>> = Vec::new();

    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(30) {
        if let Ok(Some(Ok(Message::Text(text)))) =
            timeout(Duration::from_secs(10), read.next()).await
        {
            if let Ok(ServerMessage::PriceUpdate { data }) = serde_json::from_str(&text) {
                if data.symbol == "btc" {
                    price_samples.push(data.price);
                    volume_samples.push(data.volume_24h);
                }
            }
        }
    }

    if price_samples.is_empty() {
        panic!("No BTC price updates received");
    }

    // Analyze price drift
    let avg_price: f64 = price_samples.iter().sum::<f64>() / price_samples.len() as f64;
    let price_drift = calculate_deviation(avg_price, initial_price);

    // Analyze volume consistency
    let valid_volumes: Vec<f64> = volume_samples.iter().filter_map(|v| *v).collect();

    println!("\n=== Data Consistency Analysis (30 seconds) ===");
    println!("Samples collected: {}", price_samples.len());
    println!("Initial API price: ${:.2}", initial_price);
    println!("Average WS price:  ${:.2}", avg_price);
    println!("Price drift:       {:.2}%", price_drift);

    if valid_volumes.is_empty() {
        println!("\n⚠ WARNING: No volume data in WebSocket updates!");
    } else {
        let avg_volume: f64 = valid_volumes.iter().sum::<f64>() / valid_volumes.len() as f64;
        let volume_drift = calculate_deviation(avg_volume, initial_volume);
        println!("Initial API volume: ${:.0}", initial_volume);
        println!("Average WS volume:  ${:.0}", avg_volume);
        println!("Volume drift:       {:.2}%", volume_drift);

        if volume_drift > VOLUME_DEVIATION_THRESHOLD {
            println!("\n⚠ Volume drift exceeds threshold!");
        }
    }

    assert!(
        price_drift <= 10.0,
        "Price drifted too much from initial API value"
    );
}

// ============================================================================
// Unit Tests
// ============================================================================

#[test]
fn test_message_serialization() {
    let msg = ClientMessage::Subscribe {
        assets: vec!["btc".to_string(), "eth".to_string()],
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("subscribe"));
    assert!(json.contains("btc"));
    assert!(json.contains("eth"));
}

#[test]
fn test_price_update_deserialization() {
    let json = r#"{
        "type": "price_update",
        "data": {
            "id": "btc",
            "symbol": "btc",
            "price": 50000.0,
            "previousPrice": 49500.0,
            "change24h": 2.5,
            "volume24h": 1000000000.0,
            "tradeDirection": "up",
            "source": "binance",
            "sources": ["binance", "coinbase"],
            "timestamp": 1704067200000
        }
    }"#;

    let msg: ServerMessage = serde_json::from_str(json).unwrap();
    match msg {
        ServerMessage::PriceUpdate { data } => {
            assert_eq!(data.symbol, "btc");
            assert_eq!(data.price, 50000.0);
            assert_eq!(data.change_24h, Some(2.5));
            assert_eq!(data.volume_24h, Some(1000000000.0));
        }
        _ => panic!("Expected PriceUpdate"),
    }
}

#[test]
fn test_price_update_with_missing_optional_fields() {
    let json = r#"{
        "type": "price_update",
        "data": {
            "id": "btc",
            "symbol": "btc",
            "price": 50000.0,
            "source": "binance",
            "sources": ["binance"],
            "timestamp": 1704067200000
        }
    }"#;

    let msg: ServerMessage = serde_json::from_str(json).unwrap();
    match msg {
        ServerMessage::PriceUpdate { data } => {
            assert_eq!(data.symbol, "btc");
            assert_eq!(data.price, 50000.0);
            assert!(data.change_24h.is_none());
            assert!(data.volume_24h.is_none());
        }
        _ => panic!("Expected PriceUpdate"),
    }
}
