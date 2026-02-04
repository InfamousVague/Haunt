use super::TradeDirection;
use serde::{Deserialize, Serialize};

/// A cryptocurrency asset with full metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: i64,
    pub name: String,
    pub symbol: String,
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_added: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urls: Option<AssetUrls>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote: Option<Quote>,
}

/// URLs associated with an asset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetUrls {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explorer: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_code: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twitter: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reddit: Option<Vec<String>>,
}

/// Quote data for an asset.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    pub price: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_24h: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_change_24h: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_cap: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_cap_dominance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent_change_1h: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent_change_24h: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent_change_7d: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent_change_30d: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fully_diluted_market_cap: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub circulating_supply: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_supply: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_supply: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
}

/// A simplified asset listing for paginated results.
/// This matches the frontend's expected Asset type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetListing {
    pub id: i64,
    pub rank: i32,
    pub name: String,
    pub symbol: String,
    pub image: String,
    pub price: f64,
    pub change_1h: f64,
    pub change_24h: f64,
    pub change_7d: f64,
    pub market_cap: f64,
    pub volume_24h: f64,
    pub circulating_supply: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_supply: Option<f64>,
    pub sparkline: Vec<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trade_direction: Option<TradeDirection>,
    /// Asset type discriminator: "crypto", "stock", "etf"
    #[serde(default)]
    pub asset_type: String,
    /// Exchange name (for stocks/ETFs): "NASDAQ", "NYSE"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exchange: Option<String>,
    /// Sector (for stocks): "Technology", "Healthcare"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sector: Option<String>,
}

/// Paginated response wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub page: i32,
    pub limit: i32,
    pub total: i32,
    #[serde(rename = "hasMore")]
    pub has_more: bool,
}

