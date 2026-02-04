//! API Comparison Tests
//!
//! These tests compare our API values against external APIs (CoinMarketCap, CoinGecko)
//! to ensure our aggregated data doesn't deviate significantly from industry standards.
//!
//! Run with: cargo test --test api_comparison_test -- --ignored --nocapture
//!
//! Note: These tests require network access and are marked as #[ignore] by default
//! since they depend on external services.

use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

/// Acceptable price deviation percentage (5%)
const PRICE_DEVIATION_THRESHOLD: f64 = 5.0;

/// Acceptable volume deviation percentage (20% - volume varies more across sources)
const VOLUME_DEVIATION_THRESHOLD: f64 = 20.0;

/// Acceptable percent change deviation (absolute difference in percentage points)
const PERCENT_CHANGE_DEVIATION_THRESHOLD: f64 = 2.0;

/// Acceptable market cap deviation percentage (10%)
const MARKET_CAP_DEVIATION_THRESHOLD: f64 = 10.0;

/// Our API base URL (local development)
const HAUNT_API_URL: &str = "http://localhost:3001/api";

/// CoinGecko API base URL
const COINGECKO_API_URL: &str = "https://api.coingecko.com/api/v3";

/// CryptoCompare API base URL
const CRYPTOCOMPARE_API_URL: &str = "https://min-api.cryptocompare.com/data";

