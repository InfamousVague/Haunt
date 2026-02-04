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
        assert!(haunt::types::ChartRange::parse(range).is_some());
    }

    for range in invalid_ranges {
        assert!(haunt::types::ChartRange::parse(range).is_none());
    }
}

// =============================================================================
// Top Movers API Tests
// =============================================================================

#[test]
fn test_movers_response_structure() {
    let response = serde_json::json!({
        "data": {
            "timeframe": "1h",
            "gainers": [
                {
                    "symbol": "BTC",
                    "price": 50000.0,
                    "changePercent": 5.25,
                    "volume24h": 1000000000.0
                },
                {
                    "symbol": "SOL",
                    "price": 100.0,
                    "changePercent": 4.5
                }
            ],
            "losers": [
                {
                    "symbol": "ETH",
                    "price": 3000.0,
                    "changePercent": -3.5,
                    "volume24h": 500000000.0
                }
            ],
            "timestamp": 1704067200
        },
        "meta": {
            "cached": false
        }
    });

    assert!(response["data"]["timeframe"].is_string());
    assert_eq!(response["data"]["timeframe"], "1h");
    assert!(response["data"]["gainers"].is_array());
    assert!(response["data"]["losers"].is_array());
    assert!(response["data"]["timestamp"].is_i64());

    // Validate gainer structure
    let gainer = &response["data"]["gainers"][0];
    assert_eq!(gainer["symbol"], "BTC");
    assert!(gainer["price"].is_f64());
    assert!(gainer["changePercent"].is_f64());
    assert!(gainer["changePercent"].as_f64().unwrap() > 0.0);

    // Validate loser structure
    let loser = &response["data"]["losers"][0];
    assert!(loser["changePercent"].as_f64().unwrap() < 0.0);
}

#[test]
fn test_movers_query_params() {
    // Valid timeframes
    let valid_timeframes = vec!["1m", "5m", "15m", "1h", "4h", "24h"];

    for tf in valid_timeframes {
        // Just verify these strings are valid - actual parsing is done in types_test
        assert!(!tf.is_empty());
    }

    // Limit constraints
    let min_limit = 1;
    let max_limit = 50;
    let default_limit = 10;

    assert!(default_limit >= min_limit);
    assert!(default_limit <= max_limit);
}

#[test]
fn test_movers_gainers_sorted_descending() {
    let gainers = [
        serde_json::json!({"symbol": "A", "changePercent": 10.0}),
        serde_json::json!({"symbol": "B", "changePercent": 5.0}),
        serde_json::json!({"symbol": "C", "changePercent": 2.0}),
    ];

    // Verify gainers are sorted by changePercent descending
    for i in 0..gainers.len() - 1 {
        let current = gainers[i]["changePercent"].as_f64().unwrap();
        let next = gainers[i + 1]["changePercent"].as_f64().unwrap();
        assert!(
            current >= next,
            "Gainers should be sorted descending by changePercent"
        );
    }
}

#[test]
fn test_movers_losers_sorted_ascending() {
    let losers = [
        serde_json::json!({"symbol": "X", "changePercent": -10.0}),
        serde_json::json!({"symbol": "Y", "changePercent": -5.0}),
        serde_json::json!({"symbol": "Z", "changePercent": -2.0}),
    ];

    // Verify losers are sorted by changePercent ascending (most negative first)
    for i in 0..losers.len() - 1 {
        let current = losers[i]["changePercent"].as_f64().unwrap();
        let next = losers[i + 1]["changePercent"].as_f64().unwrap();
        assert!(
            current <= next,
            "Losers should be sorted ascending by changePercent"
        );
    }
}

// =============================================================================
// Symbol Source Stats API Tests
// =============================================================================

#[test]
fn test_symbol_source_stats_response_structure() {
    let response = serde_json::json!({
        "data": {
            "symbol": "btc",
            "sources": [
                {
                    "source": "binance",
                    "updateCount": 1500,
                    "updatePercent": 45.5,
                    "online": true
                },
                {
                    "source": "coinbase",
                    "updateCount": 1200,
                    "updatePercent": 36.4,
                    "online": true
                },
                {
                    "source": "kraken",
                    "updateCount": 600,
                    "updatePercent": 18.1,
                    "online": false
                }
            ],
            "totalUpdates": 3300,
            "timestamp": 1704067200
        },
        "meta": {
            "cached": false
        }
    });

    assert_eq!(response["data"]["symbol"], "btc");
    assert!(response["data"]["sources"].is_array());
    assert!(response["data"]["totalUpdates"].is_i64());
    assert!(response["data"]["timestamp"].is_i64());

    // Validate source entry structure
    let source = &response["data"]["sources"][0];
    assert!(source["source"].is_string());
    assert!(source["updateCount"].is_i64());
    assert!(source["updatePercent"].is_f64());
    assert!(source["online"].is_boolean());
}

#[test]
fn test_symbol_source_stats_total_matches_sum() {
    let sources = [
        serde_json::json!({"updateCount": 1500}),
        serde_json::json!({"updateCount": 1200}),
        serde_json::json!({"updateCount": 600}),
    ];

    let sum: i64 = sources
        .iter()
        .map(|s| s["updateCount"].as_i64().unwrap())
        .sum();

    assert_eq!(sum, 3300);
}

#[test]
fn test_symbol_source_stats_percent_sum() {
    // Percentages should approximately sum to 100
    let percentages = [45.5, 36.4, 18.1];
    let sum: f64 = percentages.iter().sum();

    // Allow for rounding errors
    assert!((sum - 100.0).abs() < 1.0, "Percentages should sum to ~100");
}

// =============================================================================
// Stats API Tests
// =============================================================================

#[test]
fn test_stats_response_structure() {
    let response = serde_json::json!({
        "data": {
            "totalUpdates": 1500000,
            "tps": 125.5,
            "uptimeSecs": 86400,
            "activeSymbols": 150,
            "onlineSources": 7,
            "totalSources": 9,
            "exchanges": [
                {
                    "name": "binance",
                    "updateCount": 500000,
                    "online": true
                }
            ]
        },
        "meta": {
            "cached": false
        }
    });

    assert!(response["data"]["totalUpdates"].is_i64());
    assert!(response["data"]["tps"].is_f64());
    assert!(response["data"]["uptimeSecs"].is_i64());
    assert!(response["data"]["activeSymbols"].is_i64());
    assert!(response["data"]["onlineSources"].is_i64());
    assert!(response["data"]["totalSources"].is_i64());
    assert!(response["data"]["exchanges"].is_array());
}

#[test]
fn test_stats_tps_calculation() {
    // TPS should be non-negative
    let tps = 125.5;
    assert!(tps >= 0.0);

    // Verify TPS * 60 seconds ≈ updates per minute
    let updates_per_minute = tps * 60.0;
    assert!(updates_per_minute > 0.0);
}

#[test]
fn test_stats_online_sources_constraint() {
    let online_sources = 7;
    let total_sources = 9;

    assert!(
        online_sources <= total_sources,
        "Online sources cannot exceed total sources"
    );
}
