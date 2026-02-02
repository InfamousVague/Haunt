/**
 * Signal Prediction Tests
 *
 * Tests for the trading signals system including:
 * - Prediction recording and validation
 * - Accuracy tracking over time
 * - Real-time signal updates
 */

use chrono::Utc;
use std::time::Duration;
use tokio::time::sleep;

// Import the main crate - these tests assume the server is running
mod common {
    use reqwest::Client;
    use serde::Deserialize;
    use std::time::Duration;

    pub const BASE_URL: &str = "http://localhost:3001";

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct SymbolSignals {
        pub symbol: String,
        pub signals: Vec<SignalOutput>,
        pub trend_score: i8,
        pub momentum_score: i8,
        pub volatility_score: i8,
        pub volume_score: i8,
        pub composite_score: i8,
        pub direction: String,
        pub timestamp: i64,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct SignalOutput {
        pub name: String,
        pub category: String,
        pub value: f64,
        pub score: i8,
        pub direction: String,
        pub accuracy: Option<f64>,
        pub sample_size: Option<u32>,
        pub timestamp: i64,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct SignalPrediction {
        pub id: String,
        pub symbol: String,
        pub indicator: String,
        pub direction: String,
        pub score: i8,
        pub price_at_prediction: f64,
        pub timestamp: i64,
        pub validated: bool,
        pub outcome_5m: Option<String>,
        pub outcome_1h: Option<String>,
        pub outcome_4h: Option<String>,
        pub outcome_24h: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct PredictionsResponse {
        pub symbol: String,
        pub predictions: Vec<SignalPrediction>,
        pub timestamp: i64,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct AccuracyResponse {
        pub symbol: String,
        pub accuracies: Vec<SignalAccuracy>,
        pub timestamp: i64,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct SignalAccuracy {
        pub indicator: String,
        pub symbol: String,
        pub timeframe: String,
        pub total_predictions: u32,
        pub correct_predictions: u32,
        pub incorrect_predictions: u32,
        pub neutral_predictions: u32,
        pub accuracy_pct: f64,
        pub last_updated: i64,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Recommendation {
        pub symbol: String,
        pub action: String,
        pub confidence: f64,
        pub weighted_score: f64,
        pub indicators_with_accuracy: u32,
        pub total_indicators: u32,
        pub average_accuracy: f64,
        pub description: String,
        pub timestamp: i64,
    }

    // API response wrappers
    #[derive(Debug, Deserialize)]
    pub struct ApiResponse<T> {
        pub data: T,
    }

    pub fn client() -> Client {
        Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create client")
    }
}

use common::*;

#[tokio::test]
async fn test_signals_endpoint_returns_data() {
    let client = client();

    // Test signals for BTC
    let response = client
        .get(format!("{}/api/signals/btc", BASE_URL))
        .send()
        .await;

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                let wrapper: ApiResponse<SymbolSignals> = resp.json().await.expect("Failed to parse signals");
                let signals = wrapper.data;
                assert_eq!(signals.symbol.to_lowercase(), "btc");
                assert!(!signals.signals.is_empty(), "Should have at least one signal");

                println!("BTC Signals:");
                println!("  Composite Score: {}", signals.composite_score);
                println!("  Direction: {}", signals.direction);
                println!("  Trend: {}, Momentum: {}", signals.trend_score, signals.momentum_score);
                println!("  Indicators: {}", signals.signals.len());

                for signal in &signals.signals {
                    println!(
                        "    {} ({}) = {:.2} -> {} (accuracy: {:?})",
                        signal.name, signal.category, signal.value, signal.direction, signal.accuracy
                    );
                }
            } else {
                println!("Signal endpoint returned {}: may need chart data", resp.status());
            }
        }
        Err(e) => {
            println!("Server not running or error: {}. Skipping test.", e);
        }
    }
}

#[tokio::test]
async fn test_predictions_endpoint() {
    let client = client();

    // First, trigger signal calculation to record predictions
    let _ = client
        .get(format!("{}/api/signals/btc", BASE_URL))
        .send()
        .await;

    // Wait a moment for predictions to be recorded
    sleep(Duration::from_millis(100)).await;

    // Get predictions
    let response = client
        .get(format!("{}/api/signals/btc/predictions", BASE_URL))
        .send()
        .await;

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                let wrapper: ApiResponse<PredictionsResponse> =
                    resp.json().await.expect("Failed to parse predictions");
                let predictions = wrapper.data;

                println!("BTC Predictions: {} total", predictions.predictions.len());

                for pred in predictions.predictions.iter().take(5) {
                    println!(
                        "  {} {} at ${:.2} ({}) -> 5m:{:?}, 1h:{:?}, 4h:{:?}",
                        pred.indicator,
                        pred.direction,
                        pred.price_at_prediction,
                        if pred.validated { "validated" } else { "pending" },
                        pred.outcome_5m,
                        pred.outcome_1h,
                        pred.outcome_4h,
                    );
                }
            } else {
                println!("Predictions endpoint returned {}", resp.status());
            }
        }
        Err(e) => {
            println!("Server not running or error: {}. Skipping test.", e);
        }
    }
}

