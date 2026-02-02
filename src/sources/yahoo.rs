//! Yahoo Finance API client for historical stock data.
//!
//! Provides historical OHLC data for stocks and ETFs.
//! Uses the unofficial Yahoo Finance API (no rate limits).

use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::{debug, warn};

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
struct YahooResult {
    meta: YahooMeta,
    timestamp: Option<Vec<i64>>,
    indicators: YahooIndicators,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
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
            return Err(format!("Yahoo API error: {} - {}", error.code, error.description));
        }

        // Extract results
        let results = data.chart.result
            .ok_or_else(|| "No results in response".to_string())?;

        let result = results.into_iter().next()
            .ok_or_else(|| "Empty results array".to_string())?;

        let timestamps = result.timestamp
            .ok_or_else(|| "No timestamps in response".to_string())?;

        let quote = result.indicators.quote.into_iter().next()
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
    pub async fn get_yearly_history(&self, symbol: &str) -> Result<Vec<YahooOhlcPoint>, String> {
        self.get_historical_data(symbol, "1y", "1d").await
    }
}

impl Default for YahooFinanceClient {
    fn default() -> Self {
        Self::new()
    }
}
