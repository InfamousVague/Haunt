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

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // OrderBookLevel Tests
    // =========================================================================

    #[test]
    fn test_order_book_level_creation() {
        let level = OrderBookLevel {
            price: 50000.0,
            quantity: 1.5,
        };

        assert_eq!(level.price, 50000.0);
        assert_eq!(level.quantity, 1.5);
    }

    #[test]
    fn test_order_book_level_serialization() {
        let level = OrderBookLevel {
            price: 50000.0,
            quantity: 1.5,
        };

        let json = serde_json::to_string(&level).unwrap();
        assert!(json.contains("\"price\":50000"));
        assert!(json.contains("\"quantity\":1.5"));

        let parsed: OrderBookLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.price, level.price);
        assert_eq!(parsed.quantity, level.quantity);
    }

    // =========================================================================
    // ExchangeOrderBook Tests
    // =========================================================================

    #[test]
    fn test_exchange_order_book_creation() {
        let book = ExchangeOrderBook {
            exchange: PriceSource::Coinbase,
            symbol: "BTC".to_string(),
            bids: vec![
                OrderBookLevel {
                    price: 50000.0,
                    quantity: 1.0,
                },
                OrderBookLevel {
                    price: 49999.0,
                    quantity: 2.0,
                },
            ],
            asks: vec![
                OrderBookLevel {
                    price: 50001.0,
                    quantity: 1.5,
                },
                OrderBookLevel {
                    price: 50002.0,
                    quantity: 2.5,
                },
            ],
            timestamp: 1704067200000,
        };

        assert_eq!(book.exchange, PriceSource::Coinbase);
        assert_eq!(book.symbol, "BTC");
        assert_eq!(book.bids.len(), 2);
        assert_eq!(book.asks.len(), 2);
    }

    // =========================================================================
    // AggregatedLevel Tests
    // =========================================================================

    #[test]
    fn test_aggregated_level_creation() {
        let mut exchanges = HashMap::new();
        exchanges.insert("coinbase".to_string(), 1.0);
        exchanges.insert("binance".to_string(), 0.5);

        let level = AggregatedLevel {
            price: 50000.0,
            total_quantity: 1.5,
            exchanges,
        };

        assert_eq!(level.price, 50000.0);
        assert_eq!(level.total_quantity, 1.5);
        assert_eq!(level.exchanges.len(), 2);
    }

    #[test]
    fn test_aggregated_level_serialization() {
        let mut exchanges = HashMap::new();
        exchanges.insert("coinbase".to_string(), 1.0);

        let level = AggregatedLevel {
            price: 50000.0,
            total_quantity: 1.0,
            exchanges,
        };

        let json = serde_json::to_string(&level).unwrap();
        assert!(json.contains("\"price\":50000"));
        assert!(json.contains("\"totalQuantity\":1"));
    }

    // =========================================================================
    // AggregatedOrderBook Tests
    // =========================================================================

    #[test]
    fn test_aggregated_order_book_empty() {
        let book = AggregatedOrderBook::empty("BTC");

        assert_eq!(book.symbol, "BTC");
        assert!(book.bids.is_empty());
        assert!(book.asks.is_empty());
        assert_eq!(book.bid_total, 0.0);
        assert_eq!(book.ask_total, 0.0);
        assert_eq!(book.imbalance, 0.0);
        assert_eq!(book.exchange_count, 0);
    }

    #[test]
    fn test_aggregated_order_book_from_single_exchange() {
        let book = ExchangeOrderBook {
            exchange: PriceSource::Coinbase,
            symbol: "BTC".to_string(),
            bids: vec![
                OrderBookLevel {
                    price: 50000.0,
                    quantity: 1.0,
                },
                OrderBookLevel {
                    price: 49999.0,
                    quantity: 2.0,
                },
            ],
            asks: vec![
                OrderBookLevel {
                    price: 50001.0,
                    quantity: 1.0,
                },
                OrderBookLevel {
                    price: 50002.0,
                    quantity: 2.0,
                },
            ],
            timestamp: 1704067200000,
        };

        let aggregated = AggregatedOrderBook::from_exchange_books("BTC", vec![book], 10);

        assert_eq!(aggregated.symbol, "BTC");
        assert_eq!(aggregated.exchange_count, 1);
        assert_eq!(aggregated.exchanges, vec!["coinbase"]);
        assert_eq!(aggregated.bid_total, 3.0);
        assert_eq!(aggregated.ask_total, 3.0);
        assert_eq!(aggregated.best_bid, 50000.0);
        assert_eq!(aggregated.best_ask, 50001.0);
        assert_eq!(aggregated.spread, 1.0);
        assert_eq!(aggregated.mid_price, 50000.5);
    }

    #[test]
    fn test_aggregated_order_book_from_multiple_exchanges() {
        let book1 = ExchangeOrderBook {
            exchange: PriceSource::Coinbase,
            symbol: "BTC".to_string(),
            bids: vec![OrderBookLevel {
                price: 50000.0,
                quantity: 1.0,
            }],
            asks: vec![OrderBookLevel {
                price: 50001.0,
                quantity: 1.0,
            }],
            timestamp: 1704067200000,
        };

        let book2 = ExchangeOrderBook {
            exchange: PriceSource::Binance,
            symbol: "BTC".to_string(),
            bids: vec![
                OrderBookLevel {
                    price: 50000.0,
                    quantity: 2.0,
                }, // Same price level
            ],
            asks: vec![
                OrderBookLevel {
                    price: 50001.0,
                    quantity: 2.0,
                }, // Same price level
            ],
            timestamp: 1704067200000,
        };

        let aggregated = AggregatedOrderBook::from_exchange_books("BTC", vec![book1, book2], 10);

        assert_eq!(aggregated.exchange_count, 2);
        assert_eq!(aggregated.bid_total, 3.0); // 1.0 + 2.0 aggregated
        assert_eq!(aggregated.ask_total, 3.0);

        // Check exchange breakdown in aggregated level
        let bid_level = &aggregated.bids[0];
        assert_eq!(bid_level.total_quantity, 3.0);
        assert_eq!(bid_level.exchanges.get("coinbase"), Some(&1.0));
        assert_eq!(bid_level.exchanges.get("binance"), Some(&2.0));
    }

    #[test]
    fn test_aggregated_order_book_imbalance_calculation() {
        // Create bid-heavy book
        let book = ExchangeOrderBook {
            exchange: PriceSource::Coinbase,
            symbol: "BTC".to_string(),
            bids: vec![OrderBookLevel {
                price: 50000.0,
                quantity: 10.0,
            }],
            asks: vec![OrderBookLevel {
                price: 50001.0,
                quantity: 2.0,
            }],
            timestamp: 1704067200000,
        };

        let aggregated = AggregatedOrderBook::from_exchange_books("BTC", vec![book], 10);

        // Imbalance = (10 - 2) / (10 + 2) = 8/12 = 0.666...
        assert!((aggregated.imbalance - 0.6666).abs() < 0.01);
    }

    #[test]
    fn test_aggregated_order_book_max_levels() {
        let bids: Vec<OrderBookLevel> = (0..20)
            .map(|i| OrderBookLevel {
                price: 50000.0 - i as f64,
                quantity: 1.0,
            })
            .collect();

        let asks: Vec<OrderBookLevel> = (0..20)
            .map(|i| OrderBookLevel {
                price: 50001.0 + i as f64,
                quantity: 1.0,
            })
            .collect();

        let book = ExchangeOrderBook {
            exchange: PriceSource::Coinbase,
            symbol: "BTC".to_string(),
            bids,
            asks,
            timestamp: 1704067200000,
        };

        let aggregated = AggregatedOrderBook::from_exchange_books("BTC", vec![book], 10);

        assert_eq!(aggregated.bids.len(), 10);
        assert_eq!(aggregated.asks.len(), 10);
    }

    #[test]
    fn test_aggregated_order_book_spread_percentage() {
        let book = ExchangeOrderBook {
            exchange: PriceSource::Coinbase,
            symbol: "BTC".to_string(),
            bids: vec![OrderBookLevel {
                price: 100.0,
                quantity: 1.0,
            }],
            asks: vec![OrderBookLevel {
                price: 101.0,
                quantity: 1.0,
            }],
            timestamp: 1704067200000,
        };

        let aggregated = AggregatedOrderBook::from_exchange_books("TEST", vec![book], 10);

        // Spread = 1.0, mid_price = 100.5, spread_pct = (1.0 / 100.5) * 100 ≈ 0.995%
        assert!((aggregated.spread_pct - 0.995).abs() < 0.01);
    }

    #[test]
    fn test_aggregated_order_book_bids_sorted_descending() {
        let book = ExchangeOrderBook {
            exchange: PriceSource::Coinbase,
            symbol: "BTC".to_string(),
            bids: vec![
                OrderBookLevel {
                    price: 49998.0,
                    quantity: 1.0,
                },
                OrderBookLevel {
                    price: 50000.0,
                    quantity: 1.0,
                },
                OrderBookLevel {
                    price: 49999.0,
                    quantity: 1.0,
                },
            ],
            asks: vec![],
            timestamp: 1704067200000,
        };

        let aggregated = AggregatedOrderBook::from_exchange_books("BTC", vec![book], 10);

        assert_eq!(aggregated.bids[0].price, 50000.0); // Highest first
        assert_eq!(aggregated.bids[1].price, 49999.0);
        assert_eq!(aggregated.bids[2].price, 49998.0);
    }

    #[test]
    fn test_aggregated_order_book_asks_sorted_ascending() {
        let book = ExchangeOrderBook {
            exchange: PriceSource::Coinbase,
            symbol: "BTC".to_string(),
            bids: vec![],
            asks: vec![
                OrderBookLevel {
                    price: 50003.0,
                    quantity: 1.0,
                },
                OrderBookLevel {
                    price: 50001.0,
                    quantity: 1.0,
                },
                OrderBookLevel {
                    price: 50002.0,
                    quantity: 1.0,
                },
            ],
            timestamp: 1704067200000,
        };

        let aggregated = AggregatedOrderBook::from_exchange_books("BTC", vec![book], 10);

        assert_eq!(aggregated.asks[0].price, 50001.0); // Lowest first
        assert_eq!(aggregated.asks[1].price, 50002.0);
        assert_eq!(aggregated.asks[2].price, 50003.0);
    }

    #[test]
    fn test_aggregated_order_book_serialization() {
        let book = AggregatedOrderBook::empty("ETH");

        let json = serde_json::to_string(&book).unwrap();
        assert!(json.contains("\"symbol\":\"ETH\""));
        assert!(json.contains("\"bidTotal\":0"));
        assert!(json.contains("\"askTotal\":0"));
        assert!(json.contains("\"imbalance\":0"));
    }

    #[test]
    fn test_price_to_key() {
        // Test that price_to_key preserves 4 decimal places
        assert_eq!(AggregatedOrderBook::price_to_key(1.0), 10000);
        assert_eq!(AggregatedOrderBook::price_to_key(1.5), 15000);
        assert_eq!(AggregatedOrderBook::price_to_key(1.2345), 12345);
        assert_eq!(AggregatedOrderBook::price_to_key(50000.0), 500000000);
    }
}
