//! Peer mesh integration tests
//!
//! Run with: cargo test --test peer_mesh_test -- --nocapture

use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};

const WS_URL: &str = "ws://localhost:3001/ws";

/// Test that we can subscribe to peer updates and receive them
#[tokio::test]
async fn test_peer_subscription() {
    let (ws_stream, _) = connect_async(WS_URL)
        .await
        .expect("Failed to connect to WebSocket");

    let (mut write, mut read) = ws_stream.split();

    // Subscribe to peers
    let subscribe_msg = json!({ "type": "subscribe_peers" });
    write
        .send(Message::Text(subscribe_msg.to_string()))
        .await
        .expect("Failed to send subscribe_peers");

    // Wait for subscription confirmation
    let mut confirmed = false;
    let mut received_updates = 0;

    let timeout = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(Ok(msg)) = read.next().await {
            if let Message::Text(text) = msg {
                let json: Value = serde_json::from_str(&text).expect("Invalid JSON");

                match json["type"].as_str() {
                    Some("peers_subscribed") => {
                        println!("✓ Received peers_subscribed confirmation");
                        confirmed = true;
                    }
                    Some("peer_update") => {
                        received_updates += 1;
                        let peers = json["data"]["peers"].as_array().expect("Missing peers array");

                        println!("✓ Peer update #{}: {} peers", received_updates, peers.len());

                        for peer in peers {
                            let id = peer["id"].as_str().unwrap_or("unknown");
                            let status = peer["status"].as_str().unwrap_or("unknown");
                            let latency = peer["latencyMs"].as_f64().unwrap_or(0.0);
                            let ping_count = peer["pingCount"].as_u64().unwrap_or(0);

                            println!("  - {}: status={}, latency={:.2}ms, pingCount={}",
                                id, status, latency, ping_count);
                        }

                        if received_updates >= 3 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    timeout.await.expect("Test timed out");

    assert!(confirmed, "Did not receive peers_subscribed confirmation");
    assert!(received_updates >= 3, "Did not receive enough peer updates");
}

/// Test that ping/pong messages work correctly
#[tokio::test]
async fn test_ping_pong_protocol() {
    let (ws_stream, _) = connect_async(WS_URL)
        .await
        .expect("Failed to connect to WebSocket");

    let (mut write, mut read) = ws_stream.split();

    // Send a peer mesh ping
    let timestamp = chrono::Utc::now().timestamp_millis();
    let ping_msg = json!({
        "type": "ping",
        "from_id": "test-client",
        "from_region": "Test",
        "timestamp": timestamp
    });

    write
        .send(Message::Text(ping_msg.to_string()))
        .await
        .expect("Failed to send ping");

    // Wait for pong response
    let timeout = tokio::time::timeout(Duration::from_secs(2), async {
        while let Some(Ok(msg)) = read.next().await {
            if let Message::Text(text) = msg {
                let json: Value = serde_json::from_str(&text).expect("Invalid JSON");

                if json["type"].as_str() == Some("pong") {
                    let original_ts = json["original_timestamp"].as_i64().expect("Missing original_timestamp");
                    let from_id = json["from_id"].as_str().expect("Missing from_id");

                    assert_eq!(original_ts, timestamp, "Timestamp mismatch in pong");
                    println!("✓ Received pong from {} with correct timestamp", from_id);
                    return;
                }
            }
        }
        panic!("Did not receive pong response");
    });

    timeout.await.expect("Pong response timed out");
}

/// Test that peer latency is being recorded
#[tokio::test]
async fn test_latency_recording() {
    let (ws_stream, _) = connect_async(WS_URL)
        .await
        .expect("Failed to connect to WebSocket");

    let (mut write, mut read) = ws_stream.split();

    // Subscribe to peers
    let subscribe_msg = json!({ "type": "subscribe_peers" });
    write
        .send(Message::Text(subscribe_msg.to_string()))
        .await
        .expect("Failed to send subscribe_peers");

    let mut found_latency = false;

    let timeout = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(Ok(msg)) = read.next().await {
            if let Message::Text(text) = msg {
                let json: Value = serde_json::from_str(&text).expect("Invalid JSON");

                if json["type"].as_str() == Some("peer_update") {
                    let peers = json["data"]["peers"].as_array().expect("Missing peers array");

                    for peer in peers {
                        let latency = peer["latencyMs"].as_f64();
                        let ping_count = peer["pingCount"].as_u64().unwrap_or(0);

                        if latency.is_some() && ping_count > 0 {
                            let id = peer["id"].as_str().unwrap_or("unknown");
                            println!("✓ Found peer {} with latency {:.2}ms (pingCount={})",
                                id, latency.unwrap(), ping_count);
                            found_latency = true;
                        }
                    }

                    if found_latency {
                        break;
                    }
                }
            }
        }
    });

    timeout.await.expect("Test timed out");

    assert!(found_latency, "No peer with recorded latency found (make sure haunt is running with peers configured)");
}
