mod api;
mod config;
mod error;
mod services;
mod sources;
mod types;
mod websocket;

use axum::{routing::get, Router};
use config::Config;
use services::{
    AccuracyStore, AssetService, AuthService, ChartStore, HistoricalDataService,
    MultiSourceCoordinator, OrderBookService, PeerMesh, PredictionStore, SignalStore, SqliteStore,
};
use sources::{AlpacaWs, CoinMarketCapClient, FinnhubClient, TiingoWs};
// FinnhubWs requires paid tier for US stocks - use Tiingo or Alpaca instead
#[allow(unused_imports)]
use sources::FinnhubWs;
use std::sync::Arc;
use types::TradingTimeframe;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{debug, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use websocket::RoomManager;

/// Application state shared across handlers.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub coordinator: Arc<MultiSourceCoordinator>,
    pub room_manager: Arc<RoomManager>,
    pub chart_store: Arc<ChartStore>,
    pub cmc_client: Arc<CoinMarketCapClient>,
    pub finnhub_client: Option<Arc<FinnhubClient>>,
    pub asset_service: Arc<AssetService>,
    pub price_cache: Arc<services::PriceCache>,
    pub historical_service: Arc<HistoricalDataService>,
    pub signal_store: Arc<SignalStore>,
    pub auth_service: Arc<AuthService>,
    pub sqlite_store: Arc<SqliteStore>,
    pub orderbook_service: Arc<OrderBookService>,
    pub peer_mesh: Arc<PeerMesh>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "haunt=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = Arc::new(Config::from_env());
    info!("Starting Haunt server on {}:{}", config.host, config.port);

    // Create the multi-source coordinator
    let (coordinator, _price_rx) = MultiSourceCoordinator::new(&config);

    // Get references to shared services
    let chart_store = coordinator.chart_store();
    let price_cache = coordinator.price_cache();

    // Connect to Redis for persistence
    if let Some(ref redis_url) = config.redis_url {
        // Connect chart store
        chart_store.connect_redis(redis_url).await;

        // Connect price cache
        price_cache.connect_redis(redis_url).await;

        // Load ALL available sparkline data from Redis (scans for all persisted symbols)
        chart_store.load_all_from_redis().await;

        // Also load from the known symbol list for price cache
        let symbols: Vec<&str> = sources::coingecko::SYMBOL_TO_ID
            .iter()
            .map(|(s, _)| *s)
            .collect();
        price_cache.load_from_redis(&symbols).await;

        // Load update counts from Redis
        price_cache.load_update_counts().await;
    }

    // Create CoinMarketCap client for API endpoints
    let cmc_client = Arc::new(CoinMarketCapClient::new(
        config.cmc_api_key.clone().unwrap_or_default(),
        price_cache.clone(),
        chart_store.clone(),
    ));

    // Create Finnhub client for stock/ETF data (optional)
    let finnhub_client = config.finnhub_api_key.as_ref().map(|api_key| {
        info!("Finnhub API key found, enabling stock/ETF data");
        let client = Arc::new(FinnhubClient::new(api_key.clone()));
        // Start background polling for stock data
        client.clone().start_polling();
        client
    });

    // Create Alpha Vantage client for historical stock/ETF data (optional)
    let alphavantage_client = config.alpha_vantage_api_key.as_ref().map(|api_key| {
        info!("Alpha Vantage API key found, enabling historical stock data");
        Arc::new(sources::AlphaVantageClient::new(api_key.clone()))
    });

    // Note: Finnhub WebSocket for US stocks requires a paid subscription.
    // The free tier only supports crypto WebSocket. Use Tiingo or Alpaca for free
    // real-time stock data instead.
    // To enable paid Finnhub WS, uncomment the following block:
    /*
    if let Some(ref api_key) = config.finnhub_api_key {
        info!("Starting Finnhub WebSocket for real-time stock data");
        let finnhub_ws = FinnhubWs::new(
            api_key.clone(),
            price_cache.clone(),
            chart_store.clone(),
        );
        tokio::spawn(async move {
            if let Err(e) = finnhub_ws.connect().await {
                tracing::error!("Finnhub WebSocket error: {}", e);
            }
        });
    }
    */
    if config.finnhub_api_key.is_some() {
        info!("Finnhub API configured for REST polling (WebSocket requires paid tier)");
    }

    // Start Alpaca WebSocket for real-time stock data (if configured)
    if let (Some(api_key), Some(api_secret)) = (&config.alpaca_api_key, &config.alpaca_api_secret) {
        info!("Starting Alpaca WebSocket for real-time stock data");
        let alpaca_ws = AlpacaWs::new(
            api_key.clone(),
            api_secret.clone(),
            price_cache.clone(),
            chart_store.clone(),
        );
        tokio::spawn(async move {
            if let Err(e) = alpaca_ws.connect().await {
                tracing::error!("Alpaca WebSocket error: {}", e);
            }
        });
    }

    // Note: Tiingo IEX WebSocket free tier requirements have changed.
    // The free tier may no longer support real-time data via WebSocket.
    // Use Alpaca instead for free real-time IEX stock data.
    // To enable Tiingo (if you have a paid tier), uncomment the following:
    /*
    if let Some(ref api_key) = config.tiingo_api_key {
        info!("Starting Tiingo WebSocket for real-time stock data");
        let tiingo_ws = TiingoWs::new(
            api_key.clone(),
            price_cache.clone(),
            chart_store.clone(),
        );
        tokio::spawn(async move {
            if let Err(e) = tiingo_ws.connect().await {
                tracing::error!("Tiingo WebSocket error: {}", e);
            }
        });
    }
    */

    // Create unified asset service
    let asset_service = Arc::new(AssetService::new(
        cmc_client.clone(),
        finnhub_client.clone(),
    ));

    // Create historical data service for seeding chart data
    let historical_service = HistoricalDataService::new(
        chart_store.clone(),
        config.coingecko_api_key.clone(),
        config.cryptocompare_api_key.clone(),
        alphavantage_client.clone(),
    );

    // Connect historical service to Redis and load historical data
    if let Some(ref redis_url) = config.redis_url {
        historical_service.connect_redis(redis_url).await;
        // Load historical data for common symbols on startup
        historical_service.load_common_symbols().await;
        // Load historical data for stocks/ETFs
        historical_service.load_stock_symbols().await;
    }

    // Create signal stores for trading signals
    let prediction_store = PredictionStore::new();
    let accuracy_store = AccuracyStore::new();

    // Connect signal stores to Redis
    if let Some(ref redis_url) = config.redis_url {
        prediction_store.connect_redis(redis_url).await;
        accuracy_store.connect_redis(redis_url).await;
        accuracy_store.load_all_from_redis().await;
    }

    let signal_store = SignalStore::new(
        chart_store.clone(),
        prediction_store.clone(),
        accuracy_store.clone(),
    );

    // Create SQLite store for persistent profile and prediction storage
    let sqlite_store = Arc::new(
        SqliteStore::new("haunt.db").expect("Failed to initialize SQLite database"),
    );
    info!("SQLite database initialized at haunt.db");

    // Connect prediction store to SQLite for permanent history
    prediction_store.connect_sqlite(sqlite_store.clone()).await;
    info!("Prediction store connected to SQLite for permanent history");

    // Load existing predictions from SQLite
    prediction_store.load_from_sqlite().await;

    // Create order book service for aggregated depth data
    let orderbook_service = Arc::new(OrderBookService::new());
    info!("Order book service initialized");

    // Create auth service with Redis (for sessions) and SQLite (for profiles)
    let redis_conn = if let Some(ref redis_url) = config.redis_url {
        match redis::Client::open(redis_url.as_str()) {
            Ok(client) => match redis::aio::ConnectionManager::new(client).await {
                Ok(conn) => {
                    info!("Auth service connected to Redis");
                    Some(conn)
                }
                Err(e) => {
                    tracing::warn!("Failed to connect auth service to Redis: {}", e);
                    None
                }
            },
            Err(e) => {
                tracing::warn!("Invalid Redis URL for auth service: {}", e);
                None
            }
        }
    } else {
        None
    };
    let auth_service = Arc::new(AuthService::new(redis_conn, Some(sqlite_store.clone())));

    // Create room manager for WebSocket subscriptions
    let room_manager = RoomManager::new();

    // Create peer mesh for cross-server sync
    let peer_mesh = Arc::new(PeerMesh::new(&config));
    info!(
        "Peer mesh initialized for server '{}' in '{}'",
        config.server_id, config.server_region
    );

    // Start peer health check task
    peer_mesh.clone().start_health_check();

    // Create application state
    let state = AppState {
        config: config.clone(),
        coordinator: coordinator.clone(),
        room_manager,
        chart_store: chart_store.clone(),
        cmc_client,
        finnhub_client,
        asset_service,
        price_cache: price_cache.clone(),
        historical_service,
        signal_store: signal_store.clone(),
        auth_service,
        sqlite_store,
        orderbook_service,
        peer_mesh,
    };

    // Start the price sources
    coordinator.start().await;

    // Start periodic Redis save tasks
    {
        let chart_store = chart_store.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                chart_store.save_all_to_redis().await;
            }
        });
    }
    {
        let price_cache = price_cache.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                price_cache.save_update_counts().await;
            }
        });
    }

    // Start prediction validation task (every 30 seconds for faster scalping feedback)
    {
        let signal_store = signal_store.clone();
        let chart_store_clone = chart_store.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;

                // Get all symbols with pending predictions using the new efficient method
                let symbols = signal_store.prediction_store().get_pending_symbols();

                if symbols.is_empty() {
                    continue;
                }

                let symbol_count = symbols.len();
                let mut total_validations = 0usize;

                for symbol in &symbols {
                    if let Some(current_price) = chart_store_clone.get_current_price(symbol) {
                        // Validate for each timeframe (including 5m for quick feedback)
                        for timeframe in &["5m", "1h", "4h", "24h"] {
                            let outcomes = signal_store
                                .prediction_store()
                                .validate_pending(symbol, current_price, timeframe)
                                .await;

                            // Record outcomes in accuracy store
                            for (indicator, outcome) in &outcomes {
                                signal_store
                                    .accuracy_store()
                                    .record_outcome(symbol, indicator, timeframe, *outcome)
                                    .await;
                            }

                            total_validations += outcomes.len();
                        }
                    } else {
                        debug!("No price data for {} - skipping validation", symbol);
                    }
                }

                if total_validations > 0 {
                    info!(
                        "Validated {} predictions across {} symbols",
                        total_validations, symbol_count
                    );
                } else {
                    debug!("Checked {} symbols - no predictions ready for validation", symbol_count);
                }
            }
        });
    }

    // Start automatic prediction generation for liquid assets
    {
        let asset_service = state.asset_service.clone();
        let signal_store = signal_store.clone();
        let chart_store = chart_store.clone();

        tokio::spawn(async move {
            // Initial delay to let system stabilize
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

            loop {
                // Fetch top 100 crypto assets sorted by volume
                match asset_service
                    .get_listings(crate::api::crypto::AssetType::Crypto, 1, 100)
                    .await
                {
                    Ok((listings, _)) => {
                        let mut processed = 0;

                        for listing in &listings {
                            // LIQUIDITY FILTER: Skip low-volume assets (< $1M 24h volume)
                            if listing.volume_24h < 1_000_000.0 {
                                continue;
                            }

                            let symbol = listing.symbol.to_lowercase();

                            // Check if we have price data
                            if chart_store.get_current_price(&symbol).is_some() {
                                // Generate signals (creates predictions as side effect)
                                let _ = signal_store
                                    .get_signals(&symbol, TradingTimeframe::DayTrading)
                                    .await;
                                processed += 1;
                            }

                            // Stagger: 3 seconds between assets to avoid CPU spikes
                            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                        }

                        info!(
                            "Prediction cycle complete: {} liquid assets processed",
                            processed
                        );
                    }
                    Err(e) => {
                        tracing::warn!("Failed to fetch listings for prediction generation: {}", e);
                    }
                }

                // Brief pause before next cycle
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            }
        });
    }

    // Build CORS layer
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build the router
    let app = Router::new()
        .merge(api::router())
        .route("/ws", get(websocket::ws_handler))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start the server
    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Haunt server listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
