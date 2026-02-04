//! Integration tests for trading endpoints.
//!
//! These tests verify the complete trading flow:
//! 1. Authentication (challenge/verify with HMAC-SHA256)
//! 2. Portfolio creation/retrieval
//! 3. Order placement
//! 4. Order listing
//! 5. Portfolio balance updates
//!
//! Run with: cargo test --test trading_integration_test -- --ignored --nocapture

use hmac::{Hmac, Mac};
use sha2::Sha256;
use serde::{Deserialize, Serialize};
use serde_json::json;

const API_BASE_URL: &str = "http://localhost:3001";

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ApiResponse<T> {
    data: T,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct AuthChallenge {
    challenge: String,
    timestamp: i64,
    expires_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct AuthResponse {
    authenticated: bool,
    public_key: String,
    session_token: String,
    expires_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct Portfolio {
    id: String,
    user_id: String,
    name: String,
    starting_balance: f64,
    cash_balance: f64,
    margin_used: f64,
    margin_available: f64,
    unrealized_pnl: f64,
    realized_pnl: f64,
    total_value: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaceOrderRequest {
    portfolio_id: String,
    symbol: String,
    asset_class: String,
    side: String,
    order_type: String,
    quantity: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    price: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct Order {
    id: String,
    portfolio_id: String,
    symbol: String,
    side: String,
    order_type: String,
    status: String,
    quantity: f64,
    filled_quantity: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct Position {
    id: String,
    portfolio_id: String,
    symbol: String,
    side: String,
    quantity: f64,
    entry_price: f64,
}

/// Helper to create a test user and get auth token using challenge/verify flow
async fn authenticate_test_user(client: &reqwest::Client) -> (String, String) {
    // Generate a unique test key (hex-encoded for consistency)
    let test_key = format!("{:064x}", chrono::Utc::now().timestamp_millis());

    // Step 1: Get challenge
    let challenge_response = client
        .get(&format!("{}/api/auth/challenge", API_BASE_URL))
        .send()
        .await
        .expect("Failed to get challenge");

    assert!(
        challenge_response.status().is_success(),
        "Challenge request failed with status: {}",
        challenge_response.status()
    );

    let challenge: ApiResponse<AuthChallenge> = challenge_response
        .json()
        .await
        .expect("Failed to parse challenge response");

    // Step 2: Sign the challenge with HMAC-SHA256
    // Use the test_key as both the public key and secret (for simplicity in tests)
    let mut mac = HmacSha256::new_from_slice(test_key.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(challenge.data.challenge.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());

    let timestamp = chrono::Utc::now().timestamp_millis();

    // Step 3: Verify the signature
    let verify_response = client
        .post(&format!("{}/api/auth/verify", API_BASE_URL))
        .json(&json!({
            "publicKey": test_key,
            "challenge": challenge.data.challenge,
            "signature": signature,
            "timestamp": timestamp
        }))
        .send()
        .await
        .expect("Failed to verify signature");

    assert!(
        verify_response.status().is_success(),
        "Verify failed with status: {}",
        verify_response.status()
    );

    let auth: ApiResponse<AuthResponse> = verify_response
        .json()
        .await
        .expect("Failed to parse auth response");

    (auth.data.session_token, auth.data.public_key)
}

/// Helper to get or create a test portfolio
async fn get_or_create_portfolio(client: &reqwest::Client, token: &str, user_id: &str) -> Portfolio {
    // First, try to list existing portfolios
    let list_response = client
        .get(&format!("{}/api/trading/portfolios", API_BASE_URL))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to list portfolios");

    if list_response.status().is_success() {
        let portfolios: ApiResponse<Vec<Portfolio>> = list_response
            .json()
            .await
            .expect("Failed to parse portfolios");

        // Return first portfolio for this user if exists
        if let Some(portfolio) = portfolios.data.into_iter().find(|p| p.user_id == user_id) {
            return portfolio;
        }
    }

    // Create a new portfolio - requires user_id and name (snake_case)
    let create_response = client
        .post(&format!("{}/api/trading/portfolios", API_BASE_URL))
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .json(&json!({
            "user_id": user_id,
            "name": "Test Trading Portfolio"
        }))
        .send()
        .await
        .expect("Failed to create portfolio");

    let status = create_response.status();
    let body = create_response.text().await.unwrap();

    if !status.is_success() {
        panic!("Portfolio creation failed with status: {} - body: {}", status, body);
    }

    let portfolio: ApiResponse<Portfolio> = serde_json::from_str(&body)
        .expect("Failed to parse portfolio response");

    portfolio.data
}

/// Test the complete trading flow
#[tokio::test]
#[ignore]
async fn test_trading_flow_end_to_end() {
    let client = reqwest::Client::new();

    // Step 1: Authenticate
    println!("Step 1: Authenticating...");
    let (token, user_id) = authenticate_test_user(&client).await;
    println!("  ✓ Authenticated successfully (user: {}...)", &user_id[..8]);

    // Step 2: Get or create portfolio
    println!("Step 2: Getting/creating portfolio...");
    let portfolio = get_or_create_portfolio(&client, &token, &user_id).await;
    println!(
        "  ✓ Portfolio: {} (cash: ${:.2}, total: ${:.2})",
        portfolio.name, portfolio.cash_balance, portfolio.total_value
    );
    let initial_balance = portfolio.cash_balance;

    // Step 3: Place a market buy order
    println!("Step 3: Placing market buy order for BTC...");
    let order_request = PlaceOrderRequest {
        portfolio_id: portfolio.id.clone(),
        symbol: "BTC".to_string(),
        asset_class: "crypto_spot".to_string(),
        side: "buy".to_string(),
        order_type: "market".to_string(),
        quantity: 0.01,
        price: None,
    };

    let order_response = client
        .post(&format!("{}/api/trading/orders", API_BASE_URL))
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .json(&order_request)
        .send()
        .await
        .expect("Failed to place order");

    println!("  Order response status: {}", order_response.status());

    let order_text = order_response.text().await.unwrap();
    println!("  Order response body: {}", order_text);

    let order: ApiResponse<Order> = serde_json::from_str(&order_text)
        .expect("Failed to parse order response");

    println!(
        "  ✓ Order placed: {} - {} {} {} @ market",
        order.data.id, order.data.side, order.data.quantity, order.data.symbol
    );
    println!("  ✓ Order status: {}", order.data.status);

    // Step 4: List orders
    println!("Step 4: Listing orders...");
    let orders_response = client
        .get(&format!(
            "{}/api/trading/orders?portfolio_id={}",
            API_BASE_URL, portfolio.id
        ))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to list orders");

    assert!(
        orders_response.status().is_success(),
        "List orders failed: {}",
        orders_response.status()
    );

    let orders: ApiResponse<Vec<Order>> = orders_response
        .json()
        .await
        .expect("Failed to parse orders");

    println!("  ✓ Found {} pending orders", orders.data.len());

    // Step 5: Check positions (market orders should fill immediately)
    println!("Step 5: Checking positions...");
    let positions_response = client
        .get(&format!(
            "{}/api/trading/positions?portfolio_id={}",
            API_BASE_URL, portfolio.id
        ))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to list positions");

    if positions_response.status().is_success() {
        let positions: ApiResponse<Vec<Position>> = positions_response
            .json()
            .await
            .expect("Failed to parse positions");

        println!("  ✓ Found {} positions", positions.data.len());
        for pos in &positions.data {
            println!(
                "    - {} {} {} @ ${:.2}",
                pos.side, pos.quantity, pos.symbol, pos.entry_price
            );
        }
    }

    // Step 6: Check portfolio balance was updated
    println!("Step 6: Verifying portfolio balance...");
    let updated_portfolio_response = client
        .get(&format!(
            "{}/api/trading/portfolios/{}",
            API_BASE_URL, portfolio.id
        ))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to get updated portfolio");

    if updated_portfolio_response.status().is_success() {
        let updated_portfolio: ApiResponse<Portfolio> = updated_portfolio_response
            .json()
            .await
            .expect("Failed to parse portfolio");

        println!(
            "  Initial cash balance: ${:.2}",
            initial_balance
        );
        println!(
            "  Current cash balance: ${:.2}",
            updated_portfolio.data.cash_balance
        );

        // For a market buy, cash balance should decrease (funds used to buy)
        // Only assert if order was filled
        if order.data.status == "filled" {
            assert!(
                updated_portfolio.data.cash_balance < initial_balance,
                "Cash balance should decrease after market buy"
            );
            println!("  ✓ Cash balance decreased as expected after market buy");
        } else {
            println!("  ⚠ Order not filled yet (status: {}), balance unchanged", order.data.status);
        }
    }

    println!("\n✅ Trading flow test completed successfully!");
}

/// Test that unauthorized requests are rejected
#[tokio::test]
#[ignore]
async fn test_trading_requires_auth() {
    let client = reqwest::Client::new();

    // Try to place order without auth
    let response = client
        .post(&format!("{}/api/trading/orders", API_BASE_URL))
        .header("Content-Type", "application/json")
        .json(&json!({
            "portfolioId": "test",
            "symbol": "BTC",
            "assetClass": "crypto_spot",
            "side": "buy",
            "orderType": "market",
            "quantity": 1.0
        }))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status().as_u16(),
        401,
        "Expected 401 Unauthorized, got {}",
        response.status()
    );

    println!("✓ Trading endpoints correctly require authentication");
}

/// Test that users can only trade on their own portfolios
#[tokio::test]
#[ignore]
async fn test_portfolio_ownership_check() {
    let client = reqwest::Client::new();

    // Authenticate as user 1
    let (token1, user_id1) = authenticate_test_user(&client).await;
    let portfolio1 = get_or_create_portfolio(&client, &token1, &user_id1).await;

    // Authenticate as user 2
    let (token2, _user_id2) = authenticate_test_user(&client).await;

    // Try to place order on user1's portfolio with user2's token
    let response = client
        .post(&format!("{}/api/trading/orders", API_BASE_URL))
        .header("Authorization", format!("Bearer {}", token2))
        .header("Content-Type", "application/json")
        .json(&json!({
            "portfolioId": portfolio1.id,
            "symbol": "BTC",
            "assetClass": "crypto_spot",
            "side": "buy",
            "orderType": "market",
            "quantity": 0.01
        }))
        .send()
        .await
        .expect("Request failed");

    // Should be rejected - unauthorized to trade on someone else's portfolio
    assert!(
        response.status().is_client_error(),
        "Expected client error (401/403), got {}",
        response.status()
    );

    println!("✓ Portfolio ownership correctly enforced");
}

/// Test limit order placement
#[tokio::test]
#[ignore]
async fn test_limit_order() {
    let client = reqwest::Client::new();

    let (token, user_id) = authenticate_test_user(&client).await;
    let portfolio = get_or_create_portfolio(&client, &token, &user_id).await;

    // Place a limit buy order below current price (shouldn't fill immediately)
    let order_request = PlaceOrderRequest {
        portfolio_id: portfolio.id.clone(),
        symbol: "BTC".to_string(),
        asset_class: "crypto_spot".to_string(),
        side: "buy".to_string(),
        order_type: "limit".to_string(),
        quantity: 0.01,
        price: Some(1000.0), // Very low price, won't fill
    };

    let response = client
        .post(&format!("{}/api/trading/orders", API_BASE_URL))
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .json(&order_request)
        .send()
        .await
        .expect("Failed to place limit order");

    assert!(
        response.status().is_success(),
        "Limit order failed: {}",
        response.status()
    );

    let order: ApiResponse<Order> = response
        .json()
        .await
        .expect("Failed to parse order");

    assert_eq!(order.data.order_type, "limit");
    // Limit order at $1000 shouldn't fill immediately
    assert!(
        order.data.status == "open" || order.data.status == "pending",
        "Limit order should be open/pending, got: {}",
        order.data.status
    );

    println!("✓ Limit order placed successfully (status: {})", order.data.status);

    // Cancel the order
    let cancel_response = client
        .delete(&format!("{}/api/trading/orders/{}", API_BASE_URL, order.data.id))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to cancel order");

    assert!(
        cancel_response.status().is_success(),
        "Cancel order failed: {}",
        cancel_response.status()
    );

    println!("✓ Limit order cancelled successfully");
}