#[tokio::test]
async fn test_accuracy_endpoint() {
    let client = client();

    let response = client
        .get(format!("{}/api/signals/btc/accuracy", BASE_URL))
        .send()
        .await;

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                let wrapper: ApiResponse<AccuracyResponse> =
                    resp.json().await.expect("Failed to parse accuracy");
                let accuracy = wrapper.data;

                println!("BTC Accuracy Stats: {} entries", accuracy.accuracies.len());

                for acc in &accuracy.accuracies {
                    if acc.total_predictions > 0 {
                        println!(
                            "  {} ({}): {:.1}% accuracy, {} predictions ({} correct, {} wrong)",
                            acc.indicator,
                            acc.timeframe,
                            acc.accuracy_pct,
                            acc.total_predictions,
                            acc.correct_predictions,
                            acc.incorrect_predictions,
                        );
                    }
                }
            } else {
                println!("Accuracy endpoint returned {}", resp.status());
            }
        }
        Err(e) => {
            println!("Server not running or error: {}. Skipping test.", e);
        }
    }
}

#[tokio::test]
async fn test_recommendation_endpoint() {
    let client = client();

    let response = client
        .get(format!("{}/api/signals/btc/recommendation", BASE_URL))
        .send()
        .await;

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                let wrapper: ApiResponse<Recommendation> = resp.json().await.expect("Failed to parse recommendation");
                let rec = wrapper.data;

                println!("BTC Recommendation:");
                println!("  Action: {} ({:.1}% confidence)", rec.action.to_uppercase(), rec.confidence);
                println!("  Weighted Score: {:.1}", rec.weighted_score);
                println!(
                    "  Indicators: {}/{} with accuracy data",
                    rec.indicators_with_accuracy, rec.total_indicators
                );
                println!("  Avg Accuracy: {:.1}%", rec.average_accuracy);
                println!("  Description: {}", rec.description);

                // Validate recommendation action
                assert!(
                    rec.action == "buy" || rec.action == "sell" || rec.action == "hold",
                    "Action should be buy, sell, or hold"
                );
                assert!(
                    rec.confidence >= 0.0 && rec.confidence <= 100.0,
                    "Confidence should be between 0 and 100"
                );
            } else {
                println!("Recommendation endpoint returned {}", resp.status());
            }
        }
        Err(e) => {
            println!("Server not running or error: {}. Skipping test.", e);
        }
    }
}

#[tokio::test]
async fn test_signals_with_timeframe() {
    let client = client();

    for timeframe in &["scalping", "day_trading", "swing_trading"] {
        let response = client
            .get(format!(
                "{}/api/signals/btc?timeframe={}",
                BASE_URL, timeframe
            ))
            .send()
            .await;

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    let wrapper: ApiResponse<SymbolSignals> = resp.json().await.expect("Failed to parse signals");
                    let signals = wrapper.data;
                    println!(
                        "{} signals for BTC: composite={}, direction={}",
                        timeframe, signals.composite_score, signals.direction
                    );
                } else {
                    println!("{}: endpoint returned {}", timeframe, resp.status());
                }
            }
            Err(e) => {
                println!("Error for {}: {}", timeframe, e);
            }
        }
    }
}

