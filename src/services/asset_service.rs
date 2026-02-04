//! Unified asset service for blending crypto, stocks, and ETFs.
//!
//! Provides a single interface for fetching and filtering assets across
//! different asset types, with support for blended listings sorted by market cap.
//!
//! Features redundant data sources:
//! 1. CoinMarketCap (primary)
//! 2. CoinCap.io (fallback)
//! 3. Historic price data from exchanges (last resort)

use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::api::crypto::AssetType;
use crate::services::{ChartStore, PriceCache};
use crate::sources::{CoinCapClient, CoinMarketCapClient, FinnhubClient};
use crate::types::AssetListing;

/// Well-known crypto assets for fallback listings.
/// These are used when both CMC and CoinCap are unavailable.
const KNOWN_CRYPTO_ASSETS: &[(&str, &str, i32)] = &[
    ("BTC", "Bitcoin", 1),
    ("ETH", "Ethereum", 2),
    ("BNB", "BNB", 3),
    ("XRP", "XRP", 4),
    ("SOL", "Solana", 5),
    ("ADA", "Cardano", 6),
    ("DOGE", "Dogecoin", 7),
    ("TRX", "TRON", 8),
    ("AVAX", "Avalanche", 9),
    ("LINK", "Chainlink", 10),
    ("DOT", "Polkadot", 11),
    ("MATIC", "Polygon", 12),
    ("SHIB", "Shiba Inu", 13),
    ("LTC", "Litecoin", 14),
    ("UNI", "Uniswap", 15),
    ("ATOM", "Cosmos", 16),
    ("XLM", "Stellar", 17),
    ("ETC", "Ethereum Classic", 18),
    ("BCH", "Bitcoin Cash", 19),
    ("FIL", "Filecoin", 20),
];

/// Unified asset service with fallback sources.
pub struct AssetService {
    cmc_client: Arc<CoinMarketCapClient>,
    coincap_client: Arc<CoinCapClient>,
    finnhub_client: Option<Arc<FinnhubClient>>,
    price_cache: Arc<PriceCache>,
    chart_store: Arc<ChartStore>,
}

/// Generate stable ID from symbol (same algorithm as stock_to_listing).
fn symbol_to_id(symbol: &str) -> i64 {
    symbol
        .to_uppercase()
        .bytes()
        .fold(0i64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as i64))
        .abs()
}

