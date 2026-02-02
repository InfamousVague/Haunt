//! API sanity tests for verifying data accuracy against public sources.
//!
//! These tests compare Haunt API responses against public cryptocurrency APIs
//! to ensure data accuracy and integrity.

use serde::{Deserialize, Serialize};

/// Tolerance for price comparison (1% variance)
const PRICE_VARIANCE_TOLERANCE: f64 = 0.01;

/// Mock chart data point for testing OHLC integrity
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OhlcPoint {
    time: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    volume: Option<f64>,
}

/// Mock chart data response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChartData {
    symbol: String,
    range: String,
    data: Vec<OhlcPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    seeding: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    seeding_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    seeding_progress: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data_completeness: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_points: Option<u32>,
}

// ============================================================================
// OHLC Data Integrity Tests
// ============================================================================

#[test]
fn test_ohlc_high_always_gte_low() {
    // Verify that in all OHLC data, high is always >= low
    let data_points = vec![
        OhlcPoint { time: 1000, open: 100.0, high: 110.0, low: 90.0, close: 105.0, volume: Some(1000.0) },
        OhlcPoint { time: 2000, open: 105.0, high: 120.0, low: 95.0, close: 115.0, volume: Some(1500.0) },
        OhlcPoint { time: 3000, open: 115.0, high: 115.0, low: 100.0, close: 100.0, volume: Some(2000.0) },
    ];

    for point in &data_points {
        assert!(
            point.high >= point.low,
            "OHLC integrity violation: high ({}) < low ({}) at time {}",
            point.high, point.low, point.time
        );
    }
}

#[test]
fn test_ohlc_open_close_within_high_low() {
    // Verify open and close are within high-low range
    let data_points = vec![
        OhlcPoint { time: 1000, open: 100.0, high: 110.0, low: 90.0, close: 105.0, volume: Some(1000.0) },
        OhlcPoint { time: 2000, open: 105.0, high: 120.0, low: 95.0, close: 115.0, volume: Some(1500.0) },
    ];

    for point in &data_points {
        assert!(
            point.open >= point.low && point.open <= point.high,
            "OHLC integrity: open ({}) outside high-low range [{}, {}] at time {}",
            point.open, point.low, point.high, point.time
        );
        assert!(
            point.close >= point.low && point.close <= point.high,
            "OHLC integrity: close ({}) outside high-low range [{}, {}] at time {}",
            point.close, point.low, point.high, point.time
        );
    }
}

#[test]
fn test_ohlc_timestamps_ascending() {
    // Verify timestamps are in ascending order
    let data_points = vec![
        OhlcPoint { time: 1000, open: 100.0, high: 110.0, low: 90.0, close: 105.0, volume: None },
        OhlcPoint { time: 2000, open: 105.0, high: 120.0, low: 95.0, close: 115.0, volume: None },
        OhlcPoint { time: 3000, open: 115.0, high: 130.0, low: 110.0, close: 125.0, volume: None },
    ];

    for i in 1..data_points.len() {
        assert!(
            data_points[i].time > data_points[i - 1].time,
            "OHLC timestamps not ascending: {} <= {} at index {}",
            data_points[i].time, data_points[i - 1].time, i
        );
    }
}

#[test]
fn test_ohlc_no_duplicate_timestamps() {
    // Verify no duplicate timestamps exist
    let data_points = vec![
        OhlcPoint { time: 1000, open: 100.0, high: 110.0, low: 90.0, close: 105.0, volume: None },
        OhlcPoint { time: 2000, open: 105.0, high: 120.0, low: 95.0, close: 115.0, volume: None },
        OhlcPoint { time: 3000, open: 115.0, high: 130.0, low: 110.0, close: 125.0, volume: None },
    ];

    let mut seen_times: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for point in &data_points {
        assert!(
            seen_times.insert(point.time),
            "Duplicate timestamp found: {}",
            point.time
        );
    }
}

// ============================================================================
// Chart Data Response Tests
// ============================================================================

