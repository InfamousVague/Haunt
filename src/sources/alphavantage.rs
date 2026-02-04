//! Alpha Vantage API client for historical stock data.
//!
//! Provides historical OHLC data and company fundamentals for stocks and ETFs.
//! Note: Free tier has very limited rate limits (25 requests/day, 5/minute).

// Some structs/methods are kept for API completeness
#![allow(dead_code)]

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[allow(unused_imports)]
use tracing::debug;

const ALPHA_VANTAGE_URL: &str = "https://www.alphavantage.co/query";

/// Time series data point from Alpha Vantage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OhlcPoint {
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// Alpha Vantage global quote response.
#[derive(Debug, Clone, Deserialize)]
pub struct GlobalQuoteResponse {
    #[serde(rename = "Global Quote")]
    pub global_quote: Option<GlobalQuote>,
}

/// Global quote data.
#[derive(Debug, Clone, Deserialize)]
pub struct GlobalQuote {
    #[serde(rename = "01. symbol")]
    pub symbol: String,
    #[serde(rename = "02. open")]
    pub open: String,
    #[serde(rename = "03. high")]
    pub high: String,
    #[serde(rename = "04. low")]
    pub low: String,
    #[serde(rename = "05. price")]
    pub price: String,
    #[serde(rename = "06. volume")]
    pub volume: String,
    #[serde(rename = "07. latest trading day")]
    pub latest_trading_day: String,
    #[serde(rename = "08. previous close")]
    pub previous_close: String,
    #[serde(rename = "09. change")]
    pub change: String,
    #[serde(rename = "10. change percent")]
    pub change_percent: String,
}

/// Time series daily response.
#[derive(Debug, Clone, Deserialize)]
pub struct TimeSeriesDailyResponse {
    #[serde(rename = "Meta Data")]
    pub meta_data: Option<TimeSeriesMetaData>,
    #[serde(rename = "Time Series (Daily)")]
    pub time_series: Option<HashMap<String, TimeSeriesDataPoint>>,
}

/// Time series intraday response.
#[derive(Debug, Clone, Deserialize)]
pub struct TimeSeriesIntradayResponse {
    #[serde(rename = "Meta Data")]
    pub meta_data: Option<TimeSeriesMetaData>,
    #[serde(flatten)]
    pub time_series: HashMap<String, HashMap<String, TimeSeriesDataPoint>>,
}

/// Time series meta data.
#[derive(Debug, Clone, Deserialize)]
pub struct TimeSeriesMetaData {
    #[serde(rename = "1. Information")]
    pub information: Option<String>,
    #[serde(rename = "2. Symbol")]
    pub symbol: Option<String>,
    #[serde(rename = "3. Last Refreshed")]
    pub last_refreshed: Option<String>,
}

/// Individual time series data point.
#[derive(Debug, Clone, Deserialize)]
pub struct TimeSeriesDataPoint {
    #[serde(rename = "1. open")]
    pub open: String,
    #[serde(rename = "2. high")]
    pub high: String,
    #[serde(rename = "3. low")]
    pub low: String,
    #[serde(rename = "4. close")]
    pub close: String,
    #[serde(rename = "5. volume")]
    pub volume: String,
}

/// Company overview data.
#[derive(Debug, Clone, Deserialize)]
pub struct CompanyOverview {
    #[serde(rename = "Symbol")]
    pub symbol: Option<String>,
    #[serde(rename = "AssetType")]
    pub asset_type: Option<String>,
    #[serde(rename = "Name")]
    pub name: Option<String>,
    #[serde(rename = "Description")]
    pub description: Option<String>,
    #[serde(rename = "Exchange")]
    pub exchange: Option<String>,
    #[serde(rename = "Currency")]
    pub currency: Option<String>,
    #[serde(rename = "Country")]
    pub country: Option<String>,
    #[serde(rename = "Sector")]
    pub sector: Option<String>,
    #[serde(rename = "Industry")]
    pub industry: Option<String>,
    #[serde(rename = "MarketCapitalization")]
    pub market_cap: Option<String>,
    #[serde(rename = "52WeekHigh")]
    pub week_52_high: Option<String>,
    #[serde(rename = "52WeekLow")]
    pub week_52_low: Option<String>,
    #[serde(rename = "DividendYield")]
    pub dividend_yield: Option<String>,
}

/// Alpha Vantage API client.
pub struct AlphaVantageClient {
    client: Client,
    api_key: String,
}

