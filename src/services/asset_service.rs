//! Unified asset service for blending crypto, stocks, and ETFs.
//!
//! Provides a single interface for fetching and filtering assets across
//! different asset types, with support for blended listings sorted by market cap.

use std::sync::Arc;
use tracing::{debug, info};

use crate::api::crypto::AssetType;
use crate::sources::{CoinMarketCapClient, FinnhubClient};
use crate::types::AssetListing;

/// Unified asset service.
pub struct AssetService {
    cmc_client: Arc<CoinMarketCapClient>,
    finnhub_client: Option<Arc<FinnhubClient>>,
}

/// Generate stable ID from symbol (same algorithm as stock_to_listing).
fn symbol_to_id(symbol: &str) -> i64 {
    symbol
        .bytes()
        .fold(0i64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as i64))
        .abs()
}

impl AssetService {
    /// Create a new asset service.
    pub fn new(
        cmc_client: Arc<CoinMarketCapClient>,
        finnhub_client: Option<Arc<FinnhubClient>>,
    ) -> Self {
        Self {
            cmc_client,
            finnhub_client,
        }
    }

    /// Get unified listings based on asset type filter.
    pub async fn get_listings(
        &self,
        asset_type: AssetType,
        page: i32,
        limit: i32,
    ) -> Result<(Vec<AssetListing>, i32), String> {
        match asset_type {
            AssetType::All => self.get_blended_listings(page, limit).await,
            AssetType::Crypto => self.get_crypto_listings(page, limit).await,
            AssetType::Stock => self.get_stock_listings(page, limit).await,
            AssetType::Etf => self.get_etf_listings(page, limit).await,
            _ => self.get_crypto_listings(page, limit).await,
        }
    }

    /// Get crypto listings from CoinMarketCap.
    async fn get_crypto_listings(
        &self,
        page: i32,
        limit: i32,
    ) -> Result<(Vec<AssetListing>, i32), String> {
        let result = self
            .cmc_client
            .get_listings(page, limit)
            .await
            .map_err(|e| format!("Failed to fetch crypto listings: {}", e))?;

        // Add asset_type to each listing
        let listings: Vec<AssetListing> = result
            .data
            .into_iter()
            .map(|mut listing| {
                if listing.asset_type.is_empty() {
                    listing.asset_type = "crypto".to_string();
                }
                listing
            })
            .collect();

        Ok((listings, result.total))
    }

    /// Get stock listings from Finnhub.
    async fn get_stock_listings(
        &self,
        page: i32,
        limit: i32,
    ) -> Result<(Vec<AssetListing>, i32), String> {
        let Some(finnhub) = &self.finnhub_client else {
            return Ok((vec![], 0));
        };

        let stocks = finnhub.get_stock_listings().await;
        let total = stocks.len() as i32;

        // Convert to AssetListing and paginate
        let start = ((page - 1) * limit) as usize;
        let listings: Vec<AssetListing> = stocks
            .into_iter()
            .skip(start)
            .take(limit as usize)
            .enumerate()
            .map(|(idx, stock)| stock_to_listing(stock, (start + idx + 1) as i32))
            .collect();

        Ok((listings, total))
    }

    /// Get ETF listings from Finnhub.
    async fn get_etf_listings(
        &self,
        page: i32,
        limit: i32,
    ) -> Result<(Vec<AssetListing>, i32), String> {
        let Some(finnhub) = &self.finnhub_client else {
            return Ok((vec![], 0));
        };

        let etfs = finnhub.get_etf_listings().await;
        let total = etfs.len() as i32;

        // Convert to AssetListing and paginate
        let start = ((page - 1) * limit) as usize;
        let listings: Vec<AssetListing> = etfs
            .into_iter()
            .skip(start)
            .take(limit as usize)
            .enumerate()
            .map(|(idx, etf)| stock_to_listing(etf, (start + idx + 1) as i32))
            .collect();

        Ok((listings, total))
    }

    /// Get a stock or ETF asset by ID.
    /// Returns None if not found or if Finnhub is not configured.
    pub async fn get_stock_or_etf_by_id(&self, id: i64) -> Option<AssetListing> {
        let finnhub = self.finnhub_client.as_ref()?;

        // Check stocks
        let stocks = finnhub.get_stock_listings().await;
        for (idx, stock) in stocks.into_iter().enumerate() {
            let listing = stock_to_listing(stock, (idx + 1) as i32);
            if listing.id == id {
                return Some(listing);
            }
        }

        // Check ETFs
        let etfs = finnhub.get_etf_listings().await;
        for (idx, etf) in etfs.into_iter().enumerate() {
            let listing = stock_to_listing(etf, (idx + 1) as i32);
            if listing.id == id {
                return Some(listing);
            }
        }

        None
    }

    /// Get blended listings (crypto + stocks + ETFs) sorted by market cap.
    async fn get_blended_listings(
        &self,
        page: i32,
        limit: i32,
    ) -> Result<(Vec<AssetListing>, i32), String> {
        // Fetch all available assets
        let mut all_listings = Vec::new();

        // Get crypto (fetch more to have enough for blending)
        if let Ok(result) = self.cmc_client.get_listings(1, 100).await {
            for mut listing in result.data {
                if listing.asset_type.is_empty() {
                    listing.asset_type = "crypto".to_string();
                }
                all_listings.push(listing);
            }
        }

        // Get stocks and ETFs if Finnhub is available
        if let Some(finnhub) = &self.finnhub_client {
            let stocks = finnhub.get_stock_listings().await;
            for (idx, stock) in stocks.into_iter().enumerate() {
                all_listings.push(stock_to_listing(stock, (idx + 1) as i32));
            }

            let etfs = finnhub.get_etf_listings().await;
            for (idx, etf) in etfs.into_iter().enumerate() {
                all_listings.push(stock_to_listing(etf, (idx + 1) as i32));
            }
        }

        // Sort by market cap descending
        all_listings.sort_by(|a, b| {
            b.market_cap
                .partial_cmp(&a.market_cap)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Re-assign ranks after sorting
        for (idx, listing) in all_listings.iter_mut().enumerate() {
            listing.rank = (idx + 1) as i32;
        }

        let total = all_listings.len() as i32;

        // Paginate
        let start = ((page - 1) * limit) as usize;
        let listings: Vec<AssetListing> = all_listings
            .into_iter()
            .skip(start)
            .take(limit as usize)
            .collect();

        Ok((listings, total))
    }
}

/// Convert Finnhub stock data to AssetListing.
fn stock_to_listing(
    stock: crate::sources::finnhub::StockData,
    rank: i32,
) -> AssetListing {
    // Generate a stable ID from symbol hash
    let id = stock
        .symbol
        .bytes()
        .fold(0i64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as i64))
        .abs();

    // Generate logo URL or use provided
    let image = stock.logo.unwrap_or_else(|| {
        format!(
            "https://financialmodelingprep.com/image-stock/{}.png",
            stock.symbol
        )
    });

    AssetListing {
        id,
        rank,
        name: stock.name,
        symbol: stock.symbol,
        image,
        price: stock.price,
        change_1h: 0.0, // Not available from Finnhub free tier
        change_24h: stock.change_24h,
        change_7d: 0.0, // Not available from Finnhub free tier
        market_cap: stock.market_cap,
        volume_24h: stock.volume_24h,
        circulating_supply: 0.0, // Not applicable for stocks
        max_supply: None,
        sparkline: vec![],
        trade_direction: None,
        asset_type: stock.asset_type,
        exchange: stock.exchange,
        sector: stock.sector,
    }
}
