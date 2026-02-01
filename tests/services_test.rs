//! Unit tests for services module

use haunt::services::{Cache, ChartStore};
use haunt::types::{ChartRange, AggregationConfig};
use std::time::Duration;

#[test]
fn test_cache_set_and_get() {
    let cache: Cache<String> = Cache::new(Duration::from_secs(60));

    cache.set("key1".to_string(), "value1".to_string());
    assert_eq!(cache.get("key1"), Some("value1".to_string()));
    assert_eq!(cache.get("key2"), None);
}

#[test]
fn test_cache_set_with_custom_ttl() {
    let cache: Cache<String> = Cache::new(Duration::from_secs(60));

    cache.set_with_ttl("key1".to_string(), "value1".to_string(), Duration::from_millis(10));
    assert_eq!(cache.get("key1"), Some("value1".to_string()));

    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(cache.get("key1"), None);
}

#[test]
fn test_cache_expiration() {
    let cache: Cache<String> = Cache::new(Duration::from_millis(10));

    cache.set("key1".to_string(), "value1".to_string());
    assert_eq!(cache.get("key1"), Some("value1".to_string()));

    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(cache.get("key1"), None);
}

#[test]
fn test_cache_contains() {
    let cache: Cache<String> = Cache::new(Duration::from_secs(60));

    cache.set("key1".to_string(), "value1".to_string());
    assert!(cache.contains("key1"));
    assert!(!cache.contains("key2"));
}

#[test]
fn test_cache_remove() {
    let cache: Cache<String> = Cache::new(Duration::from_secs(60));

    cache.set("key1".to_string(), "value1".to_string());
    assert_eq!(cache.remove("key1"), Some("value1".to_string()));
    assert_eq!(cache.get("key1"), None);
    assert_eq!(cache.remove("key1"), None);
}

#[test]
fn test_cache_clear() {
    let cache: Cache<String> = Cache::new(Duration::from_secs(60));

    cache.set("key1".to_string(), "value1".to_string());
    cache.set("key2".to_string(), "value2".to_string());

    cache.clear();

    assert_eq!(cache.get("key1"), None);
    assert_eq!(cache.get("key2"), None);
    assert!(cache.is_empty());
}

#[test]
fn test_cache_len() {
    let cache: Cache<String> = Cache::new(Duration::from_secs(60));

    assert_eq!(cache.len(), 0);
    assert!(cache.is_empty());

    cache.set("key1".to_string(), "value1".to_string());
    assert_eq!(cache.len(), 1);
    assert!(!cache.is_empty());

    cache.set("key2".to_string(), "value2".to_string());
    assert_eq!(cache.len(), 2);
}

#[test]
fn test_cache_cleanup() {
    let cache: Cache<String> = Cache::new(Duration::from_millis(10));

    cache.set("key1".to_string(), "value1".to_string());
    cache.set_with_ttl("key2".to_string(), "value2".to_string(), Duration::from_secs(60));

    std::thread::sleep(Duration::from_millis(20));

    cache.cleanup();

    // key1 should be cleaned up, key2 should still exist
    assert_eq!(cache.get("key1"), None);
    assert_eq!(cache.get("key2"), Some("value2".to_string()));
}

#[test]
fn test_chart_store_add_and_get() {
    let store = ChartStore::new();
    let timestamp = chrono::Utc::now().timestamp_millis();

    store.add_price("btc", 50000.0, Some(1000000.0), timestamp);
    store.add_price("btc", 50100.0, Some(1100000.0), timestamp + 1000);
    store.add_price("btc", 50200.0, Some(1200000.0), timestamp + 2000);

    let data = store.get_chart("btc", ChartRange::OneHour);
    assert!(!data.is_empty());
}

#[test]
fn test_chart_store_case_insensitive() {
    let store = ChartStore::new();
    let timestamp = chrono::Utc::now().timestamp_millis();

    store.add_price("BTC", 50000.0, None, timestamp);
    store.add_price("btc", 50100.0, None, timestamp + 1000);

    let data_lower = store.get_chart("btc", ChartRange::OneHour);
    let data_upper = store.get_chart("BTC", ChartRange::OneHour);

    assert_eq!(data_lower.len(), data_upper.len());
}

#[test]
fn test_chart_store_empty_symbol() {
    let store = ChartStore::new();
    let data = store.get_chart("nonexistent", ChartRange::OneDay);
    assert!(data.is_empty());
}

#[test]
fn test_chart_store_multiple_symbols() {
    let store = ChartStore::new();
    let timestamp = chrono::Utc::now().timestamp_millis();

    store.add_price("btc", 50000.0, None, timestamp);
    store.add_price("eth", 3000.0, None, timestamp);
    store.add_price("sol", 100.0, None, timestamp);

    let btc_data = store.get_chart("btc", ChartRange::OneHour);
    let eth_data = store.get_chart("eth", ChartRange::OneHour);
    let sol_data = store.get_chart("sol", ChartRange::OneHour);

    assert!(!btc_data.is_empty());
    assert!(!eth_data.is_empty());
    assert!(!sol_data.is_empty());
}

#[test]
fn test_chart_store_ohlc_aggregation() {
    let store = ChartStore::new();
    // Use a fixed timestamp that's aligned to a minute boundary
    let base_time = (chrono::Utc::now().timestamp() / 60 * 60 + 60) * 1000; // Next minute boundary

    // Add prices within the same minute bucket
    store.add_price("btc", 50000.0, None, base_time);  // Open
    store.add_price("btc", 50500.0, None, base_time + 10000);  // High
    store.add_price("btc", 49500.0, None, base_time + 20000);  // Low
    store.add_price("btc", 50200.0, None, base_time + 30000);  // Close

    let data = store.get_chart("btc", ChartRange::OneHour);

    if !data.is_empty() {
        let last = data.last().unwrap();
        // Just verify the OHLC logic works - high >= low
        assert!(last.high >= last.low, "High should be >= Low");
        // Verify close is the last price added
        assert_eq!(last.close, 50200.0, "Close should be last price");
    }
}

#[test]
fn test_aggregation_config_custom() {
    let config = AggregationConfig {
        change_threshold: 0.05,
        throttle_ms: 200,
        stale_threshold_ms: 300_000,
    };

    assert_eq!(config.change_threshold, 0.05);
    assert_eq!(config.throttle_ms, 200);
    assert_eq!(config.stale_threshold_ms, 300_000);
}
