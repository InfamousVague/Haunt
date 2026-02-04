//! Unit tests for WebSocket module

use haunt::types::{ClientMessage, PriceSource, PriceUpdateData, ServerMessage};

#[test]
fn test_client_message_subscribe_parsing() {
    let json = r#"{"type":"subscribe","assets":["btc","eth","sol"]}"#;
    let msg: ClientMessage = serde_json::from_str(json).unwrap();

    match msg {
        ClientMessage::Subscribe { assets } => {
            assert_eq!(assets.len(), 3);
            assert!(assets.contains(&"btc".to_string()));
            assert!(assets.contains(&"eth".to_string()));
            assert!(assets.contains(&"sol".to_string()));
        }
        _ => panic!("Expected Subscribe message"),
    }
}

#[test]
fn test_client_message_unsubscribe_parsing() {
    let json = r#"{"type":"unsubscribe","assets":["btc"]}"#;
    let msg: ClientMessage = serde_json::from_str(json).unwrap();

    match msg {
        ClientMessage::Unsubscribe { assets } => {
            assert_eq!(assets.len(), 1);
            assert_eq!(assets[0], "btc");
        }
        _ => panic!("Expected Unsubscribe message"),
    }
}

#[test]
fn test_client_message_empty_assets() {
    let json = r#"{"type":"subscribe","assets":[]}"#;
    let msg: ClientMessage = serde_json::from_str(json).unwrap();

    match msg {
        ClientMessage::Subscribe { assets } => {
            assert!(assets.is_empty());
        }
        _ => panic!("Expected Subscribe message"),
    }
}

#[test]
fn test_server_message_price_update() {
    let data = PriceUpdateData {
        id: "btc".to_string(),
        symbol: "btc".to_string(),
        price: 50000.0,
        previous_price: Some(49500.0),
        change_24h: Some(2.5),
        volume_24h: Some(1000000000.0),
        trade_direction: None,
        source: PriceSource::Coinbase,
        sources: vec![PriceSource::Coinbase, PriceSource::CoinGecko],
        timestamp: 1704067200000,
        asset_type: None,
    };

    let msg = ServerMessage::PriceUpdate { data };
    let json = serde_json::to_string(&msg).unwrap();

    assert!(json.contains("\"type\":\"price_update\""));
    assert!(json.contains("\"symbol\":\"btc\""));
    assert!(json.contains("\"price\":50000.0"));
    assert!(json.contains("\"source\":\"coinbase\""));
}

#[test]
fn test_server_message_subscribed() {
    let msg = ServerMessage::Subscribed {
        assets: vec!["btc".to_string(), "eth".to_string()],
    };
    let json = serde_json::to_string(&msg).unwrap();

    assert!(json.contains("\"type\":\"subscribed\""));
    assert!(json.contains("\"assets\":[\"btc\",\"eth\"]"));
}

#[test]
fn test_server_message_unsubscribed() {
    let msg = ServerMessage::Unsubscribed {
        assets: vec!["btc".to_string()],
    };
    let json = serde_json::to_string(&msg).unwrap();

    assert!(json.contains("\"type\":\"unsubscribed\""));
    assert!(json.contains("\"assets\":[\"btc\"]"));
}

#[test]
fn test_server_message_error() {
    let msg = ServerMessage::Error {
        error: "Invalid message format".to_string(),
    };
    let json = serde_json::to_string(&msg).unwrap();

    assert!(json.contains("\"type\":\"error\""));
    assert!(json.contains("\"error\":\"Invalid message format\""));
}

#[test]
fn test_price_update_optional_fields() {
    let data = PriceUpdateData {
        id: "btc".to_string(),
        symbol: "btc".to_string(),
        price: 50000.0,
        previous_price: None,
        change_24h: None,
        volume_24h: None,
        trade_direction: None,
        source: PriceSource::CoinGecko,
        sources: vec![PriceSource::CoinGecko],
        timestamp: 1704067200000,
        asset_type: None,
    };

    let msg = ServerMessage::PriceUpdate { data };
    let json = serde_json::to_string(&msg).unwrap();

    // Optional fields should be omitted when None
    assert!(!json.contains("previousPrice"));
    assert!(!json.contains("change24h"));
    assert!(!json.contains("volume24h"));
}

#[test]
fn test_invalid_client_message() {
    let invalid_json = r#"{"type":"invalid","assets":["btc"]}"#;
    let result: Result<ClientMessage, _> = serde_json::from_str(invalid_json);
    assert!(result.is_err());
}

#[test]
fn test_malformed_json() {
    let malformed = r#"{"type":"subscribe","assets":}"#;
    let result: Result<ClientMessage, _> = serde_json::from_str(malformed);
    assert!(result.is_err());
}

#[test]
fn test_missing_required_field() {
    let missing_assets = r#"{"type":"subscribe"}"#;
    let result: Result<ClientMessage, _> = serde_json::from_str(missing_assets);
    assert!(result.is_err());
}

#[test]
fn test_case_sensitivity() {
    // Message type should be lowercase
    let uppercase = r#"{"type":"SUBSCRIBE","assets":["btc"]}"#;
    let result: Result<ClientMessage, _> = serde_json::from_str(uppercase);
    assert!(result.is_err());
}

#[test]
fn test_price_source_in_update() {
    let sources = vec![
        PriceSource::Coinbase,
        PriceSource::CoinGecko,
        PriceSource::CryptoCompare,
        PriceSource::CoinMarketCap,
        PriceSource::Binance,
    ];

    for source in sources {
        let data = PriceUpdateData {
            id: "btc".to_string(),
            symbol: "btc".to_string(),
            price: 50000.0,
            previous_price: None,
            change_24h: None,
            volume_24h: None,
            trade_direction: None,
            source,
            sources: vec![source],
            timestamp: 1704067200000,
            asset_type: None,
        };

        let msg = ServerMessage::PriceUpdate { data };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(&format!("\"{}\"", source)));
    }
}