/// Search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct SearchResult {
    pub id: i64,
    pub name: String,
    pub symbol: String,
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmc_rank: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Asset Tests
    // =========================================================================

    #[test]
    fn test_asset_minimal_creation() {
        let asset = Asset {
            id: 1,
            name: "Bitcoin".to_string(),
            symbol: "BTC".to_string(),
            slug: "bitcoin".to_string(),
            rank: None,
            logo: None,
            description: None,
            category: None,
            date_added: None,
            tags: None,
            urls: None,
            quote: None,
        };

        assert_eq!(asset.id, 1);
        assert_eq!(asset.name, "Bitcoin");
        assert_eq!(asset.symbol, "BTC");
    }

    #[test]
    fn test_asset_full_creation() {
        let asset = Asset {
            id: 1,
            name: "Bitcoin".to_string(),
            symbol: "BTC".to_string(),
            slug: "bitcoin".to_string(),
            rank: Some(1),
            logo: Some("https://example.com/btc.png".to_string()),
            description: Some("Digital gold".to_string()),
            category: Some("Cryptocurrency".to_string()),
            date_added: Some("2013-04-28".to_string()),
            tags: Some(vec!["pow".to_string(), "store-of-value".to_string()]),
            urls: Some(AssetUrls {
                website: Some(vec!["https://bitcoin.org".to_string()]),
                explorer: None,
                source_code: Some(vec!["https://github.com/bitcoin".to_string()]),
                twitter: None,
                reddit: None,
            }),
            quote: Some(Quote {
                price: 50000.0,
                volume_24h: Some(30_000_000_000.0),
                volume_change_24h: Some(5.0),
                market_cap: Some(1_000_000_000_000.0),
                market_cap_dominance: Some(52.0),
                percent_change_1h: Some(0.5),
                percent_change_24h: Some(2.0),
                percent_change_7d: Some(5.0),
                percent_change_30d: Some(-3.0),
                fully_diluted_market_cap: Some(1_050_000_000_000.0),
                circulating_supply: Some(19_000_000.0),
                total_supply: Some(19_000_000.0),
                max_supply: Some(21_000_000.0),
                last_updated: Some("2024-01-01T00:00:00Z".to_string()),
            }),
        };

        assert_eq!(asset.rank, Some(1));
        assert!(asset.tags.as_ref().unwrap().contains(&"pow".to_string()));
    }

    #[test]
    fn test_asset_serialization_skips_none() {
        let asset = Asset {
            id: 1,
            name: "Test".to_string(),
            symbol: "TST".to_string(),
            slug: "test".to_string(),
            rank: None,
            logo: None,
            description: None,
            category: None,
            date_added: None,
            tags: None,
            urls: None,
            quote: None,
        };

        let json = serde_json::to_string(&asset).unwrap();
        assert!(!json.contains("rank"));
        assert!(!json.contains("logo"));
        assert!(!json.contains("description"));
    }

    // =========================================================================
    // AssetUrls Tests
    // =========================================================================

    #[test]
    fn test_asset_urls_creation() {
        let urls = AssetUrls {
            website: Some(vec!["https://example.com".to_string()]),
            explorer: Some(vec!["https://explorer.com".to_string()]),
            source_code: Some(vec!["https://github.com".to_string()]),
            twitter: Some(vec!["https://twitter.com/example".to_string()]),
            reddit: Some(vec!["https://reddit.com/r/example".to_string()]),
        };

        assert_eq!(urls.website.as_ref().unwrap().len(), 1);
        assert!(urls.explorer.is_some());
    }

    #[test]
    fn test_asset_urls_serialization_skips_none() {
        let urls = AssetUrls {
            website: Some(vec!["https://example.com".to_string()]),
            explorer: None,
            source_code: None,
            twitter: None,
            reddit: None,
        };

        let json = serde_json::to_string(&urls).unwrap();
        assert!(json.contains("website"));
        assert!(!json.contains("explorer"));
    }

    // =========================================================================
    // Quote Tests
    // =========================================================================

    #[test]
    fn test_quote_minimal() {
        let quote = Quote {
            price: 50000.0,
            volume_24h: None,
            volume_change_24h: None,
            market_cap: None,
            market_cap_dominance: None,
            percent_change_1h: None,
            percent_change_24h: None,
            percent_change_7d: None,
            percent_change_30d: None,
            fully_diluted_market_cap: None,
            circulating_supply: None,
            total_supply: None,
            max_supply: None,
            last_updated: None,
        };

        assert_eq!(quote.price, 50000.0);
    }

    #[test]
    fn test_quote_serialization() {
        let quote = Quote {
            price: 50000.0,
            volume_24h: Some(30_000_000_000.0),
            volume_change_24h: None,
            market_cap: Some(1_000_000_000_000.0),
            market_cap_dominance: None,
            percent_change_1h: None,
            percent_change_24h: Some(2.5),
            percent_change_7d: None,
            percent_change_30d: None,
            fully_diluted_market_cap: None,
            circulating_supply: None,
            total_supply: None,
            max_supply: None,
            last_updated: None,
        };

        let json = serde_json::to_string(&quote).unwrap();
        assert!(json.contains("\"price\":50000"));
        assert!(json.contains("\"volume24h\":"));
        assert!(json.contains("\"marketCap\":"));
        assert!(json.contains("\"percentChange24h\":2.5"));
        assert!(!json.contains("percentChange1h")); // None should be omitted
    }

    // =========================================================================
    // AssetListing Tests
    // =========================================================================

    #[test]
    fn test_asset_listing_crypto() {
        let listing = AssetListing {
            id: 1,
            rank: 1,
            name: "Bitcoin".to_string(),
            symbol: "BTC".to_string(),
            image: "https://example.com/btc.png".to_string(),
            price: 50000.0,
            change_1h: 0.5,
            change_24h: 2.0,
            change_7d: 5.0,
            market_cap: 1_000_000_000_000.0,
            volume_24h: 30_000_000_000.0,
            circulating_supply: 19_000_000.0,
            max_supply: Some(21_000_000.0),
            sparkline: vec![49000.0, 49500.0, 50000.0],
            trade_direction: Some(TradeDirection::Up),
            asset_type: "crypto".to_string(),
            exchange: None,
            sector: None,
        };

        assert_eq!(listing.asset_type, "crypto");
        assert_eq!(listing.trade_direction, Some(TradeDirection::Up));
        assert_eq!(listing.sparkline.len(), 3);
    }

    #[test]
    fn test_asset_listing_stock() {
        let listing = AssetListing {
            id: 100,
            rank: 1,
            name: "Apple Inc.".to_string(),
            symbol: "AAPL".to_string(),
            image: "https://example.com/aapl.png".to_string(),
            price: 180.0,
            change_1h: 0.1,
            change_24h: 1.5,
            change_7d: 3.0,
            market_cap: 3_000_000_000_000.0,
            volume_24h: 80_000_000.0,
            circulating_supply: 15_000_000_000.0,
            max_supply: None,
            sparkline: vec![178.0, 179.0, 180.0],
            trade_direction: Some(TradeDirection::Up),
            asset_type: "stock".to_string(),
            exchange: Some("NASDAQ".to_string()),
            sector: Some("Technology".to_string()),
        };

        assert_eq!(listing.asset_type, "stock");
        assert_eq!(listing.exchange, Some("NASDAQ".to_string()));
        assert_eq!(listing.sector, Some("Technology".to_string()));
    }

    #[test]
    fn test_asset_listing_serialization() {
        let listing = AssetListing {
            id: 1,
            rank: 1,
            name: "Bitcoin".to_string(),
            symbol: "BTC".to_string(),
            image: "https://example.com/btc.png".to_string(),
            price: 50000.0,
            change_1h: 0.5,
            change_24h: 2.0,
            change_7d: 5.0,
            market_cap: 1_000_000_000_000.0,
            volume_24h: 30_000_000_000.0,
            circulating_supply: 19_000_000.0,
            max_supply: None,
            sparkline: vec![],
            trade_direction: None,
            asset_type: "crypto".to_string(),
            exchange: None,
            sector: None,
        };

        let json = serde_json::to_string(&listing).unwrap();
        assert!(json.contains("\"change1h\":0.5"));
        assert!(json.contains("\"change24h\":2"));
        assert!(!json.contains("maxSupply")); // None should be omitted
        assert!(!json.contains("tradeDirection")); // None should be omitted
    }

    // =========================================================================
    // PaginatedResponse Tests
    // =========================================================================

    #[test]
    fn test_paginated_response_creation() {
        let response: PaginatedResponse<String> = PaginatedResponse {
            data: vec!["item1".to_string(), "item2".to_string()],
            page: 1,
            limit: 10,
            total: 100,
            has_more: true,
        };

        assert_eq!(response.data.len(), 2);
        assert_eq!(response.page, 1);
        assert_eq!(response.total, 100);
        assert!(response.has_more);
    }

    #[test]
    fn test_paginated_response_last_page() {
        let response: PaginatedResponse<i32> = PaginatedResponse {
            data: vec![1, 2, 3],
            page: 10,
            limit: 10,
            total: 93,
            has_more: false,
        };

        assert!(!response.has_more);
    }

    #[test]
    fn test_paginated_response_serialization() {
        let response: PaginatedResponse<String> = PaginatedResponse {
            data: vec!["item".to_string()],
            page: 1,
            limit: 10,
            total: 1,
            has_more: false,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"data\":[\"item\"]"));
        assert!(json.contains("\"page\":1"));
        assert!(json.contains("\"hasMore\":false"));
    }

    // =========================================================================
    // SearchResult Tests
    // =========================================================================

    #[test]
    fn test_search_result_creation() {
        let result = SearchResult {
            id: 1,
            name: "Bitcoin".to_string(),
            symbol: "BTC".to_string(),
            slug: "bitcoin".to_string(),
            logo: Some("https://example.com/btc.png".to_string()),
            cmc_rank: Some(1),
        };

        assert_eq!(result.id, 1);
        assert_eq!(result.symbol, "BTC");
        assert_eq!(result.cmc_rank, Some(1));
    }

    #[test]
    fn test_search_result_minimal() {
        let result = SearchResult {
            id: 999,
            name: "Unknown Token".to_string(),
            symbol: "UNK".to_string(),
            slug: "unknown-token".to_string(),
            logo: None,
            cmc_rank: None,
        };

        assert!(result.logo.is_none());
        assert!(result.cmc_rank.is_none());
    }

    #[test]
    fn test_search_result_serialization() {
        let result = SearchResult {
            id: 1,
            name: "Bitcoin".to_string(),
            symbol: "BTC".to_string(),
            slug: "bitcoin".to_string(),
            logo: None,
            cmc_rank: Some(1),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"cmcRank\":1"));
        assert!(!json.contains("logo")); // None should be omitted
    }
}