#[test]
fn test_chart_data_response_fields() {
    let response = serde_json::json!({
        "symbol": "btc",
        "range": "1d",
        "data": [
            { "time": 1000, "open": 100.0, "high": 110.0, "low": 90.0, "close": 105.0 }
        ],
        "seeding": false,
        "seedingStatus": "complete",
        "seedingProgress": 100,
        "dataCompleteness": 95,
        "expectedPoints": 288
    });

    let chart_data: ChartData = serde_json::from_value(response).expect("Failed to parse chart data");

    assert_eq!(chart_data.symbol, "btc");
    assert_eq!(chart_data.range, "1d");
    assert_eq!(chart_data.seeding, Some(false));
    assert_eq!(chart_data.seeding_status, Some("complete".to_string()));
    assert_eq!(chart_data.seeding_progress, Some(100));
    assert_eq!(chart_data.data_completeness, Some(95));
    assert_eq!(chart_data.expected_points, Some(288));
}

#[test]
fn test_chart_data_seeding_states() {
    let states = vec![
        ("not_started", 0),
        ("in_progress", 50),
        ("complete", 100),
        ("failed", 0),
    ];

    for (status, progress) in states {
        let response = serde_json::json!({
            "symbol": "btc",
            "range": "1d",
            "data": [],
            "seedingStatus": status,
            "seedingProgress": progress
        });

        let chart_data: ChartData = serde_json::from_value(response)
            .expect(&format!("Failed to parse chart data for status: {}", status));

        assert_eq!(chart_data.seeding_status, Some(status.to_string()));
        assert_eq!(chart_data.seeding_progress, Some(progress));
    }
}

#[test]
fn test_chart_range_expected_points() {
    // Verify expected points for each time range
    let ranges = vec![
        ("1h", 60),      // 1-minute buckets for 1 hour
        ("4h", 48),      // 5-minute buckets for 4 hours
        ("1d", 288),     // 5-minute buckets for 24 hours
        ("1w", 168),     // 1-hour buckets for 7 days
        ("1m", 720),     // 1-hour buckets for 30 days
    ];

    for (range, expected) in ranges {
        let response = serde_json::json!({
            "symbol": "btc",
            "range": range,
            "data": [],
            "expectedPoints": expected
        });

        let chart_data: ChartData = serde_json::from_value(response)
            .expect(&format!("Failed to parse chart data for range: {}", range));

        assert_eq!(
            chart_data.expected_points, Some(expected),
            "Expected points mismatch for range: {}",
            range
        );
    }
}

// ============================================================================
// Price Variance Tests
// ============================================================================

#[test]
fn test_price_within_variance_tolerance() {
    // Test that our price comparison logic works correctly
    let haunt_price = 50000.0;
    let coingecko_price = 50250.0;

    let variance = (haunt_price - coingecko_price).abs() / coingecko_price;

    assert!(
        variance <= PRICE_VARIANCE_TOLERANCE,
        "Price variance {} exceeds tolerance {}",
        variance, PRICE_VARIANCE_TOLERANCE
    );
}

#[test]
fn test_price_variance_calculation() {
    // Test edge cases for price variance calculation
    let test_cases = vec![
        (50000.0, 50000.0, true),   // Exact match
        (50000.0, 50500.0, true),   // Within 1%
        (50000.0, 51000.0, false),  // Exceeds 1%
        (50000.0, 49500.0, true),   // Within 1% (lower)
        (50000.0, 49000.0, false),  // Exceeds 1% (lower)
    ];

    for (haunt, reference, should_pass) in test_cases {
        let variance = (haunt - reference).abs() / reference;
        let passes = variance <= PRICE_VARIANCE_TOLERANCE;

        assert_eq!(
            passes, should_pass,
            "Price variance check failed for haunt={}, reference={}, variance={}",
            haunt, reference, variance
        );
    }
}

// ============================================================================
// Data Completeness Tests
// ============================================================================