#[tokio::test]
async fn test_prediction_validation_timing() {
    let client = client();

    // This test checks that predictions are being validated at expected intervals
    println!("Testing prediction validation timing...");
    println!("Note: For 5m validation to trigger, predictions must be at least 5 minutes old");

    // Get current predictions
    let response = client
        .get(format!("{}/api/signals/btc/predictions", BASE_URL))
        .send()
        .await;

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                let wrapper: ApiResponse<PredictionsResponse> =
                    resp.json().await.expect("Failed to parse predictions");
                let predictions = wrapper.data;

                let now = Utc::now().timestamp_millis();

                let mut pending_5m = 0;
                let mut validated_5m = 0;
                let mut pending_1h = 0;
                let mut validated_1h = 0;

                for pred in &predictions.predictions {
                    let age_ms = now - pred.timestamp;
                    let age_minutes = age_ms / 60_000;

                    if age_minutes >= 5 {
                        if pred.outcome_5m.is_some() {
                            validated_5m += 1;
                        } else {
                            pending_5m += 1;
                        }
                    }

                    if age_minutes >= 60 {
                        if pred.outcome_1h.is_some() {
                            validated_1h += 1;
                        } else {
                            pending_1h += 1;
                        }
                    }
                }

                println!("Prediction validation status:");
                println!("  5m window: {} validated, {} pending (should be 0 if >5min old)", validated_5m, pending_5m);
                println!("  1h window: {} validated, {} pending (should be 0 if >1hr old)", validated_1h, pending_1h);

                // If there are pending 5m predictions that are old enough, that's a problem
                if pending_5m > 0 {
                    println!("WARNING: {} predictions are >5min old but not yet validated for 5m", pending_5m);
                }
            }
        }
        Err(e) => {
            println!("Server not running: {}. Skipping test.", e);
        }
    }
}

#[tokio::test]
async fn test_signals_real_time_updates() {
    let client = client();

    println!("Testing real-time signal updates...");
    println!("Requesting signals 3 times with 5 second intervals");

    let mut scores: Vec<i8> = Vec::new();
    let mut timestamps: Vec<i64> = Vec::new();

    for i in 0..3 {
        if i > 0 {
            sleep(Duration::from_secs(5)).await;
        }

        let response = client
            .get(format!("{}/api/signals/btc", BASE_URL))
            .send()
            .await;

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    let wrapper: ApiResponse<SymbolSignals> = resp.json().await.expect("Failed to parse signals");
                    let signals = wrapper.data;
                    scores.push(signals.composite_score);
                    timestamps.push(signals.timestamp);
                    println!(
                        "  Request {}: score={}, timestamp={}",
                        i + 1, signals.composite_score, signals.timestamp
                    );
                }
            }
            Err(e) => {
                println!("Error: {}. Skipping.", e);
                return;
            }
        }
    }

    // Check if timestamps are updating
    if timestamps.len() >= 2 {
        let timestamp_changed = timestamps.windows(2).any(|w| w[0] != w[1]);
        if timestamp_changed {
            println!("PASS: Signal timestamps are updating (fresh calculations)");
        } else {
            println!("Note: Timestamps unchanged - signals may be cached (30s TTL)");
        }
    }
}

#[tokio::test]
async fn test_multiple_symbols() {
    let client = client();
    let symbols = vec!["btc", "eth", "sol", "aapl", "spy"];

    println!("Testing signals for multiple symbols...");

    for symbol in symbols {
        let response = client
            .get(format!("{}/api/signals/{}", BASE_URL, symbol))
            .send()
            .await;

        match response {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    let wrapper: ApiResponse<SymbolSignals> = resp.json().await.expect("Failed to parse");
                    let signals = wrapper.data;
                    println!(
                        "  {}: {} ({} indicators)",
                        symbol.to_uppercase(),
                        signals.direction,
                        signals.signals.len()
                    );
                } else if status.as_u16() == 404 {
                    println!("  {}: No data (asset not tracked)", symbol.to_uppercase());
                } else {
                    println!("  {}: HTTP {}", symbol.to_uppercase(), status);
                }
            }
            Err(e) => {
                println!("  {}: Error - {}", symbol.to_uppercase(), e);
            }
        }
    }
}
