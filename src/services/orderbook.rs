//! Order Book Service
//!
//! Fetches and aggregates order book data from multiple exchanges.

use crate::types::{AggregatedOrderBook, ExchangeOrderBook, OrderBookLevel, PriceSource};
use dashmap::DashMap;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

/// Cache TTL for order book data (2 seconds for faster updates)
const CACHE_TTL_MS: u64 = 2000;

/// Default number of depth levels to fetch
const DEFAULT_DEPTH: usize = 50;

/// Maximum depth levels to return
const MAX_DEPTH: usize = 100;

// ============================================================================
// Exchange symbol mappings
// ============================================================================

fn get_binance_pair(symbol: &str) -> Option<&'static str> {
    match symbol {
        "btc" => Some("BTCUSDT"),
        "eth" => Some("ETHUSDT"),
        "bnb" => Some("BNBUSDT"),
        "sol" => Some("SOLUSDT"),
        "xrp" => Some("XRPUSDT"),
        "doge" => Some("DOGEUSDT"),
        "ada" => Some("ADAUSDT"),
        "avax" => Some("AVAXUSDT"),
        "dot" => Some("DOTUSDT"),
        "link" => Some("LINKUSDT"),
        "matic" => Some("MATICUSDT"),
        "shib" => Some("SHIBUSDT"),
        "ltc" => Some("LTCUSDT"),
        "atom" => Some("ATOMUSDT"),
        "uni" => Some("UNIUSDT"),
        "xlm" => Some("XLMUSDT"),
        "bch" => Some("BCHUSDT"),
        "near" => Some("NEARUSDT"),
        "apt" => Some("APTUSDT"),
        _ => None,
    }
}

fn get_kraken_pair(symbol: &str) -> Option<&'static str> {
    match symbol {
        "btc" => Some("XXBTZUSD"),
        "eth" => Some("XETHZUSD"),
        "sol" => Some("SOLUSD"),
        "xrp" => Some("XXRPZUSD"),
        "doge" => Some("XDGUSD"),
        "ada" => Some("ADAUSD"),
        "avax" => Some("AVAXUSD"),
        "dot" => Some("DOTUSD"),
        "link" => Some("LINKUSD"),
        "matic" => Some("MATICUSD"),
        "ltc" => Some("XLTCZUSD"),
        "atom" => Some("ATOMUSD"),
        "uni" => Some("UNIUSD"),
        "xlm" => Some("XXLMZUSD"),
        "bch" => Some("BCHUSD"),
        "near" => Some("NEARUSD"),
        _ => None,
    }
}

fn get_kucoin_pair(symbol: &str) -> Option<String> {
    match symbol {
        "btc" | "eth" | "sol" | "xrp" | "doge" | "ada" | "avax" | "dot" | "link" |
        "matic" | "shib" | "ltc" | "trx" | "atom" | "uni" | "xlm" | "bch" | "near" | "apt" => {
            Some(format!("{}-USDT", symbol.to_uppercase()))
        }
        _ => None,
    }
}

fn get_okx_pair(symbol: &str) -> Option<String> {
    match symbol {
        "btc" | "eth" | "sol" | "xrp" | "doge" | "ada" | "avax" | "dot" | "link" |
        "matic" | "shib" | "ltc" | "trx" | "atom" | "uni" | "xlm" | "bch" | "near" | "apt" => {
            Some(format!("{}-USDT", symbol.to_uppercase()))
        }
        _ => None,
    }
}

fn get_huobi_pair(symbol: &str) -> Option<String> {
    match symbol {
        "btc" | "eth" | "sol" | "xrp" | "doge" | "ada" | "avax" | "dot" | "link" |
        "matic" | "shib" | "ltc" | "trx" | "atom" | "uni" | "xlm" | "bch" | "near" | "apt" => {
            Some(format!("{}usdt", symbol.to_lowercase()))
        }
        _ => None,
    }
}

fn get_hyperliquid_pair(symbol: &str) -> Option<&'static str> {
    // Hyperliquid uses uppercase symbols without suffix
    match symbol {
        "btc" => Some("BTC"),
        "eth" => Some("ETH"),
        "sol" => Some("SOL"),
        "xrp" => Some("XRP"),
        "doge" => Some("DOGE"),
        "ada" => Some("ADA"),
        "avax" => Some("AVAX"),
        "dot" => Some("DOT"),
        "link" => Some("LINK"),
        "matic" => Some("MATIC"),
        "ltc" => Some("LTC"),
        "atom" => Some("ATOM"),
        "uni" => Some("UNI"),
        "bch" => Some("BCH"),
        "near" => Some("NEAR"),
        "apt" => Some("APT"),
        "arb" => Some("ARB"),
        "op" => Some("OP"),
        "sui" => Some("SUI"),
        "sei" => Some("SEI"),
        "inj" => Some("INJ"),
        "jup" => Some("JUP"),
        "wif" => Some("WIF"),
        "pepe" => Some("PEPE"),
        _ => None,
    }
}

