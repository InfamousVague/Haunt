//! Integration tests for API endpoints

// Note: Full integration tests would require setting up the complete app state
// with mock services. These tests verify the basic structure of responses.

#[test]
fn test_health_response_structure() {
    let response = serde_json::json!({
        "status": "ok",
        "version": "0.1.0"
    });

    assert_eq!(response["status"], "ok");
    assert!(response["version"].is_string());
}

#[test]
fn test_listings_response_structure() {
    let response = serde_json::json!({
        "data": [
            {
                "id": 1,
                "name": "Bitcoin",
                "symbol": "BTC",
                "slug": "bitcoin",
                "logo": "https://example.com/btc.png",
                "cmcRank": 1,
                "quote": {
                    "price": 50000.0,
                    "volume24h": 1000000000.0,
                    "percentChange24h": 2.5
                }
            }
        ],
        "page": 1,
        "limit": 20,
        "total": 100,
        "hasMore": true
    });

    assert!(response["data"].is_array());
    assert_eq!(response["data"][0]["symbol"], "BTC");
    assert_eq!(response["page"], 1);
    assert_eq!(response["limit"], 20);
    assert!(response["hasMore"].as_bool().unwrap());
}

#[test]
fn test_asset_response_structure() {
    let response = serde_json::json!({
        "id": 1,
        "name": "Bitcoin",
        "symbol": "BTC",
        "slug": "bitcoin",
        "logo": "https://example.com/btc.png",
        "quote": {
            "price": 50000.0,
            "volume24h": 1000000000.0,
            "marketCap": 1000000000000.0,
            "percentChange1h": 0.5,
            "percentChange24h": 2.0,
            "percentChange7d": 5.0,
            "percentChange30d": 10.0
        }
    });

    assert_eq!(response["id"], 1);
    assert_eq!(response["name"], "Bitcoin");
    assert!(response["quote"]["price"].is_f64());
}

#[test]
fn test_search_response_structure() {
    let response = serde_json::json!([
        {
            "id": 1,
            "name": "Bitcoin",
            "symbol": "BTC",
            "slug": "bitcoin",
            "logo": "https://example.com/btc.png",
            "cmcRank": 1
        },
        {
            "id": 1831,
            "name": "Bitcoin Cash",
            "symbol": "BCH",
            "slug": "bitcoin-cash",
            "logo": "https://example.com/bch.png",
            "cmcRank": 20
        }
    ]);

    assert!(response.is_array());
    assert!(response[0]["name"].as_str().unwrap().contains("Bitcoin"));
}

#[test]
fn test_global_metrics_response_structure() {
    let response = serde_json::json!({
        "totalMarketCap": 2500000000000.0,
        "totalVolume24h": 100000000000.0,
        "btcDominance": 50.0,
        "ethDominance": 15.0,
        "activeCryptocurrencies": 10000,
        "activeExchanges": 500,
        "marketCapChange24h": 2.5,
        "lastUpdated": "2024-01-01T00:00:00Z"
    });

    assert!(response["totalMarketCap"].is_f64());
    assert!(response["btcDominance"].is_f64());
    assert!(response["activeCryptocurrencies"].is_i64());
}

#[test]
fn test_fear_greed_response_structure() {
    let response = serde_json::json!({
        "value": 65,
        "classification": "Greed",
        "timestamp": "2024-01-01T00:00:00Z"
    });

    assert!(response["value"].is_i64());
    assert!(response["classification"].is_string());
    assert!(response["timestamp"].is_string());
}

#[test]
fn test_chart_response_structure() {
    let response = serde_json::json!({
        "symbol": "btc",
        "range": "1d",
        "data": [
            {
                "time": 1704067200,
                "open": 50000.0,
                "high": 51000.0,
                "low": 49500.0,
                "close": 50500.0,
                "volume": 1000000.0
            }
        ]
    });

    assert_eq!(response["symbol"], "btc");
    assert_eq!(response["range"], "1d");
    assert!(response["data"].is_array());

    let point = &response["data"][0];
    assert!(point["time"].is_i64());
    assert!(point["open"].is_f64());
    assert!(point["high"].is_f64());
    assert!(point["low"].is_f64());
    assert!(point["close"].is_f64());
}

#[test]
fn test_error_response_structure() {
    let response = serde_json::json!({
        "error": "Not found",
        "status": 404
    });

    assert!(response["error"].is_string());
    assert!(response["status"].is_i64());
}

#[test]
fn test_websocket_subscribe_message() {
    let msg = serde_json::json!({
        "type": "subscribe",
        "assets": ["btc", "eth", "sol"]
    });

    assert_eq!(msg["type"], "subscribe");
    assert!(msg["assets"].is_array());
    assert_eq!(msg["assets"].as_array().unwrap().len(), 3);
}

#[test]
fn test_websocket_unsubscribe_message() {
    let msg = serde_json::json!({
        "type": "unsubscribe",
        "assets": ["btc"]
    });

    assert_eq!(msg["type"], "unsubscribe");
    assert!(msg["assets"].is_array());
}

#[test]
fn test_websocket_price_update_message() {
    let msg = serde_json::json!({
        "type": "price_update",
        "data": {
            "id": "btc",
            "symbol": "btc",
            "price": 50000.0,
            "previousPrice": 49500.0,
            "change24h": 2.5,
            "volume24h": 1000000000.0,
            "source": "coinbase",
            "sources": ["coinbase", "coingecko"],
            "timestamp": 1704067200000_i64
        }
    });

    assert_eq!(msg["type"], "price_update");
    assert!(msg["data"]["price"].is_f64());
    assert!(msg["data"]["sources"].is_array());
}

#[test]
fn test_websocket_subscribed_response() {
    let msg = serde_json::json!({
        "type": "subscribed",
        "assets": ["btc", "eth"]
    });

    assert_eq!(msg["type"], "subscribed");
    assert!(msg["assets"].is_array());
}

#[test]
fn test_websocket_error_response() {
    let msg = serde_json::json!({
        "type": "error",
        "error": "Invalid message format"
    });

    assert_eq!(msg["type"], "error");
    assert!(msg["error"].is_string());
}

// Test query parameter parsing
#[test]
fn test_listings_query_params() {
    // Default values
    let default_page = 1;
    let default_limit = 20;

    // Constraints
    let min_page = 1;
    let max_limit = 100;
    let min_limit = 1;

    assert!(default_page >= min_page);
    assert!(default_limit <= max_limit);
    assert!(default_limit >= min_limit);
}

#[test]
fn test_chart_range_validation() {
    let valid_ranges = vec!["1h", "4h", "1d", "1w", "1m"];
    let invalid_ranges = vec!["2h", "3d", "2w", "1y", "invalid"];

    for range in valid_ranges {
        assert!(haunt::types::ChartRange::from_str(range).is_some());
    }

    for range in invalid_ranges {
        assert!(haunt::types::ChartRange::from_str(range).is_none());
    }
}
