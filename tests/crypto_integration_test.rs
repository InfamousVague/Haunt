//! Integration tests for crypto endpoints.
//!
//! These tests require a running Haunt server at localhost:3001.
//! Run with: cargo test --test crypto_integration_test -- --ignored
//!
//! To run automatically in CI, ensure the server is started before tests.

use serde::Deserialize;

const API_BASE_URL: &str = "http://localhost:3001";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssetListing {
    id: i64,
    name: String,
    symbol: String,
    price: f64,
    asset_type: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ApiResponse<T> {
    data: T,
    meta: ApiMeta,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ApiMeta {
    #[serde(default)]
    total: Option<i32>,
}

/// Test that the crypto listings endpoint returns data.
///
/// This test will FAIL if:
/// - The server is not running
/// - The CoinMarketCap API is not returning data (invalid key, rate limited, etc.)
/// - There are no crypto assets in the response
///
/// Run with: cargo test --test crypto_integration_test test_crypto_listings_returns_data -- --ignored
#[tokio::test]
#[ignore] // Run manually or in CI with server running
async fn test_crypto_listings_returns_data() {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/api/crypto/listings?asset_type=crypto&limit=20",
        API_BASE_URL
    );

    let response = client
        .get(&url)
        .send()
        .await
        .expect("Failed to connect to Haunt server. Is it running at localhost:3001?");

    assert!(
        response.status().is_success(),
        "API returned error status: {}",
        response.status()
    );

    let body: ApiResponse<Vec<AssetListing>> = response
        .json()
        .await
        .expect("Failed to parse listings response");

    // CRITICAL: This should never be empty when CMC API is working
    assert!(
        !body.data.is_empty(),
        "CRITICAL: Crypto listings endpoint returned 0 assets! \
         This indicates the CoinMarketCap API is not returning data. \
         Check: 1) CMC_API_KEY in .env is valid, 2) API is not rate limited, \
         3) Network connectivity to pro-api.coinmarketcap.com"
    );

    // Verify we have a reasonable number of crypto assets
    let min_expected = 10;
    assert!(
        body.data.len() >= min_expected,
        "Expected at least {} crypto assets, got {}. \
         CMC API may be partially working or rate limited.",
        min_expected,
        body.data.len()
    );

    // Verify all returned assets are actually crypto
    for asset in &body.data {
        assert_eq!(
            asset.asset_type, "crypto",
            "Asset {} ({}) has wrong asset_type: expected 'crypto', got '{}'",
            asset.name, asset.symbol, asset.asset_type
        );
    }

    // Verify assets have valid data
    for asset in &body.data {
        assert!(asset.id > 0, "Asset {} has invalid ID", asset.symbol);
        assert!(
            !asset.name.is_empty(),
            "Asset {} has empty name",
            asset.symbol
        );
        assert!(!asset.symbol.is_empty(), "Asset has empty symbol");
        // Price can be 0 for some very small cap coins, but should be non-negative
        assert!(
            asset.price >= 0.0,
            "Asset {} has negative price",
            asset.symbol
        );
    }

    // Check for well-known crypto assets
    let symbols: Vec<&str> = body.data.iter().map(|a| a.symbol.as_str()).collect();
    let expected_coins = ["BTC", "ETH"];

    for coin in expected_coins {
        assert!(
            symbols.contains(&coin),
            "Expected to find {} in top crypto listings, but it was missing. \
             This may indicate incomplete data from CMC API.",
            coin
        );
    }

    println!(
        "✓ Crypto listings endpoint returned {} assets",
        body.data.len()
    );
    println!(
        "  Top 5: {:?}",
        body.data
            .iter()
            .take(5)
            .map(|a| &a.symbol)
            .collect::<Vec<_>>()
    );
}

/// Test that the listings endpoint works with different asset type filters.
#[tokio::test]
#[ignore]
async fn test_asset_type_filtering() {
    let client = reqwest::Client::new();

    // Test crypto filter
    let crypto_url = format!(
        "{}/api/crypto/listings?asset_type=crypto&limit=10",
        API_BASE_URL
    );
    let crypto_resp: ApiResponse<Vec<AssetListing>> = client
        .get(&crypto_url)
        .send()
        .await
        .expect("Failed to fetch crypto listings")
        .json()
        .await
        .expect("Failed to parse crypto response");

    // All returned assets should be crypto
    for asset in &crypto_resp.data {
        assert_eq!(
            asset.asset_type, "crypto",
            "Crypto filter returned non-crypto asset: {} ({})",
            asset.name, asset.asset_type
        );
    }

    println!("✓ Asset type filtering works correctly");
}

/// Test that the API gracefully handles errors.
#[tokio::test]
#[ignore]
async fn test_api_error_handling() {
    let client = reqwest::Client::new();

    // Test invalid asset ID
    let url = format!("{}/api/crypto/99999999999", API_BASE_URL);
    let response = client.get(&url).send().await.expect("Request failed");

    // Should return 404, not 500
    assert!(
        response.status().as_u16() == 404 || response.status().as_u16() == 400,
        "Invalid asset ID should return 404 or 400, got {}",
        response.status()
    );

    println!("✓ Error handling works correctly");
}

/// Quick health check to verify server is running.
#[tokio::test]
#[ignore]
async fn test_server_health() {
    let client = reqwest::Client::new();
    let url = format!("{}/health", API_BASE_URL);

    let response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .expect("Haunt server is not responding at localhost:3001. Start the server first.");

    assert!(
        response.status().is_success(),
        "Health check failed with status: {}",
        response.status()
    );

    println!("✓ Server is healthy");
}