fn get_coinbase_pair(symbol: &str) -> Option<&'static str> {
    match symbol {
        "btc" => Some("BTC-USD"),
        "eth" => Some("ETH-USD"),
        "sol" => Some("SOL-USD"),
        "xrp" => Some("XRP-USD"),
        "doge" => Some("DOGE-USD"),
        "ada" => Some("ADA-USD"),
        "avax" => Some("AVAX-USD"),
        "dot" => Some("DOT-USD"),
        "link" => Some("LINK-USD"),
        "matic" => Some("MATIC-USD"),
        "shib" => Some("SHIB-USD"),
        "ltc" => Some("LTC-USD"),
        "atom" => Some("ATOM-USD"),
        "uni" => Some("UNI-USD"),
        "xlm" => Some("XLM-USD"),
        "bch" => Some("BCH-USD"),
        "near" => Some("NEAR-USD"),
        "apt" => Some("APT-USD"),
        "aave" => Some("AAVE-USD"),
        "mkr" => Some("MKR-USD"),
        "comp" => Some("COMP-USD"),
        "grt" => Some("GRT-USD"),
        "fil" => Some("FIL-USD"),
        "algo" => Some("ALGO-USD"),
        "eos" => Some("EOS-USD"),
        _ => None,
    }
}

// ============================================================================
// Exchange response types
// ============================================================================

/// Binance depth response
#[derive(Debug, Deserialize)]
struct BinanceDepth {
    #[serde(rename = "lastUpdateId")]
    last_update_id: u64,
    bids: Vec<Vec<String>>,
    asks: Vec<Vec<String>>,
}

/// Kraken depth response
#[derive(Debug, Deserialize)]
struct KrakenDepthResponse {
    error: Vec<String>,
    result: Option<HashMap<String, KrakenDepth>>,
}

#[derive(Debug, Deserialize)]
struct KrakenDepth {
    asks: Vec<Vec<serde_json::Value>>,
    bids: Vec<Vec<serde_json::Value>>,
}

/// KuCoin depth response
#[derive(Debug, Deserialize)]
struct KuCoinDepthResponse {
    code: String,
    data: Option<KuCoinDepth>,
}

#[derive(Debug, Deserialize)]
struct KuCoinDepth {
    sequence: String,
    bids: Vec<Vec<String>>,
    asks: Vec<Vec<String>>,
}

/// OKX depth response
#[derive(Debug, Deserialize)]
struct OkxDepthResponse {
    code: String,
    data: Option<Vec<OkxDepth>>,
}

#[derive(Debug, Deserialize)]
struct OkxDepth {
    asks: Vec<Vec<String>>,
    bids: Vec<Vec<String>>,
    ts: String,
}

/// Huobi depth response
#[derive(Debug, Deserialize)]
struct HuobiDepthResponse {
    status: String,
    tick: Option<HuobiDepth>,
}

#[derive(Debug, Deserialize)]
struct HuobiDepth {
    bids: Vec<Vec<f64>>,
    asks: Vec<Vec<f64>>,
}

/// Coinbase depth response (level 2 book has [price, size, num_orders])
#[derive(Debug, Deserialize)]
struct CoinbaseDepth {
    bids: Vec<Vec<serde_json::Value>>,
    asks: Vec<Vec<serde_json::Value>>,
}

/// Hyperliquid L2 book response
#[derive(Debug, Deserialize)]
struct HyperliquidL2Response {
    levels: Vec<Vec<HyperliquidLevel>>,
}

#[derive(Debug, Deserialize)]
struct HyperliquidLevel {
    px: String,  // price
    sz: String,  // size
    n: u32,      // number of orders
}

// ============================================================================
// Cache entry
// ============================================================================

struct CacheEntry {
    book: AggregatedOrderBook,
    created_at: Instant,
}

// ============================================================================
// Order Book Service
// ============================================================================

/// Service for fetching and aggregating order book data.
pub struct OrderBookService {
    client: Client,
    cache: DashMap<String, CacheEntry>,
}