impl AlphaVantageClient {
    /// Create a new Alpha Vantage client.
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
        }
    }

    /// Get global quote for a symbol.
    pub async fn get_quote(&self, symbol: &str) -> Result<GlobalQuote, String> {
        let url = format!(
            "{}?function=GLOBAL_QUOTE&symbol={}&apikey={}",
            ALPHA_VANTAGE_URL, symbol, self.api_key
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("API error: {}", response.status()));
        }

        let data: GlobalQuoteResponse = response
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;

        data.global_quote
            .ok_or_else(|| "No quote data available".to_string())
    }

    /// Get daily time series data.
    pub async fn get_daily_time_series(
        &self,
        symbol: &str,
        output_size: &str, // "compact" (100 days) or "full" (20+ years)
    ) -> Result<Vec<OhlcPoint>, String> {
        let url = format!(
            "{}?function=TIME_SERIES_DAILY&symbol={}&outputsize={}&apikey={}",
            ALPHA_VANTAGE_URL, symbol, output_size, self.api_key
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("API error: {}", response.status()));
        }

        let data: TimeSeriesDailyResponse = response
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;

        let time_series = data
            .time_series
            .ok_or_else(|| "No time series data available".to_string())?;

        let mut points: Vec<OhlcPoint> = time_series
            .into_iter()
            .filter_map(|(date_str, point)| {
                let date = chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").ok()?;
                let timestamp = date.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis();

                Some(OhlcPoint {
                    time: timestamp,
                    open: point.open.parse().unwrap_or(0.0),
                    high: point.high.parse().unwrap_or(0.0),
                    low: point.low.parse().unwrap_or(0.0),
                    close: point.close.parse().unwrap_or(0.0),
                    volume: point.volume.parse().unwrap_or(0.0),
                })
            })
            .collect();

        // Sort by time ascending
        points.sort_by_key(|p| p.time);

        Ok(points)
    }

    /// Get company overview.
    pub async fn get_company_overview(&self, symbol: &str) -> Result<CompanyOverview, String> {
        let url = format!(
            "{}?function=OVERVIEW&symbol={}&apikey={}",
            ALPHA_VANTAGE_URL, symbol, self.api_key
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("API error: {}", response.status()));
        }

        response
            .json::<CompanyOverview>()
            .await
            .map_err(|e| format!("Parse error: {}", e))
    }

    /// Parse change percent string (e.g., "1.23%" -> 1.23).
    pub fn parse_change_percent(s: &str) -> f64 {
        s.trim_end_matches('%').parse().unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // parse_change_percent Tests
    // =========================================================================

    #[test]
    fn test_parse_change_percent_with_percent_sign() {
        assert_eq!(AlphaVantageClient::parse_change_percent("1.23%"), 1.23);
        assert_eq!(AlphaVantageClient::parse_change_percent("-2.50%"), -2.50);
    }

    #[test]
    fn test_parse_change_percent_without_percent_sign() {
        assert_eq!(AlphaVantageClient::parse_change_percent("1.23"), 1.23);
        assert_eq!(AlphaVantageClient::parse_change_percent("-0.75"), -0.75);
    }

    #[test]
    fn test_parse_change_percent_invalid() {
        assert_eq!(AlphaVantageClient::parse_change_percent("invalid"), 0.0);
        assert_eq!(AlphaVantageClient::parse_change_percent(""), 0.0);
    }

    // =========================================================================
    // OhlcPoint Tests
    // =========================================================================

    #[test]
    fn test_ohlc_point_creation() {
        let point = OhlcPoint {
            time: 1700000000000,
            open: 150.0,
            high: 155.0,
            low: 148.0,
            close: 153.0,
            volume: 50000000.0,
        };
        assert_eq!(point.time, 1700000000000);
        assert_eq!(point.close, 153.0);
    }

    #[test]
    fn test_ohlc_point_serialization() {
        let point = OhlcPoint {
            time: 1700000000000,
            open: 150.0,
            high: 155.0,
            low: 148.0,
            close: 153.0,
            volume: 50000000.0,
        };
        let json = serde_json::to_string(&point).unwrap();
        assert!(json.contains("1700000000000"));
        assert!(json.contains("153"));
    }

    #[test]
    fn test_ohlc_point_deserialization() {
        let json = r#"{
            "time": 1700000000000,
            "open": 150.0,
            "high": 155.0,
            "low": 148.0,
            "close": 153.0,
            "volume": 50000000.0
        }"#;
        let point: OhlcPoint = serde_json::from_str(json).unwrap();
        assert_eq!(point.time, 1700000000000);
        assert_eq!(point.close, 153.0);
    }

    #[test]
    fn test_ohlc_point_clone() {
        let point = OhlcPoint {
            time: 1700000000000,
            open: 150.0,
            high: 155.0,
            low: 148.0,
            close: 153.0,
            volume: 50000000.0,
        };
        let cloned = point.clone();
        assert_eq!(cloned.time, point.time);
    }

    // =========================================================================
    // GlobalQuote Tests
    // =========================================================================

    #[test]
    fn test_global_quote_deserialization() {
        let json = r#"{
            "01. symbol": "AAPL",
            "02. open": "150.00",
            "03. high": "155.00",
            "04. low": "148.00",
            "05. price": "153.25",
            "06. volume": "50000000",
            "07. latest trading day": "2024-01-15",
            "08. previous close": "151.50",
            "09. change": "1.75",
            "10. change percent": "1.15%"
        }"#;
        let quote: GlobalQuote = serde_json::from_str(json).unwrap();
        assert_eq!(quote.symbol, "AAPL");
        assert_eq!(quote.price, "153.25");
        assert_eq!(quote.change_percent, "1.15%");
    }

    // =========================================================================
    // GlobalQuoteResponse Tests
    // =========================================================================

    #[test]
    fn test_global_quote_response_with_data() {
        let json = r#"{
            "Global Quote": {
                "01. symbol": "MSFT",
                "02. open": "380.00",
                "03. high": "385.00",
                "04. low": "378.00",
                "05. price": "383.50",
                "06. volume": "25000000",
                "07. latest trading day": "2024-01-15",
                "08. previous close": "379.00",
                "09. change": "4.50",
                "10. change percent": "1.19%"
            }
        }"#;
        let response: GlobalQuoteResponse = serde_json::from_str(json).unwrap();
        assert!(response.global_quote.is_some());
        assert_eq!(response.global_quote.unwrap().symbol, "MSFT");
    }

    #[test]
    fn test_global_quote_response_empty() {
        let json = r#"{"Global Quote": null}"#;
        let response: GlobalQuoteResponse = serde_json::from_str(json).unwrap();
        assert!(response.global_quote.is_none());
    }

    // =========================================================================
    // TimeSeriesDataPoint Tests
    // =========================================================================

    #[test]
    fn test_time_series_data_point_deserialization() {
        let json = r#"{
            "1. open": "150.00",
            "2. high": "155.00",
            "3. low": "148.00",
            "4. close": "153.00",
            "5. volume": "50000000"
        }"#;
        let point: TimeSeriesDataPoint = serde_json::from_str(json).unwrap();
        assert_eq!(point.open, "150.00");
        assert_eq!(point.close, "153.00");
        assert_eq!(point.volume, "50000000");
    }

    // =========================================================================
    // TimeSeriesMetaData Tests
    // =========================================================================

    #[test]
    fn test_time_series_meta_data_deserialization() {
        let json = r#"{
            "1. Information": "Daily Time Series",
            "2. Symbol": "AAPL",
            "3. Last Refreshed": "2024-01-15"
        }"#;
        let meta: TimeSeriesMetaData = serde_json::from_str(json).unwrap();
        assert_eq!(meta.symbol, Some("AAPL".to_string()));
        assert_eq!(meta.last_refreshed, Some("2024-01-15".to_string()));
    }

    #[test]
    fn test_time_series_meta_data_partial() {
        let json = r#"{
            "1. Information": "Daily Time Series"
        }"#;
        let meta: TimeSeriesMetaData = serde_json::from_str(json).unwrap();
        assert!(meta.symbol.is_none());
        assert!(meta.last_refreshed.is_none());
    }

    // =========================================================================
    // TimeSeriesDailyResponse Tests
    // =========================================================================

    #[test]
    fn test_time_series_daily_response_deserialization() {
        let json = r#"{
            "Meta Data": {
                "1. Information": "Daily Prices",
                "2. Symbol": "AAPL",
                "3. Last Refreshed": "2024-01-15"
            },
            "Time Series (Daily)": {
                "2024-01-15": {
                    "1. open": "150.00",
                    "2. high": "155.00",
                    "3. low": "148.00",
                    "4. close": "153.00",
                    "5. volume": "50000000"
                }
            }
        }"#;
        let response: TimeSeriesDailyResponse = serde_json::from_str(json).unwrap();
        assert!(response.meta_data.is_some());
        assert!(response.time_series.is_some());
        let series = response.time_series.unwrap();
        assert!(series.contains_key("2024-01-15"));
    }

    // =========================================================================
    // CompanyOverview Tests
    // =========================================================================

    #[test]
    fn test_company_overview_deserialization() {
        let json = r#"{
            "Symbol": "AAPL",
            "AssetType": "Common Stock",
            "Name": "Apple Inc",
            "Description": "Technology company",
            "Exchange": "NASDAQ",
            "Currency": "USD",
            "Country": "USA",
            "Sector": "Technology",
            "Industry": "Consumer Electronics",
            "MarketCapitalization": "2500000000000",
            "52WeekHigh": "200.00",
            "52WeekLow": "130.00",
            "DividendYield": "0.005"
        }"#;
        let overview: CompanyOverview = serde_json::from_str(json).unwrap();
        assert_eq!(overview.symbol, Some("AAPL".to_string()));
        assert_eq!(overview.name, Some("Apple Inc".to_string()));
        assert_eq!(overview.sector, Some("Technology".to_string()));
        assert_eq!(overview.week_52_high, Some("200.00".to_string()));
    }

    #[test]
    fn test_company_overview_minimal() {
        let json = r#"{}"#;
        let overview: CompanyOverview = serde_json::from_str(json).unwrap();
        assert!(overview.symbol.is_none());
        assert!(overview.name.is_none());
    }

    // =========================================================================
    // AlphaVantageClient Tests
    // =========================================================================

    #[test]
    fn test_alpha_vantage_client_creation() {
        let client = AlphaVantageClient::new("test_api_key".to_string());
        assert_eq!(client.api_key, "test_api_key");
    }
}