// ============================================================================
// Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct HauntApiResponse<T> {
    data: T,
    meta: HauntApiMeta,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct HauntApiMeta {
    cached: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct HauntListing {
    id: i64,
    rank: i32,
    name: String,
    symbol: String,
    price: f64,
    change_1h: f64,
    change_24h: f64,
    change_7d: f64,
    market_cap: f64,
    volume_24h: f64,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CoinGeckoPrice {
    usd: f64,
    #[serde(default)]
    usd_24h_vol: Option<f64>,
    #[serde(default)]
    usd_24h_change: Option<f64>,
    #[serde(default)]
    usd_market_cap: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CryptoCompareMultiPrice {
    #[serde(rename = "USD")]
    usd: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
#[allow(dead_code)]
struct CryptoComparePriceFull {
    usd: CryptoCompareFullData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(dead_code)]
struct CryptoCompareFullData {
    price: f64,
    #[serde(rename = "VOLUME24HOURTO")]
    volume_24h_to: f64,
    #[serde(rename = "CHANGEPCT24HOUR")]
    change_pct_24h: f64,
    #[serde(rename = "MKTCAP")]
    market_cap: f64,
}

// ============================================================================
// Test Utilities
// ============================================================================

fn create_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Haunt-API-Comparison-Test/1.0")
        .build()
        .expect("Failed to create HTTP client")
}

/// Calculate percentage deviation between two values
fn calculate_deviation(our_value: f64, reference_value: f64) -> f64 {
    if reference_value == 0.0 {
        if our_value == 0.0 {
            return 0.0;
        }
        return 100.0; // Infinite deviation
    }
    ((our_value - reference_value).abs() / reference_value.abs()) * 100.0
}

/// Calculate absolute deviation for percentage values
fn calculate_percent_deviation(our_value: f64, reference_value: f64) -> f64 {
    (our_value - reference_value).abs()
}

// ============================================================================
// Comparison Result Types
// ============================================================================

#[derive(Debug)]
struct PriceComparison {
    symbol: String,
    our_price: f64,
    reference_price: f64,
    deviation_percent: f64,
    passed: bool,
}

#[derive(Debug)]
struct VolumeComparison {
    symbol: String,
    our_volume: f64,
    reference_volume: f64,
    deviation_percent: f64,
    passed: bool,
}

#[derive(Debug)]
struct PercentChangeComparison {
    symbol: String,
    our_change: f64,
    reference_change: f64,
    deviation: f64,
    passed: bool,
}

#[derive(Debug)]
struct ComparisonReport {
    source: String,
    total_tests: usize,
    passed_tests: usize,
    failed_tests: usize,
    price_comparisons: Vec<PriceComparison>,
    volume_comparisons: Vec<VolumeComparison>,
    percent_change_comparisons: Vec<PercentChangeComparison>,
}

impl ComparisonReport {
    fn new(source: &str) -> Self {
        Self {
            source: source.to_string(),
            total_tests: 0,
            passed_tests: 0,
            failed_tests: 0,
            price_comparisons: Vec::new(),
            volume_comparisons: Vec::new(),
            percent_change_comparisons: Vec::new(),
        }
    }

    fn add_price_comparison(&mut self, comparison: PriceComparison) {
        self.total_tests += 1;
        if comparison.passed {
            self.passed_tests += 1;
        } else {
            self.failed_tests += 1;
        }
        self.price_comparisons.push(comparison);
    }

    fn add_volume_comparison(&mut self, comparison: VolumeComparison) {
        self.total_tests += 1;
        if comparison.passed {
            self.passed_tests += 1;
        } else {
            self.failed_tests += 1;
        }
        self.volume_comparisons.push(comparison);
    }

    fn add_percent_change_comparison(&mut self, comparison: PercentChangeComparison) {
        self.total_tests += 1;
        if comparison.passed {
            self.passed_tests += 1;
        } else {
            self.failed_tests += 1;
        }
        self.percent_change_comparisons.push(comparison);
    }

    fn print_summary(&self) {
        println!("\n========================================");
        println!("Comparison Report: {}", self.source);
        println!("========================================");
        println!("Total tests: {}", self.total_tests);
        println!(
            "Passed: {} ({:.1}%)",
            self.passed_tests,
            if self.total_tests > 0 {
                (self.passed_tests as f64 / self.total_tests as f64) * 100.0
            } else {
                0.0
            }
        );
        println!("Failed: {}", self.failed_tests);

        if !self.price_comparisons.is_empty() {
            println!("\n--- Price Comparisons ---");
            for cmp in &self.price_comparisons {
                let status = if cmp.passed { "✓" } else { "✗" };
                println!(
                    "{} {}: ${:.4} vs ${:.4} (deviation: {:.2}%)",
                    status, cmp.symbol, cmp.our_price, cmp.reference_price, cmp.deviation_percent
                );
            }
        }

        if !self.volume_comparisons.is_empty() {
            println!("\n--- Volume Comparisons ---");
            for cmp in &self.volume_comparisons {
                let status = if cmp.passed { "✓" } else { "✗" };
                println!(
                    "{} {}: ${:.0} vs ${:.0} (deviation: {:.2}%)",
                    status, cmp.symbol, cmp.our_volume, cmp.reference_volume, cmp.deviation_percent
                );
            }
        }

        if !self.percent_change_comparisons.is_empty() {
            println!("\n--- 24h Change Comparisons ---");
            for cmp in &self.percent_change_comparisons {
                let status = if cmp.passed { "✓" } else { "✗" };
                println!(
                    "{} {}: {:.2}% vs {:.2}% (deviation: {:.2} pp)",
                    status, cmp.symbol, cmp.our_change, cmp.reference_change, cmp.deviation
                );
            }
        }

        println!("\n========================================\n");
    }
}

// ============================================================================
// Tests
// ============================================================================

/// Test that our API is running and responding
#[tokio::test]
#[ignore]
async fn test_haunt_api_health() {
    let client = create_client();

    let resp = client.get(format!("{}/health", HAUNT_API_URL)).send().await;

    match resp {
        Ok(r) => {
            assert!(r.status().is_success(), "Haunt API should be healthy");
            println!("✓ Haunt API is running and healthy");
        }
        Err(e) => {
            panic!("Haunt API is not reachable at {}: {}\nMake sure the server is running with: cargo run", HAUNT_API_URL, e);
        }
    }
}

/// Compare our prices against CoinGecko
#[tokio::test]
#[ignore]
async fn test_price_comparison_coingecko() {
    let client = create_client();
    let symbols = ["btc", "eth", "bnb", "xrp", "sol"];
    let coingecko_ids = ["bitcoin", "ethereum", "binancecoin", "ripple", "solana"];

    let mut report = ComparisonReport::new("CoinGecko");

    // Fetch our data
    let our_resp: HauntApiResponse<Vec<HauntListing>> = client
        .get(format!("{}/crypto/listings?limit=100", HAUNT_API_URL))
        .send()
        .await
        .expect("Failed to fetch from Haunt API")
        .json()
        .await
        .expect("Failed to parse Haunt response");

    // Create a map of our prices by symbol
    let our_prices: HashMap<String, &HauntListing> = our_resp
        .data
        .iter()
        .map(|l| (l.symbol.to_lowercase(), l))
        .collect();

    // Fetch CoinGecko prices
    let ids_param = coingecko_ids.join(",");
    let cg_url = format!(
        "{}/simple/price?ids={}&vs_currencies=usd&include_24hr_vol=true&include_24hr_change=true&include_market_cap=true",
        COINGECKO_API_URL, ids_param
    );

    let cg_resp: HashMap<String, CoinGeckoPrice> = client
        .get(&cg_url)
        .send()
        .await
        .expect("Failed to fetch from CoinGecko API")
        .json()
        .await
        .expect("Failed to parse CoinGecko response");

    // Compare prices
    for (i, symbol) in symbols.iter().enumerate() {
        let cg_id = coingecko_ids[i];

        if let (Some(our_listing), Some(cg_price)) = (our_prices.get(*symbol), cg_resp.get(cg_id)) {
            // Price comparison
            let price_deviation = calculate_deviation(our_listing.price, cg_price.usd);
            report.add_price_comparison(PriceComparison {
                symbol: symbol.to_uppercase(),
                our_price: our_listing.price,
                reference_price: cg_price.usd,
                deviation_percent: price_deviation,
                passed: price_deviation <= PRICE_DEVIATION_THRESHOLD,
            });

            // Volume comparison (if available)
            if let Some(cg_volume) = cg_price.usd_24h_vol {
                let volume_deviation = calculate_deviation(our_listing.volume_24h, cg_volume);
                report.add_volume_comparison(VolumeComparison {
                    symbol: symbol.to_uppercase(),
                    our_volume: our_listing.volume_24h,
                    reference_volume: cg_volume,
                    deviation_percent: volume_deviation,
                    passed: volume_deviation <= VOLUME_DEVIATION_THRESHOLD,
                });
            }

            // 24h change comparison (if available)
            if let Some(cg_change) = cg_price.usd_24h_change {
                let change_deviation =
                    calculate_percent_deviation(our_listing.change_24h, cg_change);
                report.add_percent_change_comparison(PercentChangeComparison {
                    symbol: symbol.to_uppercase(),
                    our_change: our_listing.change_24h,
                    reference_change: cg_change,
                    deviation: change_deviation,
                    passed: change_deviation <= PERCENT_CHANGE_DEVIATION_THRESHOLD,
                });
            }
        }
    }

    report.print_summary();

    // Test passes if >80% of comparisons pass
    let pass_rate = if report.total_tests > 0 {
        (report.passed_tests as f64 / report.total_tests as f64) * 100.0
    } else {
        0.0
    };

    assert!(
        pass_rate >= 80.0,
        "CoinGecko comparison pass rate ({:.1}%) is below 80%",
        pass_rate
    );
}

/// Compare our prices against CryptoCompare
#[tokio::test]
#[ignore]
async fn test_price_comparison_cryptocompare() {
    let client = create_client();
    let symbols = ["BTC", "ETH", "BNB", "XRP", "SOL"];

    let mut report = ComparisonReport::new("CryptoCompare");

    // Fetch our data
    let our_resp: HauntApiResponse<Vec<HauntListing>> = client
        .get(format!("{}/crypto/listings?limit=100", HAUNT_API_URL))
        .send()
        .await
        .expect("Failed to fetch from Haunt API")
        .json()
        .await
        .expect("Failed to parse Haunt response");

    // Create a map of our prices by symbol
    let our_prices: HashMap<String, &HauntListing> = our_resp
        .data
        .iter()
        .map(|l| (l.symbol.to_lowercase(), l))
        .collect();

    // Fetch CryptoCompare prices
    let fsyms = symbols.join(",");
    let cc_url = format!(
        "{}/pricemultifull?fsyms={}&tsyms=USD",
        CRYPTOCOMPARE_API_URL, fsyms
    );

    #[derive(Deserialize)]
    struct CryptoCompareFullResponse {
        #[serde(rename = "RAW")]
        raw: Option<HashMap<String, CryptoComparePriceFull>>,
    }

    let cc_resp: CryptoCompareFullResponse = client
        .get(&cc_url)
        .send()
        .await
        .expect("Failed to fetch from CryptoCompare API")
        .json()
        .await
        .expect("Failed to parse CryptoCompare response");

    if let Some(raw) = cc_resp.raw {
        for symbol in &symbols {
            let symbol_lower = symbol.to_lowercase();

            if let (Some(our_listing), Some(cc_data)) =
                (our_prices.get(&symbol_lower), raw.get(*symbol))
            {
                // Price comparison
                let price_deviation = calculate_deviation(our_listing.price, cc_data.usd.price);
                report.add_price_comparison(PriceComparison {
                    symbol: symbol.to_string(),
                    our_price: our_listing.price,
                    reference_price: cc_data.usd.price,
                    deviation_percent: price_deviation,
                    passed: price_deviation <= PRICE_DEVIATION_THRESHOLD,
                });

                // Volume comparison
                let volume_deviation =
                    calculate_deviation(our_listing.volume_24h, cc_data.usd.volume_24h_to);
                report.add_volume_comparison(VolumeComparison {
                    symbol: symbol.to_string(),
                    our_volume: our_listing.volume_24h,
                    reference_volume: cc_data.usd.volume_24h_to,
                    deviation_percent: volume_deviation,
                    passed: volume_deviation <= VOLUME_DEVIATION_THRESHOLD,
                });

                // 24h change comparison
                let change_deviation =
                    calculate_percent_deviation(our_listing.change_24h, cc_data.usd.change_pct_24h);
                report.add_percent_change_comparison(PercentChangeComparison {
                    symbol: symbol.to_string(),
                    our_change: our_listing.change_24h,
                    reference_change: cc_data.usd.change_pct_24h,
                    deviation: change_deviation,
                    passed: change_deviation <= PERCENT_CHANGE_DEVIATION_THRESHOLD,
                });
            }
        }
    }

    report.print_summary();

    // Test passes if >80% of comparisons pass
    let pass_rate = if report.total_tests > 0 {
        (report.passed_tests as f64 / report.total_tests as f64) * 100.0
    } else {
        0.0
    };

    assert!(
        pass_rate >= 80.0,
        "CryptoCompare comparison pass rate ({:.1}%) is below 80%",
        pass_rate
    );
}

/// Test individual asset price accuracy
#[tokio::test]
#[ignore]
async fn test_individual_asset_price_accuracy() {
    let client = create_client();

    // Test Bitcoin (id: 1 on CMC)
    let our_resp: HauntApiResponse<HauntListing> = client
        .get(format!("{}/crypto/1", HAUNT_API_URL))
        .send()
        .await
        .expect("Failed to fetch BTC from Haunt API")
        .json()
        .await
        .expect("Failed to parse Haunt response");

    // Fetch from CoinGecko for comparison
    let cg_url = format!(
        "{}/simple/price?ids=bitcoin&vs_currencies=usd&include_market_cap=true",
        COINGECKO_API_URL
    );

    let cg_resp: HashMap<String, CoinGeckoPrice> = client
        .get(&cg_url)
        .send()
        .await
        .expect("Failed to fetch from CoinGecko")
        .json()
        .await
        .expect("Failed to parse CoinGecko response");

    let cg_btc = cg_resp
        .get("bitcoin")
        .expect("Bitcoin not found in CoinGecko response");

    let price_deviation = calculate_deviation(our_resp.data.price, cg_btc.usd);

    println!("Bitcoin Price Comparison:");
    println!("  Our price:    ${:.2}", our_resp.data.price);
    println!("  CoinGecko:    ${:.2}", cg_btc.usd);
    println!("  Deviation:    {:.2}%", price_deviation);

    assert!(
        price_deviation <= PRICE_DEVIATION_THRESHOLD,
        "Bitcoin price deviation ({:.2}%) exceeds threshold ({:.2}%)",
        price_deviation,
        PRICE_DEVIATION_THRESHOLD
    );

    // Also check market cap
    if let Some(cg_market_cap) = cg_btc.usd_market_cap {
        let market_cap_deviation = calculate_deviation(our_resp.data.market_cap, cg_market_cap);
        println!("  Our market cap:    ${:.0}", our_resp.data.market_cap);
        println!("  CoinGecko:         ${:.0}", cg_market_cap);
        println!("  Deviation:         {:.2}%", market_cap_deviation);

        assert!(
            market_cap_deviation <= MARKET_CAP_DEVIATION_THRESHOLD,
            "Bitcoin market cap deviation ({:.2}%) exceeds threshold ({:.2}%)",
            market_cap_deviation,
            MARKET_CAP_DEVIATION_THRESHOLD
        );
    }
}

/// Test that rankings are reasonable
#[tokio::test]
#[ignore]
async fn test_ranking_consistency() {
    let client = create_client();

    // Fetch our top 10
    let our_resp: HauntApiResponse<Vec<HauntListing>> = client
        .get(format!("{}/crypto/listings?limit=10", HAUNT_API_URL))
        .send()
        .await
        .expect("Failed to fetch from Haunt API")
        .json()
        .await
        .expect("Failed to parse Haunt response");

    // These symbols should typically be in the top 10
    let expected_top_10 = ["BTC", "ETH"];

    let our_symbols: Vec<&str> = our_resp.data.iter().map(|l| l.symbol.as_str()).collect();

    println!("Our top 10 symbols: {:?}", our_symbols);

    for expected in &expected_top_10 {
        assert!(
            our_symbols.contains(expected),
            "{} should be in top 10, but found: {:?}",
            expected,
            our_symbols
        );
    }

    // Check that BTC is #1
    assert_eq!(
        our_resp.data[0].symbol, "BTC",
        "Bitcoin should be ranked #1"
    );

    // Check that ranks are positive
    for listing in &our_resp.data {
        assert!(
            listing.rank > 0,
            "Rank should be positive, got {} for {}",
            listing.rank,
            listing.symbol
        );
    }
}

/// Test that prices are not stale (updated recently)
#[tokio::test]
#[ignore]
async fn test_price_freshness() {
    let client = create_client();

    // Fetch twice with a small delay
    let resp1: HauntApiResponse<Vec<HauntListing>> = client
        .get(format!("{}/crypto/listings?limit=5", HAUNT_API_URL))
        .send()
        .await
        .expect("Failed to fetch from Haunt API")
        .json()
        .await
        .expect("Failed to parse first response");

    // Wait a bit
    tokio::time::sleep(Duration::from_secs(5)).await;

    let resp2: HauntApiResponse<Vec<HauntListing>> = client
        .get(format!("{}/crypto/listings?limit=5", HAUNT_API_URL))
        .send()
        .await
        .expect("Failed to fetch from Haunt API")
        .json()
        .await
        .expect("Failed to parse second response");

    // Create maps for comparison
    let prices1: HashMap<&str, f64> = resp1
        .data
        .iter()
        .map(|l| (l.symbol.as_str(), l.price))
        .collect();
    let prices2: HashMap<&str, f64> = resp2
        .data
        .iter()
        .map(|l| (l.symbol.as_str(), l.price))
        .collect();

    println!("Price changes over 5 seconds:");
    let mut any_changed = false;
    for (symbol, price1) in &prices1 {
        if let Some(price2) = prices2.get(symbol) {
            let change = (price2 - price1) / price1 * 100.0;
            println!(
                "  {}: ${:.4} -> ${:.4} ({:+.4}%)",
                symbol, price1, price2, change
            );
            if (price2 - price1).abs() > 0.0001 {
                any_changed = true;
            }
        }
    }

    // Note: This is a soft check - prices might not change in 5 seconds during quiet markets
    if !any_changed {
        println!(
            "  Note: No price changes detected in 5 seconds (may be normal during quiet periods)"
        );
    }
}

/// Comprehensive price deviation report
#[tokio::test]
#[ignore]
async fn test_comprehensive_price_report() {
    let client = create_client();

    // Fetch our top 20
    let our_resp: HauntApiResponse<Vec<HauntListing>> = client
        .get(format!("{}/crypto/listings?limit=20", HAUNT_API_URL))
        .send()
        .await
        .expect("Failed to fetch from Haunt API")
        .json()
        .await
        .expect("Failed to parse Haunt response");

    // Map symbol to CoinGecko ID
    let symbol_to_cg: HashMap<&str, &str> = [
        ("BTC", "bitcoin"),
        ("ETH", "ethereum"),
        ("BNB", "binancecoin"),
        ("XRP", "ripple"),
        ("ADA", "cardano"),
        ("DOGE", "dogecoin"),
        ("SOL", "solana"),
        ("DOT", "polkadot"),
        ("MATIC", "matic-network"),
        ("LTC", "litecoin"),
        ("SHIB", "shiba-inu"),
        ("TRX", "tron"),
        ("AVAX", "avalanche-2"),
        ("LINK", "chainlink"),
        ("ATOM", "cosmos"),
        ("UNI", "uniswap"),
        ("XLM", "stellar"),
        ("ETC", "ethereum-classic"),
        ("BCH", "bitcoin-cash"),
        ("FIL", "filecoin"),
    ]
    .into_iter()
    .collect();

    // Get CoinGecko IDs for symbols we have
    let cg_ids: Vec<&str> = our_resp
        .data
        .iter()
        .filter_map(|l| symbol_to_cg.get(l.symbol.as_str()).copied())
        .collect();

    if cg_ids.is_empty() {
        println!("No matching CoinGecko IDs found");
        return;
    }

    let ids_param = cg_ids.join(",");
    let cg_url = format!(
        "{}/simple/price?ids={}&vs_currencies=usd",
        COINGECKO_API_URL, ids_param
    );

    let cg_resp: HashMap<String, CoinGeckoPrice> = client
        .get(&cg_url)
        .send()
        .await
        .expect("Failed to fetch from CoinGecko")
        .json()
        .await
        .expect("Failed to parse CoinGecko response");

    println!("\n=== Comprehensive Price Deviation Report ===\n");
    println!(
        "{:<8} {:>14} {:>14} {:>10}",
        "Symbol", "Our Price", "CoinGecko", "Deviation"
    );
    println!("{}", "-".repeat(50));

    let mut total_deviation = 0.0;
    let mut count = 0;

    for listing in &our_resp.data {
        if let Some(cg_id) = symbol_to_cg.get(listing.symbol.as_str()) {
            if let Some(cg_price) = cg_resp.get(*cg_id) {
                let deviation = calculate_deviation(listing.price, cg_price.usd);
                let status = if deviation <= PRICE_DEVIATION_THRESHOLD {
                    "✓"
                } else {
                    "✗"
                };
                println!(
                    "{} {:<6} {:>14.4} {:>14.4} {:>9.2}%",
                    status, listing.symbol, listing.price, cg_price.usd, deviation
                );
                total_deviation += deviation;
                count += 1;
            }
        }
    }

    if count > 0 {
        println!("{}", "-".repeat(50));
        println!("Average deviation: {:.2}%", total_deviation / count as f64);
    }
}

// ============================================================================
// Unit Tests (don't require network)
// ============================================================================

#[test]
fn test_deviation_calculation() {
    // Same values = 0% deviation
    assert!((calculate_deviation(100.0, 100.0) - 0.0).abs() < 0.001);

    // 10% higher = 10% deviation
    assert!((calculate_deviation(110.0, 100.0) - 10.0).abs() < 0.001);

    // 10% lower = 10% deviation
    assert!((calculate_deviation(90.0, 100.0) - 10.0).abs() < 0.001);

    // Reference is 0 and our value is not = 100% deviation
    assert_eq!(calculate_deviation(100.0, 0.0), 100.0);

    // Both are 0 = 0% deviation
    assert_eq!(calculate_deviation(0.0, 0.0), 0.0);
}

#[test]
fn test_percent_deviation_calculation() {
    // Same values = 0 deviation
    assert!((calculate_percent_deviation(5.0, 5.0) - 0.0).abs() < 0.001);

    // 2 percentage points difference
    assert!((calculate_percent_deviation(5.0, 3.0) - 2.0).abs() < 0.001);

    // Negative to positive
    assert!((calculate_percent_deviation(-2.0, 3.0) - 5.0).abs() < 0.001);
}

#[test]
#[allow(clippy::assertions_on_constants)]
fn test_thresholds_are_reasonable() {
    // Price deviation should be relatively tight
    assert!(
        PRICE_DEVIATION_THRESHOLD <= 10.0,
        "Price deviation threshold too loose"
    );
    assert!(
        PRICE_DEVIATION_THRESHOLD >= 1.0,
        "Price deviation threshold too tight"
    );

    // Volume can vary more across sources
    assert!(
        VOLUME_DEVIATION_THRESHOLD <= 50.0,
        "Volume deviation threshold too loose"
    );
    assert!(
        VOLUME_DEVIATION_THRESHOLD >= 10.0,
        "Volume deviation threshold too tight"
    );

    // Percent change should be within a few percentage points
    assert!(
        PERCENT_CHANGE_DEVIATION_THRESHOLD <= 5.0,
        "Percent change deviation threshold too loose"
    );
}
