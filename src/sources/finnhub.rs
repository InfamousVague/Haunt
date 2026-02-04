//! Finnhub API client for stock and ETF data.
//!
//! Provides real-time quotes, company profiles, and search functionality
//! for US stocks and ETFs.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

const FINNHUB_URL: &str = "https://finnhub.io/api/v1";
const POLL_INTERVAL_SECS: u64 = 30;

/// Top stocks to track by market cap.
pub const STOCK_SYMBOLS: &[&str] = &[
    "AAPL", "MSFT", "GOOGL", "AMZN", "NVDA", "TSLA", "META", "BRK.B", "JPM", "V", "JNJ", "UNH",
    "HD", "PG", "MA", "DIS", "ADBE", "CRM", "NFLX", "PYPL",
];

/// Top ETFs to track.
pub const ETF_SYMBOLS: &[&str] = &[
    "SPY", "QQQ", "VOO", "IWM", "DIA", "VTI", "ARKK", "XLF", "XLE", "GLD", "VGT", "SCHD", "VYM",
    "JEPI", "BND",
];

/// Finnhub quote response.
#[derive(Debug, Clone, Deserialize)]
pub struct FinnhubQuote {
    /// Current price
    #[serde(rename = "c")]
    pub current: f64,
    /// Change
    #[serde(rename = "d")]
    pub change: Option<f64>,
    /// Percent change
    #[serde(rename = "dp")]
    pub change_percent: Option<f64>,
    /// High price of the day
    #[serde(rename = "h")]
    pub high: f64,
    /// Low price of the day
    #[serde(rename = "l")]
    pub low: f64,
    /// Open price of the day
    #[serde(rename = "o")]
    pub open: f64,
    /// Previous close price
    #[serde(rename = "pc")]
    pub previous_close: f64,
    /// Timestamp
    #[serde(rename = "t")]
    pub timestamp: i64,
}

/// Finnhub company profile.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinnhubProfile {
    pub country: Option<String>,
    pub currency: Option<String>,
    pub exchange: Option<String>,
    #[serde(rename = "finnhubIndustry")]
    pub industry: Option<String>,
    pub ipo: Option<String>,
    pub logo: Option<String>,
    pub market_capitalization: Option<f64>,
    pub name: Option<String>,
    pub phone: Option<String>,
    pub share_outstanding: Option<f64>,
    pub ticker: Option<String>,
    pub weburl: Option<String>,
}

/// Finnhub search result.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct FinnhubSearchResult {
    pub count: i32,
    pub result: Vec<FinnhubSymbol>,
}

/// Finnhub symbol from search.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinnhubSymbol {
    pub description: String,
    pub display_symbol: String,
    pub symbol: String,
    #[serde(rename = "type")]
    pub symbol_type: String,
}

/// Cached stock/ETF data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockData {
    pub symbol: String,
    pub name: String,
    pub price: f64,
    pub change_24h: f64,
    pub market_cap: f64,
    pub volume_24h: f64,
    pub exchange: Option<String>,
    pub sector: Option<String>,
    pub logo: Option<String>,
    pub asset_type: String, // "stock" or "etf"
    pub timestamp: i64,
}

/// Finnhub API client.
pub struct FinnhubClient {
    client: Client,
    api_key: String,
    /// Cached stock data
    stock_cache: Arc<RwLock<HashMap<String, StockData>>>,
    /// Cached ETF data
    etf_cache: Arc<RwLock<HashMap<String, StockData>>>,
}

impl FinnhubClient {
    /// Create a new Finnhub client.
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            stock_cache: Arc::new(RwLock::new(HashMap::new())),
            etf_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get a quote for a symbol.
    pub async fn get_quote(&self, symbol: &str) -> Result<FinnhubQuote, String> {
        let url = format!(
            "{}/quote?symbol={}&token={}",
            FINNHUB_URL, symbol, self.api_key
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
            .json::<FinnhubQuote>()
            .await
            .map_err(|e| format!("Parse error: {}", e))
    }

    /// Get company profile.
    pub async fn get_profile(&self, symbol: &str) -> Result<FinnhubProfile, String> {
        let url = format!(
            "{}/stock/profile2?symbol={}&token={}",
            FINNHUB_URL, symbol, self.api_key
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
            .json::<FinnhubProfile>()
            .await
            .map_err(|e| format!("Parse error: {}", e))
    }

    /// Search for symbols.
    pub async fn search(&self, query: &str) -> Result<Vec<FinnhubSymbol>, String> {
        let url = format!("{}/search?q={}&token={}", FINNHUB_URL, query, self.api_key);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("API error: {}", response.status()));
        }

        let result: FinnhubSearchResult = response
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;

        Ok(result.result)
    }

