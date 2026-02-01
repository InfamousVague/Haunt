//! Unit tests for types module

use haunt::types::*;
use serde_json;

#[test]
fn test_price_source_weight() {
    assert_eq!(PriceSource::Coinbase.weight(), 10);
    assert_eq!(PriceSource::CoinMarketCap.weight(), 8);
    assert_eq!(PriceSource::CoinGecko.weight(), 7);
    assert_eq!(PriceSource::CryptoCompare.weight(), 6);
    assert_eq!(PriceSource::Binance.weight(), 9);
}

#[test]
fn test_price_source_display() {
    assert_eq!(format!("{}", PriceSource::Coinbase), "coinbase");
    assert_eq!(format!("{}", PriceSource::CoinGecko), "coingecko");
    assert_eq!(format!("{}", PriceSource::CryptoCompare), "cryptocompare");
    assert_eq!(format!("{}", PriceSource::CoinMarketCap), "coinmarketcap");
    assert_eq!(format!("{}", PriceSource::Binance), "binance");
}

#[test]
fn test_price_source_serialization() {
    let source = PriceSource::Coinbase;
    let json = serde_json::to_string(&source).unwrap();
    assert_eq!(json, "\"coinbase\"");

    let parsed: PriceSource = serde_json::from_str("\"coingecko\"").unwrap();
    assert_eq!(parsed, PriceSource::CoinGecko);
}

#[test]
fn test_chart_range_from_str() {
    assert_eq!(ChartRange::from_str("1h"), Some(ChartRange::OneHour));
    assert_eq!(ChartRange::from_str("4h"), Some(ChartRange::FourHours));
    assert_eq!(ChartRange::from_str("1d"), Some(ChartRange::OneDay));
    assert_eq!(ChartRange::from_str("1w"), Some(ChartRange::OneWeek));
    assert_eq!(ChartRange::from_str("1m"), Some(ChartRange::OneMonth));
    assert_eq!(ChartRange::from_str("invalid"), None);
}

#[test]
fn test_chart_range_bucket_seconds() {
    assert_eq!(ChartRange::OneHour.bucket_seconds(), 60);
    assert_eq!(ChartRange::FourHours.bucket_seconds(), 60);
    assert_eq!(ChartRange::OneDay.bucket_seconds(), 300);
    assert_eq!(ChartRange::OneWeek.bucket_seconds(), 3600);
    assert_eq!(ChartRange::OneMonth.bucket_seconds(), 3600);
}

#[test]
fn test_chart_range_duration_seconds() {
    assert_eq!(ChartRange::OneHour.duration_seconds(), 3600);
    assert_eq!(ChartRange::FourHours.duration_seconds(), 14400);
    assert_eq!(ChartRange::OneDay.duration_seconds(), 86400);
    assert_eq!(ChartRange::OneWeek.duration_seconds(), 604800);
    assert_eq!(ChartRange::OneMonth.duration_seconds(), 2592000);
}

#[test]
fn test_chart_resolution_seconds() {
    assert_eq!(ChartResolution::OneMinute.seconds(), 60);
    assert_eq!(ChartResolution::FiveMinute.seconds(), 300);
    assert_eq!(ChartResolution::OneHour.seconds(), 3600);
}

#[test]
fn test_chart_resolution_retention() {
    assert_eq!(ChartResolution::OneMinute.retention_seconds(), 3600);
    assert_eq!(ChartResolution::FiveMinute.retention_seconds(), 86400);
    assert_eq!(ChartResolution::OneHour.retention_seconds(), 2592000);
}

#[test]
fn test_fear_greed_classification() {
    assert_eq!(FearGreedData::classify(0), "Extreme Fear");
    assert_eq!(FearGreedData::classify(24), "Extreme Fear");
    assert_eq!(FearGreedData::classify(25), "Fear");
    assert_eq!(FearGreedData::classify(44), "Fear");
    assert_eq!(FearGreedData::classify(45), "Neutral");
    assert_eq!(FearGreedData::classify(55), "Neutral");
    assert_eq!(FearGreedData::classify(56), "Greed");
    assert_eq!(FearGreedData::classify(75), "Greed");
    assert_eq!(FearGreedData::classify(76), "Extreme Greed");
    assert_eq!(FearGreedData::classify(100), "Extreme Greed");
}

#[test]
fn test_aggregation_config_default() {
    let config = AggregationConfig::default();
    assert_eq!(config.change_threshold, 0.01);
    assert_eq!(config.throttle_ms, 100);
    assert_eq!(config.stale_threshold_ms, 120_000);
}

