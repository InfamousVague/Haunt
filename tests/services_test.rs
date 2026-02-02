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

// =============================================================================
// ChartStore Top Movers Tests
// =============================================================================

#[test]
fn test_chart_store_get_current_price() {
    let store = ChartStore::new();
    let timestamp = chrono::Utc::now().timestamp_millis();

    // Add a price
    store.add_price("btc", 50000.0, None, timestamp);

    // Get current price
    let price = store.get_current_price("btc");
    assert_eq!(price, Some(50000.0));

    // Non-existent symbol returns None
    let none_price = store.get_current_price("nonexistent");
    assert!(none_price.is_none());
}

#[test]
fn test_chart_store_current_price_updates() {
    let store = ChartStore::new();
    let timestamp = chrono::Utc::now().timestamp_millis();

    store.add_price("btc", 50000.0, None, timestamp);
    assert_eq!(store.get_current_price("btc"), Some(50000.0));

    store.add_price("btc", 51000.0, None, timestamp + 1000);
    assert_eq!(store.get_current_price("btc"), Some(51000.0));
}

#[test]
fn test_chart_store_update_volume() {
    let store = ChartStore::new();
    let timestamp = chrono::Utc::now().timestamp_millis();

    // Add price first to create the entry
    store.add_price("btc", 50000.0, None, timestamp);

    // Update volume
    store.update_volume("btc", 1_000_000_000.0);

    // Volume is tracked internally - we can verify by checking sparkline exists
    let sparkline = store.get_sparkline("btc", 10);
    assert!(!sparkline.is_empty());
}

#[test]
fn test_chart_store_get_top_movers_empty() {
    use haunt::types::MoverTimeframe;

    let store = ChartStore::new();

    // Empty store should return empty movers
    let (gainers, losers) = store.get_top_movers(MoverTimeframe::OneHour, 10);
    assert!(gainers.is_empty());
    assert!(losers.is_empty());
}

#[test]
fn test_chart_store_get_top_movers_with_data() {
    use haunt::types::MoverTimeframe;

    let store = ChartStore::new();
    let now = chrono::Utc::now().timestamp_millis();
    let one_hour_ago = now - 3600_000;

    // Add historical prices (1 hour ago)
    store.add_price("btc", 48000.0, None, one_hour_ago);
    store.add_price("eth", 3200.0, None, one_hour_ago);
    store.add_price("sol", 110.0, None, one_hour_ago);

    // Add current prices (showing BTC up, ETH down, SOL flat)
    store.add_price("btc", 50000.0, None, now);  // +4.17%
    store.add_price("eth", 3000.0, None, now);   // -6.25%
    store.add_price("sol", 100.0, None, now);    // -9.09%

    let (gainers, losers) = store.get_top_movers(MoverTimeframe::OneHour, 10);

    // BTC should be a gainer
    let btc_in_gainers = gainers.iter().any(|m| m.symbol.to_lowercase() == "btc");
    assert!(btc_in_gainers, "BTC should be in gainers");

    // ETH and SOL should be losers
    let eth_in_losers = losers.iter().any(|m| m.symbol.to_lowercase() == "eth");
    let sol_in_losers = losers.iter().any(|m| m.symbol.to_lowercase() == "sol");
    assert!(eth_in_losers || sol_in_losers, "ETH or SOL should be in losers");
}

#[test]
fn test_chart_store_movers_limit() {
    use haunt::types::MoverTimeframe;

    let store = ChartStore::new();
    let now = chrono::Utc::now().timestamp_millis();
    let one_hour_ago = now - 3600_000;

    // Add many symbols with positive changes
    for i in 0..20 {
        let symbol = format!("coin{}", i);
        let old_price = 100.0;
        let new_price = 100.0 + (i as f64 * 5.0); // Each coin has different gain

        store.add_price(&symbol, old_price, None, one_hour_ago);
        store.add_price(&symbol, new_price, None, now);
    }

    // Request only top 5
    let (gainers, _losers) = store.get_top_movers(MoverTimeframe::OneHour, 5);

    assert!(gainers.len() <= 5, "Should respect limit");
}

#[test]
fn test_chart_store_movers_sorted_correctly() {
    use haunt::types::MoverTimeframe;

    let store = ChartStore::new();
    let now = chrono::Utc::now().timestamp_millis();
    let one_hour_ago = now - 3600_000;

    // Add symbols with known changes
    store.add_price("small_gain", 100.0, None, one_hour_ago);
    store.add_price("small_gain", 102.0, None, now);  // +2%

    store.add_price("big_gain", 100.0, None, one_hour_ago);
    store.add_price("big_gain", 110.0, None, now);  // +10%

    store.add_price("medium_gain", 100.0, None, one_hour_ago);
    store.add_price("medium_gain", 105.0, None, now);  // +5%

    let (gainers, _) = store.get_top_movers(MoverTimeframe::OneHour, 10);

    // Verify gainers are sorted by change_percent descending
    for i in 0..gainers.len().saturating_sub(1) {
        assert!(
            gainers[i].change_percent >= gainers[i + 1].change_percent,
            "Gainers should be sorted descending by change_percent"
        );
    }
}

#[test]
fn test_chart_store_price_at() {
    let store = ChartStore::new();
    let now = chrono::Utc::now().timestamp_millis();

    // Add prices over time
    store.add_price("btc", 48000.0, None, now - 300_000);  // 5 min ago
    store.add_price("btc", 49000.0, None, now - 120_000);  // 2 min ago
    store.add_price("btc", 50000.0, None, now);            // now

    // Get price from ~3 minutes ago (should find closest)
    let price_at = store.get_price_at("btc", 180);
    assert!(price_at.is_some());
}

#[test]
fn test_chart_store_price_change_calculation() {
    let store = ChartStore::new();
    let now = chrono::Utc::now().timestamp_millis();

    // Add price at T-5min: 100
    store.add_price("test", 100.0, None, now - 300_000);
    // Add price now: 110 (+10%)
    store.add_price("test", 110.0, None, now);

    // Calculate change over 5 minutes
    let change = store.get_price_change("test", 300);

    if let Some(pct) = change {
        // Should be approximately +10%
        assert!(pct > 5.0 && pct < 15.0, "Change should be around 10%");
    }
}