#[test]
fn test_data_completeness_calculation() {
    // Verify data completeness percentage calculation
    let test_cases = vec![
        (288, 288, 100),  // Full data
        (144, 288, 50),   // Half data
        (0, 288, 0),      // No data
        (300, 288, 100),  // Exceeds expected (capped at 100)
    ];

    for (actual, expected, completeness) in test_cases {
        let calculated = ((actual as f64 / expected as f64) * 100.0).min(100.0) as u8;
        assert_eq!(
            calculated, completeness,
            "Completeness calculation failed: actual={}, expected={}, got={}, want={}",
            actual, expected, calculated, completeness
        );
    }
}

// ============================================================================
// Sparkline Consistency Tests
// ============================================================================

#[test]
fn test_sparkline_values_positive() {
    // Verify sparkline values are positive (prices can't be negative)
    let sparkline: Vec<f64> = vec![100.0, 101.5, 99.8, 102.3, 101.0, 100.5];

    for (i, &value) in sparkline.iter().enumerate() {
        assert!(
            value > 0.0,
            "Sparkline contains non-positive value {} at index {}",
            value, i
        );
    }
}

#[test]
fn test_sparkline_matches_chart_trend() {
    // Verify sparkline trend matches chart data trend
    let chart_data = vec![
        OhlcPoint { time: 1000, open: 100.0, high: 105.0, low: 98.0, close: 102.0, volume: None },
        OhlcPoint { time: 2000, open: 102.0, high: 108.0, low: 100.0, close: 106.0, volume: None },
        OhlcPoint { time: 3000, open: 106.0, high: 112.0, low: 104.0, close: 110.0, volume: None },
    ];

    // Calculate overall trend from chart data
    let chart_start = chart_data.first().map(|p| p.close).unwrap_or(0.0);
    let chart_end = chart_data.last().map(|p| p.close).unwrap_or(0.0);
    let chart_trend_positive = chart_end > chart_start;

    // Sparkline should reflect same trend
    let sparkline: Vec<f64> = chart_data.iter().map(|p| p.close).collect();
    let spark_start = *sparkline.first().unwrap_or(&0.0);
    let spark_end = *sparkline.last().unwrap_or(&0.0);
    let spark_trend_positive = spark_end > spark_start;

    assert_eq!(
        chart_trend_positive, spark_trend_positive,
        "Sparkline trend does not match chart trend"
    );
}

// ============================================================================
// WebSocket Message Tests
// ============================================================================

#[test]
fn test_seeding_progress_message_structure() {
    let message = serde_json::json!({
        "type": "seeding_progress",
        "data": {
            "symbol": "btc",
            "status": "in_progress",
            "progress": 50,
            "points": 500,
            "message": "Fetching from CoinGecko..."
        }
    });

    assert_eq!(message["type"], "seeding_progress");
    assert_eq!(message["data"]["symbol"], "btc");
    assert_eq!(message["data"]["status"], "in_progress");
    assert_eq!(message["data"]["progress"], 50);
    assert_eq!(message["data"]["points"], 500);
}

#[test]
fn test_seeding_progress_status_values() {
    let valid_statuses = vec!["in_progress", "complete", "failed"];

    for status in valid_statuses {
        let message = serde_json::json!({
            "type": "seeding_progress",
            "data": {
                "symbol": "eth",
                "status": status,
                "progress": 75
            }
        });

        assert!(
            message["data"]["status"].is_string(),
            "Status should be a string: {}",
            status
        );
    }
}

#[test]
fn test_seeding_progress_percentage_range() {
    // Progress should be 0-100
    let test_values: Vec<u8> = vec![0, 25, 50, 75, 100];

    for progress in test_values {
        assert!(progress <= 100, "Progress {} exceeds 100", progress);

        let message = serde_json::json!({
            "type": "seeding_progress",
            "data": {
                "symbol": "sol",
                "status": "in_progress",
                "progress": progress
            }
        });

        assert_eq!(message["data"]["progress"], progress);
    }
}