impl OrderBookService {
    /// Create a new order book service.
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("Haunt/1.0")
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            cache: DashMap::new(),
        }
    }

    /// Get aggregated order book for a symbol.
    pub async fn get_aggregated(&self, symbol: &str, depth: Option<usize>) -> AggregatedOrderBook {
        let symbol = symbol.to_lowercase();
        let depth = depth.unwrap_or(DEFAULT_DEPTH).min(MAX_DEPTH);

        // Check cache
        if let Some(entry) = self.cache.get(&symbol) {
            if entry.created_at.elapsed().as_millis() < CACHE_TTL_MS as u128 {
                return entry.book.clone();
            }
        }

        // Fetch from all exchanges in parallel
        let books = self.fetch_all_exchanges(&symbol, depth).await;

        // Aggregate
        let aggregated = AggregatedOrderBook::from_exchange_books(&symbol, books, depth);

        // Cache
        self.cache.insert(
            symbol.clone(),
            CacheEntry {
                book: aggregated.clone(),
                created_at: Instant::now(),
            },
        );

        aggregated
    }

    /// Fetch order books from all exchanges in parallel.
    async fn fetch_all_exchanges(&self, symbol: &str, depth: usize) -> Vec<ExchangeOrderBook> {
        let (coinbase, kraken, kucoin, okx, huobi, hyperliquid) = tokio::join!(
            self.fetch_coinbase(symbol, depth),
            self.fetch_kraken(symbol, depth),
            self.fetch_kucoin(symbol, depth),
            self.fetch_okx(symbol, depth),
            self.fetch_huobi(symbol, depth),
            self.fetch_hyperliquid(symbol, depth),
        );

        // Track which exchanges returned data before moving
        let has_coinbase = coinbase.is_some();
        let has_kraken = kraken.is_some();
        let has_kucoin = kucoin.is_some();
        let has_okx = okx.is_some();
        let has_huobi = huobi.is_some();
        let has_hyperliquid = hyperliquid.is_some();

        let mut books = Vec::new();
        if let Some(b) = coinbase { books.push(b); }
        if let Some(b) = kraken { books.push(b); }
        if let Some(b) = kucoin { books.push(b); }
        if let Some(b) = okx { books.push(b); }
        if let Some(b) = huobi { books.push(b); }
        if let Some(b) = hyperliquid { books.push(b); }

        debug!(
            "Fetched order books for {}: {} exchanges (Coinbase: {}, Kraken: {}, KuCoin: {}, OKX: {}, Huobi: {}, Hyperliquid: {})",
            symbol,
            books.len(),
            has_coinbase,
            has_kraken,
            has_kucoin,
            has_okx,
            has_huobi,
            has_hyperliquid
        );

        books
    }

    /// Fetch order book from Coinbase.
    async fn fetch_coinbase(&self, symbol: &str, depth: usize) -> Option<ExchangeOrderBook> {
        let pair = get_coinbase_pair(symbol)?;
        let url = format!(
            "https://api.exchange.coinbase.com/products/{}/book?level=2",
            pair
        );

        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<CoinbaseDepth>().await {
                    Ok(data) => {
                        let bids: Vec<OrderBookLevel> = data.bids.iter().filter_map(|l| {
                            let price: f64 = l.get(0)?.as_str()?.parse().ok()?;
                            let quantity: f64 = l.get(1)?.as_str()?.parse().ok()?;
                            Some(OrderBookLevel { price, quantity })
                        }).take(depth).collect();
                        let asks: Vec<OrderBookLevel> = data.asks.iter().filter_map(|l| {
                            let price: f64 = l.get(0)?.as_str()?.parse().ok()?;
                            let quantity: f64 = l.get(1)?.as_str()?.parse().ok()?;
                            Some(OrderBookLevel { price, quantity })
                        }).take(depth).collect();

                        Some(ExchangeOrderBook {
                            exchange: PriceSource::Coinbase,
                            symbol: symbol.to_string(),
                            bids,
                            asks,
                            timestamp: chrono::Utc::now().timestamp_millis(),
                        })
                    }
                    Err(e) => {
                        warn!("Coinbase depth parse error: {}", e);
                        None
                    }
                }
            }
            Ok(resp) => {
                warn!("Coinbase depth error: {}", resp.status());
                None
            }
            Err(e) => {
                warn!("Coinbase depth fetch error: {}", e);
                None
            }
        }
    }

    /// Fetch order book from Kraken.
    async fn fetch_kraken(&self, symbol: &str, depth: usize) -> Option<ExchangeOrderBook> {
        let pair = get_kraken_pair(symbol)?;
        let count = depth.min(100);
        let url = format!(
            "https://api.kraken.com/0/public/Depth?pair={}&count={}",
            pair, count
        );

        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<KrakenDepthResponse>().await {
                    Ok(data) if data.error.is_empty() => {
                        if let Some(result) = data.result {
                            // Kraken returns with weird pair names, get first result
                            if let Some((_, depth_data)) = result.into_iter().next() {
                                let bids = depth_data.bids.iter().filter_map(|l| {
                                    let price: f64 = l.get(0)?.as_str()?.parse().ok()?;
                                    let quantity: f64 = l.get(1)?.as_str()?.parse().ok()?;
                                    Some(OrderBookLevel { price, quantity })
                                }).collect();
                                let asks = depth_data.asks.iter().filter_map(|l| {
                                    let price: f64 = l.get(0)?.as_str()?.parse().ok()?;
                                    let quantity: f64 = l.get(1)?.as_str()?.parse().ok()?;
                                    Some(OrderBookLevel { price, quantity })
                                }).collect();

                                return Some(ExchangeOrderBook {
                                    exchange: PriceSource::Kraken,
                                    symbol: symbol.to_string(),
                                    bids,
                                    asks,
                                    timestamp: chrono::Utc::now().timestamp_millis(),
                                });
                            }
                        }
                        None
                    }
                    Ok(data) => {
                        warn!("Kraken depth error: {:?}", data.error);
                        None
                    }
                    Err(e) => {
                        warn!("Kraken depth parse error: {}", e);
                        None
                    }
                }
            }
            Ok(resp) => {
                warn!("Kraken depth error: {}", resp.status());
                None
            }
            Err(e) => {
                warn!("Kraken depth fetch error: {}", e);
                None
            }
        }
    }

    /// Fetch order book from KuCoin.
    async fn fetch_kucoin(&self, symbol: &str, depth: usize) -> Option<ExchangeOrderBook> {
        let pair = get_kucoin_pair(symbol)?;
        // KuCoin only supports 20 or 100 levels
        let limit = if depth <= 20 { 20 } else { 100 };
        let url = format!(
            "https://api.kucoin.com/api/v1/market/orderbook/level2_{}?symbol={}",
            limit, pair
        );

        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<KuCoinDepthResponse>().await {
                    Ok(data) if data.code == "200000" => {
                        if let Some(depth_data) = data.data {
                            let bids = depth_data.bids.iter().filter_map(|l| {
                                Some(OrderBookLevel {
                                    price: l.get(0)?.parse().ok()?,
                                    quantity: l.get(1)?.parse().ok()?,
                                })
                            }).take(depth).collect();
                            let asks = depth_data.asks.iter().filter_map(|l| {
                                Some(OrderBookLevel {
                                    price: l.get(0)?.parse().ok()?,
                                    quantity: l.get(1)?.parse().ok()?,
                                })
                            }).take(depth).collect();

                            return Some(ExchangeOrderBook {
                                exchange: PriceSource::KuCoin,
                                symbol: symbol.to_string(),
                                bids,
                                asks,
                                timestamp: chrono::Utc::now().timestamp_millis(),
                            });
                        }
                        None
                    }
                    Ok(data) => {
                        warn!("KuCoin depth error: {}", data.code);
                        None
                    }
                    Err(e) => {
                        warn!("KuCoin depth parse error: {}", e);
                        None
                    }
                }
            }
            Ok(resp) => {
                warn!("KuCoin depth error: {}", resp.status());
                None
            }
            Err(e) => {
                warn!("KuCoin depth fetch error: {}", e);
                None
            }
        }
    }

    /// Fetch order book from OKX.
    async fn fetch_okx(&self, symbol: &str, depth: usize) -> Option<ExchangeOrderBook> {
        let pair = get_okx_pair(symbol)?;
        let sz = depth.min(100);
        let url = format!(
            "https://www.okx.com/api/v5/market/books?instId={}&sz={}",
            pair, sz
        );

        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<OkxDepthResponse>().await {
                    Ok(data) if data.code == "0" => {
                        if let Some(books) = data.data {
                            if let Some(depth_data) = books.into_iter().next() {
                                let bids = depth_data.bids.iter().filter_map(|l| {
                                    Some(OrderBookLevel {
                                        price: l.get(0)?.parse().ok()?,
                                        quantity: l.get(1)?.parse().ok()?,
                                    })
                                }).collect();
                                let asks = depth_data.asks.iter().filter_map(|l| {
                                    Some(OrderBookLevel {
                                        price: l.get(0)?.parse().ok()?,
                                        quantity: l.get(1)?.parse().ok()?,
                                    })
                                }).collect();

                                return Some(ExchangeOrderBook {
                                    exchange: PriceSource::Okx,
                                    symbol: symbol.to_string(),
                                    bids,
                                    asks,
                                    timestamp: chrono::Utc::now().timestamp_millis(),
                                });
                            }
                        }
                        None
                    }
                    Ok(data) => {
                        warn!("OKX depth error: {}", data.code);
                        None
                    }
                    Err(e) => {
                        warn!("OKX depth parse error: {}", e);
                        None
                    }
                }
            }
            Ok(resp) => {
                warn!("OKX depth error: {}", resp.status());
                None
            }
            Err(e) => {
                warn!("OKX depth fetch error: {}", e);
                None
            }
        }
    }

    /// Fetch order book from Huobi.
    async fn fetch_huobi(&self, symbol: &str, depth: usize) -> Option<ExchangeOrderBook> {
        let pair = get_huobi_pair(symbol)?;
        // Huobi depth types: step0 (no aggregation), step1-5 (increasing aggregation)
        // step0 gives most granular data
        let url = format!(
            "https://api.huobi.pro/market/depth?symbol={}&type=step0&depth={}",
            pair, depth.min(20)
        );

        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<HuobiDepthResponse>().await {
                    Ok(data) if data.status == "ok" => {
                        if let Some(tick) = data.tick {
                            let bids = tick.bids.iter().filter_map(|l| {
                                Some(OrderBookLevel {
                                    price: *l.get(0)?,
                                    quantity: *l.get(1)?,
                                })
                            }).take(depth).collect();
                            let asks = tick.asks.iter().filter_map(|l| {
                                Some(OrderBookLevel {
                                    price: *l.get(0)?,
                                    quantity: *l.get(1)?,
                                })
                            }).take(depth).collect();

                            return Some(ExchangeOrderBook {
                                exchange: PriceSource::Huobi,
                                symbol: symbol.to_string(),
                                bids,
                                asks,
                                timestamp: chrono::Utc::now().timestamp_millis(),
                            });
                        }
                        None
                    }
                    Ok(data) => {
                        warn!("Huobi depth error: {}", data.status);
                        None
                    }
                    Err(e) => {
                        warn!("Huobi depth parse error: {}", e);
                        None
                    }
                }
            }
            Ok(resp) => {
                warn!("Huobi depth error: {}", resp.status());
                None
            }
            Err(e) => {
                warn!("Huobi depth fetch error: {}", e);
                None
            }
        }
    }

    /// Fetch order book from Hyperliquid.
    async fn fetch_hyperliquid(&self, symbol: &str, depth: usize) -> Option<ExchangeOrderBook> {
        let coin = get_hyperliquid_pair(symbol)?;
        let url = "https://api.hyperliquid.xyz/info";

        // Hyperliquid uses POST with JSON body
        let body = serde_json::json!({
            "type": "l2Book",
            "coin": coin
        });

        match self.client.post(url).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<HyperliquidL2Response>().await {
                    Ok(data) => {
                        // Hyperliquid returns levels[0] = bids, levels[1] = asks
                        let bids: Vec<OrderBookLevel> = data.levels
                            .get(0)
                            .map(|levels| {
                                levels.iter().filter_map(|l| {
                                    Some(OrderBookLevel {
                                        price: l.px.parse().ok()?,
                                        quantity: l.sz.parse().ok()?,
                                    })
                                }).take(depth).collect()
                            })
                            .unwrap_or_default();

                        let asks: Vec<OrderBookLevel> = data.levels
                            .get(1)
                            .map(|levels| {
                                levels.iter().filter_map(|l| {
                                    Some(OrderBookLevel {
                                        price: l.px.parse().ok()?,
                                        quantity: l.sz.parse().ok()?,
                                    })
                                }).take(depth).collect()
                            })
                            .unwrap_or_default();

                        Some(ExchangeOrderBook {
                            exchange: PriceSource::Hyperliquid,
                            symbol: symbol.to_string(),
                            bids,
                            asks,
                            timestamp: chrono::Utc::now().timestamp_millis(),
                        })
                    }
                    Err(e) => {
                        warn!("Hyperliquid depth parse error: {}", e);
                        None
                    }
                }
            }
            Ok(resp) => {
                warn!("Hyperliquid depth error: {}", resp.status());
                None
            }
            Err(e) => {
                warn!("Hyperliquid depth fetch error: {}", e);
                None
            }
        }
    }
}

impl Default for OrderBookService {
    fn default() -> Self {
        Self::new()
    }
}
