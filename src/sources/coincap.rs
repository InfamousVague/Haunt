//! CoinCap.io API client for cryptocurrency listings.
//!
//! CoinCap provides free, no-API-key-required access to crypto data.
//! Used as a fallback when CoinMarketCap is unavailable.

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

        let response = self.client
            .get(&url)
            .header("Accept-Encoding", "gzip")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            warn!("CoinCap API error: {} - {}", status, &text[..text.len().min(200)]);
            anyhow::bail!("CoinCap API error: {}", status);
        }

        let data: CoinCapResponse<Vec<CoinCapAsset>> = response.json().await?;

        let listings: Vec<AssetListing> = data.data
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
                a.name.to_lowercase().contains(&query_lower) ||
                a.symbol.to_lowercase().contains(&query_lower)
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
        let id = asset.symbol
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
            change_24h: asset.change_percent_24_hr
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0),
            change_7d: 0.0, // CoinCap doesn't provide 7d change in listings
            market_cap: asset.market_cap_usd
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0),
            volume_24h: asset.volume_usd_24_hr
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0),
            circulating_supply: asset.supply
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
