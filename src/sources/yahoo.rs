//! Yahoo Finance API client for historical stock data.
//!
//! Provides historical OHLC data for stocks and ETFs.
//! Uses the unofficial Yahoo Finance API (no rate limits).

use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::debug;

/// Yahoo Finance chart response.
#[derive(Debug, Deserialize)]
struct YahooChartResponse {
    chart: YahooChart,
}

#[derive(Debug, Deserialize)]
struct YahooChart {
    result: Option<Vec<YahooResult>>,
    error: Option<YahooError>,
}

#[derive(Debug, Deserialize)]
struct YahooError {
    code: String,
    description: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct YahooResult {
    meta: YahooMeta,
    timestamp: Option<Vec<i64>>,
    indicators: YahooIndicators,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct YahooMeta {
    symbol: String,
    regular_market_price: Option<f64>,
    previous_close: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct YahooIndicators {
    quote: Vec<YahooQuote>,
}

#[derive(Debug, Deserialize)]
struct YahooQuote {
    open: Option<Vec<Option<f64>>>,
    high: Option<Vec<Option<f64>>>,
    low: Option<Vec<Option<f64>>>,
    close: Option<Vec<Option<f64>>>,
    volume: Option<Vec<Option<u64>>>,
}

/// OHLC data point from Yahoo Finance.
#[derive(Debug, Clone)]
pub struct YahooOhlcPoint {
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// Normalize symbol for Yahoo Finance API.
/// Yahoo uses hyphens instead of dots for share classes (e.g., BRK-B not BRK.B)
fn normalize_yahoo_symbol(symbol: &str) -> String {
    symbol.to_uppercase().replace('.', "-")
}

/// Yahoo Finance API client.
pub struct YahooFinanceClient {
    client: Client,
}

impl YahooFinanceClient {
    /// Create a new Yahoo Finance client.
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    /// Fetch historical daily data for a symbol.
    ///
    /// Arguments:
    /// - symbol: Stock/ETF symbol (e.g., "AAPL", "SPY")
    /// - range: Time range ("1d", "5d", "1mo", "3mo", "6mo", "1y", "2y", "5y", "10y", "ytd", "max")
    /// - interval: Data interval ("1m", "2m", "5m", "15m", "30m", "60m", "90m", "1h", "1d", "5d", "1wk", "1mo", "3mo")
    pub async fn get_historical_data(
        &self,
        symbol: &str,
        range: &str,
        interval: &str,
    ) -> Result<Vec<YahooOhlcPoint>, String> {
        let yahoo_symbol = normalize_yahoo_symbol(symbol);
        let url = format!(
            "https://query1.finance.yahoo.com/v8/finance/chart/{}?range={}&interval={}&includePrePost=false",
            yahoo_symbol,
            range,
            interval
        );

        debug!("Fetching Yahoo Finance data: {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("API error: {}", response.status()));
        }

        let data: YahooChartResponse = response
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;

        // Check for API error
        if let Some(error) = data.chart.error {
            return Err(format!(
                "Yahoo API error: {} - {}",
                error.code, error.description
            ));
        }

        // Extract results
        let results = data
            .chart
            .result
            .ok_or_else(|| "No results in response".to_string())?;

        let result = results
            .into_iter()
            .next()
            .ok_or_else(|| "Empty results array".to_string())?;

        let timestamps = result
            .timestamp
            .ok_or_else(|| "No timestamps in response".to_string())?;

        let quote = result
            .indicators
            .quote
            .into_iter()
            .next()
            .ok_or_else(|| "No quote data in response".to_string())?;

        let opens = quote.open.unwrap_or_default();
        let highs = quote.high.unwrap_or_default();
        let lows = quote.low.unwrap_or_default();
        let closes = quote.close.unwrap_or_default();
        let volumes = quote.volume.unwrap_or_default();

        // Build OHLC points
        let mut points = Vec::new();
        for (i, &timestamp) in timestamps.iter().enumerate() {
            let open = opens.get(i).and_then(|v| *v).unwrap_or(0.0);
            let high = highs.get(i).and_then(|v| *v).unwrap_or(0.0);
            let low = lows.get(i).and_then(|v| *v).unwrap_or(0.0);
            let close = closes.get(i).and_then(|v| *v).unwrap_or(0.0);
            let volume = volumes.get(i).and_then(|v| *v).unwrap_or(0) as f64;

            // Skip invalid data points
            if close <= 0.0 {
                continue;
            }

            points.push(YahooOhlcPoint {
                time: timestamp * 1000, // Convert to milliseconds
                open,
                high,
                low,
                close,
                volume,
            });
        }

        Ok(points)
    }

    /// Fetch 100 days of daily historical data (similar to Alpha Vantage compact).
    pub async fn get_daily_history(&self, symbol: &str) -> Result<Vec<YahooOhlcPoint>, String> {
        self.get_historical_data(symbol, "3mo", "1d").await
    }

    /// Fetch 1 year of daily historical data.
    #[allow(dead_code)]
    pub async fn get_yearly_history(&self, symbol: &str) -> Result<Vec<YahooOhlcPoint>, String> {
        self.get_historical_data(symbol, "1y", "1d").await
    }
}

impl Default for YahooFinanceClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // normalize_yahoo_symbol Tests
    // =========================================================================

    #[test]
    fn test_normalize_yahoo_symbol_uppercase() {
        assert_eq!(normalize_yahoo_symbol("aapl"), "AAPL");
        assert_eq!(normalize_yahoo_symbol("msft"), "MSFT");
    }

    #[test]
    fn test_normalize_yahoo_symbol_dots_to_hyphens() {
        assert_eq!(normalize_yahoo_symbol("BRK.B"), "BRK-B");
        assert_eq!(normalize_yahoo_symbol("brk.a"), "BRK-A");
    }

    #[test]
    fn test_normalize_yahoo_symbol_already_normalized() {
        assert_eq!(normalize_yahoo_symbol("AAPL"), "AAPL");
        assert_eq!(normalize_yahoo_symbol("BRK-B"), "BRK-B");
    }

    // =========================================================================
    // YahooOhlcPoint Tests
    // =========================================================================

    #[test]
    fn test_yahoo_ohlc_point_creation() {
        let point = YahooOhlcPoint {
            time: 1700000000000,
            open: 150.0,
            high: 155.0,
            low: 148.0,
            close: 153.0,
            volume: 50000000.0,
        };
        assert_eq!(point.time, 1700000000000);
        assert_eq!(point.open, 150.0);
        assert_eq!(point.high, 155.0);
        assert_eq!(point.low, 148.0);
        assert_eq!(point.close, 153.0);
        assert_eq!(point.volume, 50000000.0);
    }

    #[test]
    fn test_yahoo_ohlc_point_clone() {
        let point = YahooOhlcPoint {
            time: 1700000000000,
            open: 150.0,
            high: 155.0,
            low: 148.0,
            close: 153.0,
            volume: 50000000.0,
        };
        let cloned = point.clone();
        assert_eq!(cloned.close, 153.0);
    }

    #[test]
    fn test_yahoo_ohlc_point_debug() {
        let point = YahooOhlcPoint {
            time: 1700000000000,
            open: 150.0,
            high: 155.0,
            low: 148.0,
            close: 153.0,
            volume: 50000000.0,
        };
        let debug_str = format!("{:?}", point);
        assert!(debug_str.contains("YahooOhlcPoint"));
        assert!(debug_str.contains("153"));
    }

    // =========================================================================
    // YahooError Tests
    // =========================================================================

    #[test]
    fn test_yahoo_error_deserialization() {
        let json = r#"{
            "code": "Not Found",
            "description": "Symbol not found"
        }"#;
        let error: YahooError = serde_json::from_str(json).unwrap();
        assert_eq!(error.code, "Not Found");
        assert_eq!(error.description, "Symbol not found");
    }

    // =========================================================================
    // YahooMeta Tests
    // =========================================================================

    #[test]
    fn test_yahoo_meta_deserialization() {
        let json = r#"{
            "symbol": "AAPL",
            "regularMarketPrice": 153.25,
            "previousClose": 151.50
        }"#;
        let meta: YahooMeta = serde_json::from_str(json).unwrap();
        assert_eq!(meta.symbol, "AAPL");
        assert_eq!(meta.regular_market_price, Some(153.25));
        assert_eq!(meta.previous_close, Some(151.50));
    }

    #[test]
    fn test_yahoo_meta_minimal() {
        let json = r#"{"symbol": "MSFT"}"#;
        let meta: YahooMeta = serde_json::from_str(json).unwrap();
        assert_eq!(meta.symbol, "MSFT");
        assert!(meta.regular_market_price.is_none());
        assert!(meta.previous_close.is_none());
    }

    // =========================================================================
    // YahooQuote Tests
    // =========================================================================

    #[test]
    fn test_yahoo_quote_deserialization() {
        let json = r#"{
            "open": [150.0, 151.0, 152.0],
            "high": [155.0, 156.0, 157.0],
            "low": [148.0, 149.0, 150.0],
            "close": [153.0, 154.0, 155.0],
            "volume": [50000000, 51000000, 52000000]
        }"#;
        let quote: YahooQuote = serde_json::from_str(json).unwrap();
        assert!(quote.open.is_some());
        assert_eq!(quote.close.unwrap().len(), 3);
    }