#[test]
fn test_asset_serialization() {
    let asset = Asset {
        id: 1,
        name: "Bitcoin".to_string(),
        symbol: "BTC".to_string(),
        slug: "bitcoin".to_string(),
        logo: Some("https://example.com/btc.png".to_string()),
        description: None,
        category: None,
        date_added: None,
        tags: None,
        urls: None,
        quote: Some(Quote {
            price: 50000.0,
            volume_24h: Some(1000000000.0),
            volume_change_24h: None,
            market_cap: Some(1000000000000.0),
            market_cap_dominance: Some(50.0),
            percent_change_1h: Some(0.5),
            percent_change_24h: Some(2.0),
            percent_change_7d: Some(5.0),
            percent_change_30d: Some(10.0),
            fully_diluted_market_cap: None,
            circulating_supply: None,
            total_supply: None,
            max_supply: None,
            last_updated: None,
        }),
    };

    let json = serde_json::to_string(&asset).unwrap();
    assert!(json.contains("\"id\":1"));
    assert!(json.contains("\"name\":\"Bitcoin\""));
    assert!(json.contains("\"symbol\":\"BTC\""));
    assert!(json.contains("\"price\":50000.0"));
}

#[test]
fn test_ohlc_point_serialization() {
    let point = OhlcPoint {
        time: 1704067200,
        open: 100.0,
        high: 110.0,
        low: 95.0,
        close: 105.0,
        volume: Some(1000.0),
    };

    let json = serde_json::to_string(&point).unwrap();
    let parsed: OhlcPoint = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.time, point.time);
    assert_eq!(parsed.open, point.open);
    assert_eq!(parsed.high, point.high);
    assert_eq!(parsed.low, point.low);
    assert_eq!(parsed.close, point.close);
    assert_eq!(parsed.volume, point.volume);
}

#[test]
fn test_client_message_deserialization() {
    let subscribe_json = r#"{"type":"subscribe","assets":["btc","eth"]}"#;
    let msg: ClientMessage = serde_json::from_str(subscribe_json).unwrap();
    match msg {
        ClientMessage::Subscribe { assets } => {
            assert_eq!(assets, vec!["btc", "eth"]);
        }
        _ => panic!("Expected Subscribe message"),
    }

    let unsubscribe_json = r#"{"type":"unsubscribe","assets":["btc"]}"#;
    let msg: ClientMessage = serde_json::from_str(unsubscribe_json).unwrap();
    match msg {
        ClientMessage::Unsubscribe { assets } => {
            assert_eq!(assets, vec!["btc"]);
        }
        _ => panic!("Expected Unsubscribe message"),
    }
}

#[test]
fn test_server_message_serialization() {
    let msg = ServerMessage::Subscribed {
        assets: vec!["btc".to_string(), "eth".to_string()],
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"type\":\"subscribed\""));
    assert!(json.contains("\"assets\":[\"btc\",\"eth\"]"));

    let msg = ServerMessage::Error {
        error: "test error".to_string(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"type\":\"error\""));
    assert!(json.contains("\"error\":\"test error\""));
}

#[test]
fn test_price_update_data_from_aggregated_price() {
    let aggregated = AggregatedPrice {
        id: "btc".to_string(),
        symbol: "btc".to_string(),
        price: 50000.0,
        previous_price: Some(49000.0),
        change_24h: Some(2.04),
        volume_24h: Some(1000000000.0),
        source: PriceSource::Coinbase,
        sources: vec![PriceSource::Coinbase, PriceSource::CoinGecko],
        timestamp: 1704067200000,
    };

    let update: PriceUpdateData = aggregated.into();
    assert_eq!(update.id, "btc");
    assert_eq!(update.symbol, "btc");
    assert_eq!(update.price, 50000.0);
    assert_eq!(update.previous_price, Some(49000.0));
    assert_eq!(update.change_24h, Some(2.04));
    assert_eq!(update.source, PriceSource::Coinbase);
    assert_eq!(update.sources.len(), 2);
}

#[test]
fn test_paginated_response() {
    let response = PaginatedResponse {
        data: vec![1, 2, 3],
        page: 1,
        limit: 10,
        total: 100,
        has_more: true,
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"page\":1"));
    assert!(json.contains("\"limit\":10"));
    assert!(json.contains("\"total\":100"));
    assert!(json.contains("\"hasMore\":true"));
}
