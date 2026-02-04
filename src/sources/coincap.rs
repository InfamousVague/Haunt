//! CoinCap.io API client for cryptocurrency listings.
//!
//! CoinCap provides free, no-API-key-required access to crypto data.
//! Used as a fallback when CoinMarketCap is unavailable.

// Some fields are kept for API completeness
#![allow(dead_code)]

use crate::types::AssetListing;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::{debug, warn};

const COINCAP_API_URL: &str = "https://api.coincap.io/v2";

/// CoinCap REST client.
#[derive(Clone)]
pub struct CoinCapClient {
    client: Client,
}

#[derive(Debug, Deserialize)]
struct CoinCapResponse<T> {
    data: T,
    timestamp: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CoinCapAsset {
    id: String,
    rank: String,
    symbol: String,
    name: String,
    supply: Option<String>,
    max_supply: Option<String>,
    market_cap_usd: Option<String>,
    volume_usd_24_hr: Option<String>,
    price_usd: Option<String>,
    change_percent_24_hr: Option<String>,
    vwap_24_hr: Option<String>,
}

impl CoinCapClient {
    /// Create a new CoinCap client.
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    /// Get cryptocurrency listings.
    pub async fn get_listings(&self, limit: i32, offset: i32) -> anyhow::Result<Vec<AssetListing>> {
        let url = format!(
            "{}/assets?limit={}&offset={}",
            COINCAP_API_URL, limit, offset
        );

        debug!("Fetching CoinCap listings: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Accept-Encoding", "gzip")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            warn!(
                "CoinCap API error: {} - {}",
                status,
                &text[..text.len().min(200)]
            );
            anyhow::bail!("CoinCap API error: {}", status);
        }

        let data: CoinCapResponse<Vec<CoinCapAsset>> = response.json().await?;

        let listings: Vec<AssetListing> = data
            .data
            .into_iter()
            .filter_map(|asset| self.convert_to_listing(asset))
            .collect();

        debug!("CoinCap returned {} listings", listings.len());
        Ok(listings)
    }

    /// Search for assets by symbol or name.
    pub async fn search(&self, query: &str, limit: i32) -> anyhow::Result<Vec<AssetListing>> {
        // CoinCap doesn't have a search endpoint, so we fetch and filter
        let all = self.get_listings(100, 0).await?;

        let query_lower = query.to_lowercase();
        let results: Vec<AssetListing> = all
            .into_iter()
            .filter(|a| {
                a.name.to_lowercase().contains(&query_lower)
                    || a.symbol.to_lowercase().contains(&query_lower)
            })
            .take(limit as usize)
            .collect();

        Ok(results)
    }

    /// Convert CoinCap asset to our AssetListing format.
    fn convert_to_listing(&self, asset: CoinCapAsset) -> Option<AssetListing> {
        let price = asset.price_usd.as_ref()?.parse::<f64>().ok()?;
        let rank = asset.rank.parse::<i32>().unwrap_or(0);

        // Generate a stable ID from the symbol (same algorithm as other sources)
        let id = asset
            .symbol
            .bytes()
            .fold(0i64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as i64))
            .abs();

        Some(AssetListing {
            id,
            rank,
            name: asset.name,
            symbol: asset.symbol.to_uppercase(),
            image: format!(
                "https://assets.coincap.io/assets/icons/{}@2x.png",
                asset.symbol.to_lowercase()
            ),
            price,
            change_1h: 0.0, // CoinCap doesn't provide 1h change
            change_24h: asset
                .change_percent_24_hr
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0),
            change_7d: 0.0, // CoinCap doesn't provide 7d change in listings
            market_cap: asset
                .market_cap_usd
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0),
            volume_24h: asset
                .volume_usd_24_hr
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0),
            circulating_supply: asset
                .supply
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0),
            max_supply: asset.max_supply.and_then(|s| s.parse::<f64>().ok()),
            sparkline: vec![],
            trade_direction: None,
            asset_type: "crypto".to_string(),
            exchange: None,
            sector: None,
        })
    }
}

