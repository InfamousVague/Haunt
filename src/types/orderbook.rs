//! Order Book types for aggregated depth data across exchanges.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::PriceSource;

/// A single price level in an order book.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderBookLevel {
    /// Price at this level
    pub price: f64,
    /// Total quantity available at this price
    pub quantity: f64,
}

/// Order book from a single exchange.
#[derive(Debug, Clone)]
pub struct ExchangeOrderBook {
    /// Source exchange
    pub exchange: PriceSource,
    /// Symbol this order book is for
    pub symbol: String,
    /// Bid levels (buy orders), sorted by price descending
    pub bids: Vec<OrderBookLevel>,
    /// Ask levels (sell orders), sorted by price ascending
    pub asks: Vec<OrderBookLevel>,
    /// Timestamp when fetched (unix ms)
    pub timestamp: i64,
}

/// A price level in the aggregated order book with exchange breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregatedLevel {
    /// Price at this level
    pub price: f64,
    /// Total quantity across all exchanges
    pub total_quantity: f64,
    /// Quantity per exchange
    pub exchanges: HashMap<String, f64>,
}

/// Aggregated order book across multiple exchanges.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregatedOrderBook {
    /// Symbol this order book is for
    pub symbol: String,
    /// Aggregated bid levels, sorted by price descending
    pub bids: Vec<AggregatedLevel>,
    /// Aggregated ask levels, sorted by price ascending
    pub asks: Vec<AggregatedLevel>,
    /// Total bid volume (sum of all bid quantities)
    pub bid_total: f64,
    /// Total ask volume (sum of all ask quantities)
    pub ask_total: f64,
    /// Order book imbalance: (bid_total - ask_total) / (bid_total + ask_total)
    /// Range: -1.0 (all asks) to +1.0 (all bids)
    pub imbalance: f64,
    /// Best bid price
    pub best_bid: f64,
    /// Best ask price
    pub best_ask: f64,
    /// Spread: best_ask - best_bid
    pub spread: f64,
    /// Spread as percentage of mid price
    pub spread_pct: f64,
    /// Mid price: (best_bid + best_ask) / 2
    pub mid_price: f64,
    /// Number of exchanges contributing to this book
    pub exchange_count: usize,
    /// List of contributing exchanges
    pub exchanges: Vec<String>,
    /// Timestamp when aggregated (unix ms)
    pub timestamp: i64,
}

impl AggregatedOrderBook {
    /// Create a new aggregated order book from multiple exchange books.
    pub fn from_exchange_books(
        symbol: &str,
        books: Vec<ExchangeOrderBook>,
        max_levels: usize,
    ) -> Self {
        let mut bid_map: HashMap<u64, AggregatedLevel> = HashMap::new();
        let mut ask_map: HashMap<u64, AggregatedLevel> = HashMap::new();
        let mut exchanges = Vec::new();

        // Aggregate levels from all exchanges
        for book in &books {
            let exchange_name = book.exchange.to_string();
            if !exchanges.contains(&exchange_name) {
                exchanges.push(exchange_name.clone());
            }

            // Aggregate bids
            for level in &book.bids {
                let price_key = Self::price_to_key(level.price);
                let entry = bid_map.entry(price_key).or_insert_with(|| AggregatedLevel {
                    price: level.price,
                    total_quantity: 0.0,
                    exchanges: HashMap::new(),
                });
                entry.total_quantity += level.quantity;
                *entry.exchanges.entry(exchange_name.clone()).or_insert(0.0) += level.quantity;
            }

            // Aggregate asks
            for level in &book.asks {
                let price_key = Self::price_to_key(level.price);
                let entry = ask_map.entry(price_key).or_insert_with(|| AggregatedLevel {
                    price: level.price,
                    total_quantity: 0.0,
                    exchanges: HashMap::new(),
                });
                entry.total_quantity += level.quantity;
                *entry.exchanges.entry(exchange_name.clone()).or_insert(0.0) += level.quantity;
            }
        }

        // Convert to sorted vectors
        let mut bids: Vec<AggregatedLevel> = bid_map.into_values().collect();
        let mut asks: Vec<AggregatedLevel> = ask_map.into_values().collect();

        // Sort bids descending by price, asks ascending by price
        bids.sort_by(|a, b| b.price.partial_cmp(&a.price).unwrap());
        asks.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap());

        // Limit to max_levels
        bids.truncate(max_levels);
        asks.truncate(max_levels);

        // Calculate totals and metrics
        let bid_total: f64 = bids.iter().map(|l| l.total_quantity).sum();
        let ask_total: f64 = asks.iter().map(|l| l.total_quantity).sum();
        let total = bid_total + ask_total;
        let imbalance = if total > 0.0 {
            (bid_total - ask_total) / total
        } else {
            0.0
        };

        let best_bid = bids.first().map(|l| l.price).unwrap_or(0.0);
        let best_ask = asks.first().map(|l| l.price).unwrap_or(0.0);
        let spread = best_ask - best_bid;
        let mid_price = (best_bid + best_ask) / 2.0;
        let spread_pct = if mid_price > 0.0 {
            (spread / mid_price) * 100.0
        } else {
            0.0
        };

        let timestamp = chrono::Utc::now().timestamp_millis();

        Self {
            symbol: symbol.to_string(),
            bids,
            asks,
            bid_total,
            ask_total,
            imbalance,
            best_bid,
            best_ask,
            spread,
            spread_pct,
            mid_price,
            exchange_count: exchanges.len(),
            exchanges,
            timestamp,
        }
    }

    /// Convert price to a u64 key for aggregation (preserves 4 decimal places).
    fn price_to_key(price: f64) -> u64 {
        (price * 10000.0).round() as u64
    }

    /// Create an empty order book for when no data is available.
    pub fn empty(symbol: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            bids: Vec::new(),
            asks: Vec::new(),
            bid_total: 0.0,
            ask_total: 0.0,
            imbalance: 0.0,
            best_bid: 0.0,
            best_ask: 0.0,
            spread: 0.0,
            spread_pct: 0.0,
            mid_price: 0.0,
            exchange_count: 0,
            exchanges: Vec::new(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}