impl AssetService {
    /// Create a new asset service with all data sources.
    pub fn new(
        cmc_client: Arc<CoinMarketCapClient>,
        coincap_client: Arc<CoinCapClient>,
        finnhub_client: Option<Arc<FinnhubClient>>,
        price_cache: Arc<PriceCache>,
        chart_store: Arc<ChartStore>,
    ) -> Self {
        Self {
            cmc_client,
            coincap_client,
            finnhub_client,
            price_cache,
            chart_store,
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

    /// Get crypto listings with fallback chain:
    /// 1. CoinMarketCap (primary)
    /// 2. CoinCap.io (fallback)
    /// 3. Historic price data (last resort)
    async fn get_crypto_listings(
        &self,
        page: i32,
        limit: i32,
    ) -> Result<(Vec<AssetListing>, i32), String> {
        // Try CoinMarketCap first
        match self.cmc_client.get_listings(page, limit).await {
            Ok(result) => {
                debug!("CMC returned {} crypto listings", result.data.len());
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
                return Ok((listings, result.total));
            }
            Err(e) => {
                warn!("CMC failed, trying CoinCap: {}", e);
            }
        }

        // Fallback to CoinCap
        let offset = (page - 1) * limit;
        match self.coincap_client.get_listings(limit, offset).await {
            Ok(listings) => {
                info!(
                    "CoinCap returned {} crypto listings (fallback)",
                    listings.len()
                );
                // CoinCap doesn't give us total count, estimate based on crypto market
                let total = 2000; // Reasonable estimate
                return Ok((listings, total));
            }
            Err(e) => {
                warn!("CoinCap also failed, using historic data: {}", e);
            }
        }

        // Last resort: build listings from historic price data
        info!("Building crypto listings from historic exchange data");
        let listings = self.build_crypto_from_historic(page, limit);
        let total = KNOWN_CRYPTO_ASSETS.len() as i32;
        Ok((listings, total))
    }

    /// Build crypto listings from historic price data and known assets.
    fn build_crypto_from_historic(&self, page: i32, limit: i32) -> Vec<AssetListing> {
        let start = ((page - 1) * limit) as usize;

        KNOWN_CRYPTO_ASSETS
            .iter()
            .skip(start)
            .take(limit as usize)
            .filter_map(|(symbol, name, rank)| {
                let symbol_lower = symbol.to_lowercase();

                // Get current price from cache or chart store
                let price = self.price_cache.get_price(&symbol_lower).or_else(|| {
                    // Try chart store for last known price
                    self.chart_store.get_current_price(&symbol_lower)
                })?;

                // Get 24h change from chart store
                let change_24h = self
                    .chart_store
                    .get_price_change(&symbol_lower, 24 * 60 * 60)
                    .unwrap_or(0.0);

                // Get 7d change
                let change_7d = self
                    .chart_store
                    .get_price_change(&symbol_lower, 7 * 24 * 60 * 60)
                    .unwrap_or(0.0);

                // Get sparkline
                let sparkline = self.chart_store.get_sparkline(&symbol_lower, 168);

                // Get volume from chart store
                let volume_24h = self
                    .chart_store
                    .get_volume_24h(&symbol_lower)
                    .unwrap_or(0.0);

                // Get trade direction
                let trade_direction = self.price_cache.get_trade_direction(&symbol_lower);

                Some(AssetListing {
                    id: symbol_to_id(symbol),
                    rank: *rank,
                    name: name.to_string(),
                    symbol: symbol.to_string(),
                    image: format!(
                        "https://assets.coincap.io/assets/icons/{}@2x.png",
                        symbol_lower
                    ),
                    price,
                    change_1h: 0.0,
                    change_24h,
                    change_7d,
                    market_cap: 0.0, // Not available from exchanges
                    volume_24h,
                    circulating_supply: 0.0,
                    max_supply: None,
                    sparkline,
                    trade_direction,
                    asset_type: "crypto".to_string(),
                    exchange: None,
                    sector: None,
                })
            })
            .collect()
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

    /// Get a crypto asset by ID, with fallback to historic data.
    pub async fn get_crypto_by_id(&self, id: i64) -> Option<AssetListing> {
        // Check known assets
        for (symbol, name, rank) in KNOWN_CRYPTO_ASSETS {
            if symbol_to_id(symbol) == id {
                let symbol_lower = symbol.to_lowercase();

                // Get price from cache or chart store
                let price = self
                    .price_cache
                    .get_price(&symbol_lower)
                    .or_else(|| self.chart_store.get_current_price(&symbol_lower))?;

                let change_24h = self
                    .chart_store
                    .get_price_change(&symbol_lower, 24 * 60 * 60)
                    .unwrap_or(0.0);

                let sparkline = self.chart_store.get_sparkline(&symbol_lower, 168);
                let trade_direction = self.price_cache.get_trade_direction(&symbol_lower);

                return Some(AssetListing {
                    id,
                    rank: *rank,
                    name: name.to_string(),
                    symbol: symbol.to_string(),
                    image: format!(
                        "https://assets.coincap.io/assets/icons/{}@2x.png",
                        symbol_lower
                    ),
                    price,
                    change_1h: 0.0,
                    change_24h,
                    change_7d: 0.0,
                    market_cap: 0.0,
                    volume_24h: 0.0,
                    circulating_supply: 0.0,
                    max_supply: None,
                    sparkline,
                    trade_direction,
                    asset_type: "crypto".to_string(),
                    exchange: None,
                    sector: None,
                });
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
        let mut all_listings = Vec::new();

        // Get crypto with fallback
        match self.get_crypto_listings(1, 100).await {
            Ok((crypto, _)) => {
                all_listings.extend(crypto);
            }
            Err(e) => {
                warn!("Failed to get crypto for blended listings: {}", e);
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
fn stock_to_listing(stock: crate::sources::finnhub::StockData, rank: i32) -> AssetListing {
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