    #[test]
    fn test_yahoo_quote_with_nulls() {
        let json = r#"{
            "open": [150.0, null, 152.0],
            "close": [153.0, null, 155.0]
        }"#;
        let quote: YahooQuote = serde_json::from_str(json).unwrap();
        let opens = quote.open.unwrap();
        assert_eq!(opens[0], Some(150.0));
        assert_eq!(opens[1], None);
        assert_eq!(opens[2], Some(152.0));
    }

    // =========================================================================
    // YahooIndicators Tests
    // =========================================================================

    #[test]
    fn test_yahoo_indicators_deserialization() {
        let json = r#"{
            "quote": [{
                "open": [150.0],
                "close": [153.0]
            }]
        }"#;
        let indicators: YahooIndicators = serde_json::from_str(json).unwrap();
        assert_eq!(indicators.quote.len(), 1);
    }

    // =========================================================================
    // YahooChart Tests
    // =========================================================================

    #[test]
    fn test_yahoo_chart_with_error() {
        let json = r#"{
            "result": null,
            "error": {
                "code": "Not Found",
                "description": "No data"
            }
        }"#;
        let chart: YahooChart = serde_json::from_str(json).unwrap();
        assert!(chart.result.is_none());
        assert!(chart.error.is_some());
        assert_eq!(chart.error.unwrap().code, "Not Found");
    }

    // =========================================================================
    // YahooFinanceClient Tests
    // =========================================================================

    #[test]
    fn test_yahoo_finance_client_creation() {
        let _client = YahooFinanceClient::new();
        // Test passes if no panic occurs
    }

    #[test]
    fn test_yahoo_finance_client_default() {
        let _client = YahooFinanceClient::default();
        // Test passes if no panic occurs
    }
}