impl Default for CoinCapClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // CoinCapResponse Tests
    // =========================================================================

    #[test]
    fn test_coincap_response_deserialization() {
        let json = r#"{"data": [1, 2, 3], "timestamp": 1700000000000}"#;
        let response: CoinCapResponse<Vec<i32>> = serde_json::from_str(json).unwrap();
        assert_eq!(response.data, vec![1, 2, 3]);
        assert_eq!(response.timestamp, Some(1700000000000));
    }

    #[test]
    fn test_coincap_response_without_timestamp() {
        let json = r#"{"data": "test"}"#;
        let response: CoinCapResponse<String> = serde_json::from_str(json).unwrap();
        assert_eq!(response.data, "test");
        assert!(response.timestamp.is_none());
    }

    // =========================================================================
    // CoinCapAsset Tests
    // =========================================================================

    #[test]
    fn test_coincap_asset_deserialization() {
        let json = r#"{
            "id": "bitcoin",
            "rank": "1",
            "symbol": "BTC",
            "name": "Bitcoin",
            "supply": "19500000",
            "maxSupply": "21000000",
            "marketCapUsd": "850000000000",
            "volumeUsd24Hr": "15000000000",
            "priceUsd": "43500.50",
            "changePercent24Hr": "2.5",
            "vwap24Hr": "43000"
        }"#;

        let asset: CoinCapAsset = serde_json::from_str(json).unwrap();
        assert_eq!(asset.id, "bitcoin");
        assert_eq!(asset.rank, "1");
        assert_eq!(asset.symbol, "BTC");
        assert_eq!(asset.name, "Bitcoin");
        assert_eq!(asset.supply, Some("19500000".to_string()));
        assert_eq!(asset.max_supply, Some("21000000".to_string()));
        assert_eq!(asset.market_cap_usd, Some("850000000000".to_string()));
        assert_eq!(asset.volume_usd_24_hr, Some("15000000000".to_string()));
        assert_eq!(asset.price_usd, Some("43500.50".to_string()));
        assert_eq!(asset.change_percent_24_hr, Some("2.5".to_string()));
        assert_eq!(asset.vwap_24_hr, Some("43000".to_string()));
    }

    #[test]
    fn test_coincap_asset_minimal() {
        let json = r#"{
            "id": "ethereum",
            "rank": "2",
            "symbol": "ETH",
            "name": "Ethereum"
        }"#;

        let asset: CoinCapAsset = serde_json::from_str(json).unwrap();
        assert_eq!(asset.id, "ethereum");
        assert_eq!(asset.symbol, "ETH");
        assert!(asset.price_usd.is_none());
        assert!(asset.supply.is_none());
    }

    // =========================================================================
    // CoinCapClient Tests
    // =========================================================================

    #[test]
    fn test_coincap_client_new() {
        let _client = CoinCapClient::new();
        // Test passes if no panic occurs
    }

    #[test]
    fn test_coincap_client_default() {
        let _client = CoinCapClient::default();
        // Test passes if no panic occurs
    }

    #[test]
    fn test_convert_to_listing_full() {
        let client = CoinCapClient::new();
        let asset = CoinCapAsset {
            id: "bitcoin".to_string(),
            rank: "1".to_string(),
            symbol: "BTC".to_string(),
            name: "Bitcoin".to_string(),
            supply: Some("19500000".to_string()),
            max_supply: Some("21000000".to_string()),
            market_cap_usd: Some("850000000000".to_string()),
            volume_usd_24_hr: Some("15000000000".to_string()),
            price_usd: Some("43500.50".to_string()),
            change_percent_24_hr: Some("2.5".to_string()),
            vwap_24_hr: Some("43000".to_string()),
        };

        let listing = client.convert_to_listing(asset).unwrap();
        assert_eq!(listing.name, "Bitcoin");
        assert_eq!(listing.symbol, "BTC");
        assert_eq!(listing.rank, 1);
        assert_eq!(listing.price, 43500.50);
        assert_eq!(listing.change_24h, 2.5);
        assert_eq!(listing.market_cap, 850000000000.0);
        assert_eq!(listing.volume_24h, 15000000000.0);
        assert_eq!(listing.circulating_supply, 19500000.0);
        assert_eq!(listing.max_supply, Some(21000000.0));
        assert_eq!(listing.asset_type, "crypto");
        assert!(listing.image.contains("btc"));
    }

    #[test]
    fn test_convert_to_listing_without_price_returns_none() {
        let client = CoinCapClient::new();
        let asset = CoinCapAsset {
            id: "unknown".to_string(),
            rank: "999".to_string(),
            symbol: "UNK".to_string(),
            name: "Unknown".to_string(),
            supply: None,
            max_supply: None,
            market_cap_usd: None,
            volume_usd_24_hr: None,
            price_usd: None, // No price
            change_percent_24_hr: None,
            vwap_24_hr: None,
        };

        let listing = client.convert_to_listing(asset);
        assert!(listing.is_none());
    }

    #[test]
    fn test_convert_to_listing_invalid_price_returns_none() {
        let client = CoinCapClient::new();
        let asset = CoinCapAsset {
            id: "bad".to_string(),
            rank: "1".to_string(),
            symbol: "BAD".to_string(),
            name: "Bad".to_string(),
            supply: None,
            max_supply: None,
            market_cap_usd: None,
            volume_usd_24_hr: None,
            price_usd: Some("not-a-number".to_string()), // Invalid price
            change_percent_24_hr: None,
            vwap_24_hr: None,
        };

        let listing = client.convert_to_listing(asset);
        assert!(listing.is_none());
    }

    #[test]
    fn test_convert_to_listing_symbol_uppercase() {
        let client = CoinCapClient::new();
        let asset = CoinCapAsset {
            id: "test".to_string(),
            rank: "100".to_string(),
            symbol: "test".to_string(), // lowercase
            name: "Test".to_string(),
            supply: None,
            max_supply: None,
            market_cap_usd: None,
            volume_usd_24_hr: None,
            price_usd: Some("1.0".to_string()),
            change_percent_24_hr: None,
            vwap_24_hr: None,
        };

        let listing = client.convert_to_listing(asset).unwrap();
        assert_eq!(listing.symbol, "TEST"); // Uppercased
    }

    #[test]
    fn test_convert_to_listing_generates_id() {
        let client = CoinCapClient::new();
        let asset = CoinCapAsset {
            id: "bitcoin".to_string(),
            rank: "1".to_string(),
            symbol: "BTC".to_string(),
            name: "Bitcoin".to_string(),
            supply: None,
            max_supply: None,
            market_cap_usd: None,
            volume_usd_24_hr: None,
            price_usd: Some("50000".to_string()),
            change_percent_24_hr: None,
            vwap_24_hr: None,
        };

        let listing = client.convert_to_listing(asset).unwrap();
        // ID should be a non-zero stable hash of the symbol
        assert!(listing.id != 0);
    }

    #[test]
    fn test_convert_to_listing_defaults() {
        let client = CoinCapClient::new();
        let asset = CoinCapAsset {
            id: "test".to_string(),
            rank: "10".to_string(),
            symbol: "TST".to_string(),
            name: "Test".to_string(),
            supply: None,
            max_supply: None,
            market_cap_usd: None,
            volume_usd_24_hr: None,
            price_usd: Some("100.0".to_string()),
            change_percent_24_hr: None,
            vwap_24_hr: None,
        };

        let listing = client.convert_to_listing(asset).unwrap();
        assert_eq!(listing.change_1h, 0.0);
        assert_eq!(listing.change_24h, 0.0);
        assert_eq!(listing.change_7d, 0.0);
        assert_eq!(listing.market_cap, 0.0);
        assert_eq!(listing.volume_24h, 0.0);
        assert_eq!(listing.circulating_supply, 0.0);
        assert!(listing.max_supply.is_none());
        assert!(listing.sparkline.is_empty());
        assert!(listing.trade_direction.is_none());
        assert!(listing.exchange.is_none());
        assert!(listing.sector.is_none());
    }
}