    /// Fetch and cache data for a single symbol.
    async fn fetch_symbol_data(&self, symbol: &str, asset_type: &str) -> Result<StockData, String> {
        let quote = self.get_quote(symbol).await?;
        let profile = self.get_profile(symbol).await.ok();

        let name = profile
            .as_ref()
            .and_then(|p| p.name.clone())
            .unwrap_or_else(|| symbol.to_string());

        let market_cap = profile
            .as_ref()
            .and_then(|p| p.market_capitalization)
            .map(|mc| mc * 1_000_000.0) // Finnhub returns market cap in millions
            .unwrap_or(0.0);

        let data = StockData {
            symbol: symbol.to_string(),
            name,
            price: quote.current,
            change_24h: quote.change_percent.unwrap_or(0.0),
            market_cap,
            volume_24h: 0.0, // Finnhub free tier doesn't include volume in quote
            exchange: profile.as_ref().and_then(|p| p.exchange.clone()),
            sector: profile.as_ref().and_then(|p| p.industry.clone()),
            logo: profile.as_ref().and_then(|p| p.logo.clone()),
            asset_type: asset_type.to_string(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        Ok(data)
    }

    /// Refresh all stock data.
    pub async fn refresh_stocks(&self) {
        info!("Refreshing stock data from Finnhub");

        for symbol in STOCK_SYMBOLS {
            match self.fetch_symbol_data(symbol, "stock").await {
                Ok(data) => {
                    let mut cache = self.stock_cache.write().await;
                    cache.insert(symbol.to_string(), data);
                    debug!("Updated stock data for {}", symbol);
                }
                Err(e) => {
                    warn!("Failed to fetch stock data for {}: {}", symbol, e);
                }
            }

            // Rate limiting: Finnhub free tier allows 60 calls/minute
            tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;
        }
    }

    /// Refresh all ETF data.
    pub async fn refresh_etfs(&self) {
        info!("Refreshing ETF data from Finnhub");

        for symbol in ETF_SYMBOLS {
            match self.fetch_symbol_data(symbol, "etf").await {
                Ok(data) => {
                    let mut cache = self.etf_cache.write().await;
                    cache.insert(symbol.to_string(), data);
                    debug!("Updated ETF data for {}", symbol);
                }
                Err(e) => {
                    warn!("Failed to fetch ETF data for {}: {}", symbol, e);
                }
            }

            // Rate limiting
            tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;
        }
    }

    /// Get cached stock listings.
    pub async fn get_stock_listings(&self) -> Vec<StockData> {
        let cache = self.stock_cache.read().await;
        let mut listings: Vec<_> = cache.values().cloned().collect();
        listings.sort_by(|a, b| {
            b.market_cap
                .partial_cmp(&a.market_cap)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        listings
    }

    /// Get cached ETF listings.
    pub async fn get_etf_listings(&self) -> Vec<StockData> {
        let cache = self.etf_cache.read().await;
        let mut listings: Vec<_> = cache.values().cloned().collect();
        listings.sort_by(|a, b| {
            b.market_cap
                .partial_cmp(&a.market_cap)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        listings
    }

    /// Get all cached listings (stocks + ETFs) sorted by market cap.
    pub async fn get_all_listings(&self) -> Vec<StockData> {
        let stocks = self.get_stock_listings().await;
        let etfs = self.get_etf_listings().await;

        let mut all: Vec<_> = stocks.into_iter().chain(etfs).collect();
        all.sort_by(|a, b| {
            b.market_cap
                .partial_cmp(&a.market_cap)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        all
    }

    /// Start background polling.
    pub fn start_polling(self: Arc<Self>) {
        let client = self.clone();
        tokio::spawn(async move {
            loop {
                client.refresh_stocks().await;
                client.refresh_etfs().await;
                tokio::time::sleep(tokio::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
            }
        });
    }

    /// Get poll interval.
    pub fn poll_interval_secs() -> u64 {
        POLL_INTERVAL_SECS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // STOCK_SYMBOLS Tests
    // =========================================================================

    #[test]
    fn test_stock_symbols_contains_major_stocks() {
        assert!(STOCK_SYMBOLS.contains(&"AAPL"));
        assert!(STOCK_SYMBOLS.contains(&"MSFT"));
        assert!(STOCK_SYMBOLS.contains(&"GOOGL"));
        assert!(STOCK_SYMBOLS.contains(&"AMZN"));
        assert!(STOCK_SYMBOLS.contains(&"TSLA"));
    }

    #[test]
    fn test_stock_symbols_count() {
        assert!(STOCK_SYMBOLS.len() >= 20);
    }

    // =========================================================================
    // ETF_SYMBOLS Tests
    // =========================================================================

    #[test]
    fn test_etf_symbols_contains_major_etfs() {
        assert!(ETF_SYMBOLS.contains(&"SPY"));
        assert!(ETF_SYMBOLS.contains(&"QQQ"));
        assert!(ETF_SYMBOLS.contains(&"VOO"));
    }

    #[test]
    fn test_etf_symbols_count() {
        assert!(ETF_SYMBOLS.len() >= 15);
    }

    // =========================================================================
    // FinnhubQuote Tests
    // =========================================================================

    #[test]
    fn test_finnhub_quote_deserialization() {
        let json = r#"{
            "c": 175.50,
            "d": 2.5,
            "dp": 1.45,
            "h": 176.00,
            "l": 172.00,
            "o": 173.00,
            "pc": 173.00,
            "t": 1700000000
        }"#;

        let quote: FinnhubQuote = serde_json::from_str(json).unwrap();
        assert_eq!(quote.current, 175.50);
        assert_eq!(quote.change, Some(2.5));
        assert_eq!(quote.change_percent, Some(1.45));
        assert_eq!(quote.high, 176.00);
        assert_eq!(quote.low, 172.00);
        assert_eq!(quote.open, 173.00);
        assert_eq!(quote.previous_close, 173.00);
        assert_eq!(quote.timestamp, 1700000000);
    }

    #[test]
    fn test_finnhub_quote_optional_fields() {
        let json = r#"{
            "c": 100.0,
            "h": 105.0,
            "l": 95.0,
            "o": 98.0,
            "pc": 99.0,
            "t": 1700000000
        }"#;

        let quote: FinnhubQuote = serde_json::from_str(json).unwrap();
        assert!(quote.change.is_none());
        assert!(quote.change_percent.is_none());
    }

    // =========================================================================
    // FinnhubProfile Tests
    // =========================================================================

    #[test]
    fn test_finnhub_profile_deserialization() {
        let json = r#"{
            "country": "US",
            "currency": "USD",
            "exchange": "NASDAQ",
            "finnhubIndustry": "Technology",
            "ipo": "1980-12-12",
            "logo": "https://example.com/logo.png",
            "marketCapitalization": 2900000,
            "name": "Apple Inc",
            "phone": "+1234567890",
            "shareOutstanding": 15000000,
            "ticker": "AAPL",
            "weburl": "https://apple.com"
        }"#;

        let profile: FinnhubProfile = serde_json::from_str(json).unwrap();
        assert_eq!(profile.country, Some("US".to_string()));
        assert_eq!(profile.name, Some("Apple Inc".to_string()));
        assert_eq!(profile.industry, Some("Technology".to_string()));
        assert_eq!(profile.market_capitalization, Some(2900000.0));
    }

    #[test]
    fn test_finnhub_profile_minimal() {
        let json = r#"{}"#;
        let profile: FinnhubProfile = serde_json::from_str(json).unwrap();
        assert!(profile.name.is_none());
        assert!(profile.market_capitalization.is_none());
    }

    // =========================================================================
    // FinnhubSymbol Tests
    // =========================================================================

    #[test]
    fn test_finnhub_symbol_deserialization() {
        let json = r#"{
            "description": "Apple Inc",
            "displaySymbol": "AAPL",
            "symbol": "AAPL",
            "type": "Common Stock"
        }"#;

        let symbol: FinnhubSymbol = serde_json::from_str(json).unwrap();
        assert_eq!(symbol.description, "Apple Inc");
        assert_eq!(symbol.display_symbol, "AAPL");
        assert_eq!(symbol.symbol, "AAPL");
        assert_eq!(symbol.symbol_type, "Common Stock");
    }

    // =========================================================================
    // FinnhubSearchResult Tests
    // =========================================================================

    #[test]
    fn test_finnhub_search_result_deserialization() {
        let json = r#"{
            "count": 2,
            "result": [
                {"description": "Apple Inc", "displaySymbol": "AAPL", "symbol": "AAPL", "type": "Common Stock"},
                {"description": "Apple Hospitality", "displaySymbol": "APLE", "symbol": "APLE", "type": "Common Stock"}
            ]
        }"#;

        let result: FinnhubSearchResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.count, 2);
        assert_eq!(result.result.len(), 2);
    }

    // =========================================================================
    // StockData Tests
    // =========================================================================

    #[test]
    fn test_stock_data_serialization() {
        let data = StockData {
            symbol: "AAPL".to_string(),
            name: "Apple Inc".to_string(),
            price: 175.50,
            change_24h: 1.45,
            market_cap: 2900000000000.0,
            volume_24h: 50000000.0,
            exchange: Some("NASDAQ".to_string()),
            sector: Some("Technology".to_string()),
            logo: Some("https://example.com/logo.png".to_string()),
            asset_type: "stock".to_string(),
            timestamp: 1700000000000,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"symbol\":\"AAPL\""));
        assert!(json.contains("\"price\":175.5"));
        assert!(json.contains("\"asset_type\":\"stock\""));
    }

    #[test]
    fn test_stock_data_deserialization() {
        let json = r#"{
            "symbol": "MSFT",
            "name": "Microsoft Corporation",
            "price": 380.25,
            "change_24h": 0.75,
            "market_cap": 2800000000000,
            "volume_24h": 25000000,
            "exchange": "NASDAQ",
            "sector": "Technology",
            "logo": null,
            "asset_type": "stock",
            "timestamp": 1700000000000
        }"#;

        let data: StockData = serde_json::from_str(json).unwrap();
        assert_eq!(data.symbol, "MSFT");
        assert_eq!(data.price, 380.25);
        assert!(data.logo.is_none());
    }

    // =========================================================================
    // FinnhubClient Tests
    // =========================================================================

    #[test]
    fn test_finnhub_client_new() {
        let _client = FinnhubClient::new("test_api_key".to_string());
        // Test passes if no panic occurs
    }

    #[test]
    fn test_finnhub_poll_interval() {
        let interval = FinnhubClient::poll_interval_secs();
        assert!(interval > 0);
    }
}
