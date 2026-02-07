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
    AccuracyStore, AssetService, AuthService, ChartStore, ExchangeMetricsService,
    HistoricalDataService, MultiSourceCoordinator, OrderBookService, PeerConfig, PeerMesh,
    PredictionStore, RatService, SignalStore, SqliteStore, StorageManager, SyncService,
    UsernameFilterService,
};
use sources::{AlpacaWs, CoinCapClient, CoinMarketCapClient, FinnhubClient};
// FinnhubWs requires paid tier for US stocks - use Tiingo or Alpaca instead
#[allow(unused_imports)]
use sources::{FinnhubWs, TiingoWs};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{debug, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use types::TradingTimeframe;
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
    pub peer_mesh: Option<Arc<PeerMesh>>,
    pub sync_service: Option<Arc<SyncService>>,
    pub storage_manager: Option<Arc<StorageManager>>,
    pub trading_service: Arc<services::TradingService>,
    pub gridline_service: Arc<services::GridlineService>,
    pub rat_service: Arc<RatService>,
    pub exchange_metrics: Arc<ExchangeMetricsService>,
    pub username_filter: Arc<UsernameFilterService>,
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

    // Create CoinCap client for redundant crypto data
    let coincap_client = Arc::new(CoinCapClient::new());

    // Create unified asset service with fallback sources
    let asset_service = Arc::new(AssetService::new(
        cmc_client.clone(),
        coincap_client.clone(),
        finnhub_client.clone(),
        price_cache.clone(),
        chart_store.clone(),
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
    let sqlite_store =
        Arc::new(SqliteStore::new("haunt.db").expect("Failed to initialize SQLite database"));
    info!("SQLite database initialized at haunt.db");

    // Connect prediction store to SQLite for permanent history
    prediction_store.connect_sqlite(sqlite_store.clone()).await;
    info!("Prediction store connected to SQLite for permanent history");

    // Load existing predictions from SQLite
    prediction_store.load_from_sqlite().await;

    // Create order book service for aggregated depth data
    let orderbook_service = Arc::new(OrderBookService::new());
    info!("Order book service initialized");

    // Create peer mesh for multi-server connectivity
    let peer_mesh = if !config.peer_servers.is_empty()
        || !config.bootstrap_servers.is_empty()
        || !config.server_id.is_empty()
    {
        info!(
            "Initializing peer mesh: server_id={}, region={}, peers={}, bootstrap={}",
            config.server_id,
            config.server_region,
            config.peer_servers.len(),
            config.bootstrap_servers.len()
        );

        let mesh = PeerMesh::new(
            config.server_id.clone(),
            config.server_region.clone(),
            config.public_ws_url.clone(),
            config.public_api_url.clone(),
            if config.mesh_auth.shared_key.is_empty() {
                None
            } else {
                Some(config.mesh_auth.shared_key.clone())
            },
            config.mesh_auth.require_auth,
        );

        // Add configured peer servers
        for peer_config in &config.peer_servers {
            info!("Adding peer: {} ({})", peer_config.id, peer_config.region);
            mesh.add_peer(PeerConfig {
                id: peer_config.id.clone(),
                region: peer_config.region.clone(),
                ws_url: peer_config.ws_url.clone(),
                api_url: peer_config.api_url.clone(),
            });
        }

        // Add bootstrap servers (convert to peer configs)
        for bootstrap in &config.bootstrap_servers {
            // Skip if already in peer_servers
            if config.peer_servers.iter().any(|p| p.id == bootstrap.id) {
                continue;
            }
            info!(
                "Adding bootstrap peer: {} ({})",
                bootstrap.id, bootstrap.address
            );
            mesh.add_peer(PeerConfig {
                id: bootstrap.id.clone(),
                region: bootstrap.id.clone(), // Use ID as region for bootstrap
                ws_url: format!("ws://{}/ws", bootstrap.address),
                api_url: format!("http://{}", bootstrap.address),
            });
        }

        Some(mesh)
    } else {
        info!(
            "Peer mesh disabled (no SERVER_ID, PEER_SERVERS, or MESH_BOOTSTRAP_SERVERS configured)"
        );
        None
    };

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

    // Create trading service for paper trading (with room_manager for real-time updates)
    // TODO: Add interior mutability to sync_service field to enable runtime connection
    let trading_service = Arc::new(services::TradingService::with_room_manager(
        sqlite_store.clone(),
        room_manager.clone(),
    ));

    // Create gridline trading service for real-time prediction trading
    let gridline_service = Arc::new(services::GridlineService::new(
        sqlite_store.clone(),
        Some(room_manager.clone()),
    ));

    // Create sync service if peer mesh is enabled
    let sync_service = if let Some(ref mesh) = peer_mesh {
        let is_primary = config.server_id == "osaka";
        let service = SyncService::new(
            sqlite_store.clone(),
            mesh.clone(),
            config.server_id.clone(),
            is_primary,
        );
        info!("Sync service created (primary: {})", is_primary);

        // Connect sync service to trading service for distributed sync
        trading_service.set_sync_service(service.clone());

        Some(service)
    } else {
        None
    };

    // Create storage manager for disk space monitoring and cleanup
    let storage_manager = StorageManager::new(sqlite_store.clone(), config.storage.clone());
    info!("Storage manager created (limit: {} MB)", config.storage.limit_mb);

    // Create exchange metrics service for latency and dominance tracking
    let exchange_metrics = ExchangeMetricsService::new();
    info!("Exchange metrics service initialized");

    // Connect exchange metrics to price cache for automatic latency tracking
    price_cache.set_exchange_metrics(exchange_metrics.clone()).await;

    // Create username filter service for validation and content moderation
    let username_filter = UsernameFilterService::default_service();
    info!("Username filter service initialized");

    // Create RAT (Random Auto Trader) service for developer testing
    let rat_service = RatService::new(
        sqlite_store.clone(),
        trading_service.clone(),
        price_cache.clone(),
    );
    // Load persisted RAT configs from database
    rat_service.load_from_database();
    info!("RAT service initialized");

    // Create application state
    let state = AppState {
        config: config.clone(),
        coordinator: coordinator.clone(),
        room_manager: room_manager.clone(),
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
        peer_mesh: peer_mesh.clone(),
        sync_service: sync_service.clone(),
        storage_manager: Some(storage_manager.clone()),
        trading_service: trading_service.clone(),
        gridline_service: gridline_service.clone(),
        rat_service: rat_service.clone(),
        exchange_metrics: exchange_metrics.clone(),
        username_filter,
    };

    // Keep a reference for the market simulation engine
    let trading_service_for_sim = trading_service;

    // Start the price sources
    coordinator.start().await;

    // Start the peer mesh if configured
    if let Some(ref mesh) = peer_mesh {
        info!("Starting peer mesh connections...");
        mesh.clone().start();

        // Start sync service
        if let Some(ref sync) = sync_service {
            info!("Starting sync service...");
            sync.clone().start();
        }
    }

    // Start storage manager (always enabled)
    info!("Starting storage manager...");
    storage_manager.clone().start();

    // Auto-start enabled RAT instances
    rat_service.clone().auto_start_enabled();

    // Spawn peer status broadcast task to connected WebSocket clients
    if let Some(ref mesh) = peer_mesh {
        let mesh_for_broadcast = mesh.clone();
        let room_manager_for_peers = room_manager.clone();
        let config_for_peers = config.clone();

        tokio::spawn(async move {
            let mut peer_rx = mesh_for_broadcast.subscribe();

            while let Ok(statuses) = peer_rx.recv().await {
                // Build the peer update message
                let update_data = types::PeerUpdateData {
                    server_id: config_for_peers.server_id.clone(),
                    server_region: config_for_peers.server_region.clone(),
                    peers: statuses,
                    timestamp: chrono::Utc::now().timestamp_millis(),
                };

                let msg = types::ServerMessage::PeerUpdate { data: update_data };

                if let Ok(json) = serde_json::to_string(&msg) {
                    // Send to all clients subscribed to peer updates
                    let subscribers = room_manager_for_peers.get_peer_subscribers();
                    for tx in subscribers {
                        let _ = tx.send(json.clone());
                    }
                }
            }
        });
    }

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
                    debug!(
                        "Checked {} symbols - no predictions ready for validation",
                        symbol_count
                    );
                }
            }
        });
    }

    // Start market simulation engine - processes orders and updates positions
    {
        let trading_service = trading_service_for_sim.clone();
        let chart_store = chart_store.clone();

        tokio::spawn(async move {
            // Initial delay to let system stabilize
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            info!("Market simulation engine started");

            let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(5));

            loop {
                ticker.tick().await;

                // First, auto-fill any pending market orders that weren't executed
                let filled = trading_service.auto_fill_pending_market_orders(|symbol| {
                    chart_store.get_current_price(symbol)
                });

                if filled > 0 {
                    info!("Auto-filled {} pending market orders", filled);
                }

                // Then process all active symbols for limit/stop orders and position updates
                let (positions_updated, orders_triggered, positions_closed) =
                    trading_service.process_all_market_ticks(|symbol| {
                        chart_store.get_current_price(symbol)
                    });

                // Log activity only if something happened
                if orders_triggered > 0 || positions_closed > 0 {
                    info!(
                        "Market tick: {} positions updated, {} orders triggered, {} positions closed",
                        positions_updated, orders_triggered, positions_closed
                    );
                }
            }
        });
    }

    // Start gridline trading resolution engine — listens to price updates
    {
        let gridline = gridline_service.clone();
        let trading_svc = state.trading_service.clone();
        let sqlite_for_notif = state.sqlite_store.clone();
        let mut price_rx = price_cache.subscribe();

        tokio::spawn(async move {
            info!("Gridline trading resolution engine started");
            loop {
                match price_rx.recv().await {
                    Ok(price_update) => {
                        let symbol = price_update.symbol.to_uppercase();
                        let price = price_update.price;
                        let timestamp = price_update.timestamp;

                        // Process gridline trade resolutions
                        let resolutions = gridline.on_price_update(&symbol, price, timestamp);

                        // Credit/debit portfolio balances for each resolution
                        for resolution in &resolutions {
                            if resolution.won {
                                if let Some(payout) = resolution.payout {
                                    let _ = trading_svc.credit_gridline_payout(
                                        &resolution.position.portfolio_id,
                                        payout,
                                    );
                                }
                            }

                            // Update stats
                            let _ = gridline.update_stats_after_resolution(
                                &resolution.position.portfolio_id,
                                &resolution.position.symbol,
                                resolution,
                            );

                            // Create notification for the trade resolution
                            if let Some(portfolio) = trading_svc.get_portfolio(
                                &resolution.position.portfolio_id,
                            ) {
                                let (notif_type, title, message) = if resolution.won {
                                    let payout = resolution.payout.unwrap_or(0.0);
                                    (
                                        types::NotificationType::Success,
                                        format!("{} trade won!", resolution.position.symbol),
                                        Some(format!(
                                            "+${:.2} ({:.1}x) on ${:.2} bet",
                                            resolution.pnl,
                                            resolution.position.multiplier,
                                            resolution.position.amount,
                                        )),
                                    )
                                } else {
                                    (
                                        types::NotificationType::Error,
                                        format!("{} trade lost", resolution.position.symbol),
                                        Some(format!(
                                            "-${:.2} on ${:.2} bet",
                                            resolution.position.amount,
                                            resolution.position.amount,
                                        )),
                                    )
                                };

                                let notification = types::Notification {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    user_id: portfolio.user_id.clone(),
                                    notification_type: notif_type,
                                    title,
                                    message,
                                    read: false,
                                    timestamp: chrono::Utc::now().timestamp_millis(),
                                };

                                sqlite_for_notif.create_notification(&notification);
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        debug!("Gridline resolution engine lagged by {} messages", n);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        info!("Gridline resolution engine: price channel closed");
                        break;
                    }
                }
            }
        });
    }

    // Start periodic gridline multiplier broadcast (~1Hz)
    // Computes multipliers for each symbol with active gridline subscribers
    // and broadcasts the full matrix over WebSocket.
    //
    // The grid config (price_high/price_low) is cached per-symbol and only
    // re-centered when the price drifts near the edge (within 15% of the range).
    // This prevents the "jerk-back" visual bug where the grid coordinate system
    // shifts every second, causing the sparkline dot to jump.
    {
        let gridline = gridline_service.clone();
        let room_mgr = room_manager.clone();
        let price_cache_gl = price_cache.clone();

        tokio::spawn(async move {
            // Short initial delay
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            info!("Gridline multiplier broadcast engine started");

            let mut ticker = tokio::time::interval(tokio::time::Duration::from_millis(1000));

            // Cached configs per symbol — only re-center when price nears grid edge
            let mut cached_configs: std::collections::HashMap<String, crate::types::GridConfig> =
                std::collections::HashMap::new();

            loop {
                ticker.tick().await;

                // Get all symbols with active gridline subscribers
                let symbols = room_mgr.active_gridline_symbols();
                if symbols.is_empty() {
                    continue;
                }

                for symbol in &symbols {
                    // Get current price
                    let current_price = price_cache_gl
                        .get_price(symbol)
                        .unwrap_or(0.0);

                    if current_price <= 0.0 {
                        continue;
                    }

                    // Record price tick in the gridline buffer for volatility
                    let now = chrono::Utc::now().timestamp_millis();
                    gridline.record_price_tick(symbol, current_price, now);

                    // Get or build config — only re-center if price near edge
                    let config = {
                        let needs_rebuild = if let Some(cached) = cached_configs.get(symbol.as_str()) {
                            let range = cached.price_high - cached.price_low;
                            let margin = range * 0.15;
                            current_price > cached.price_high - margin
                                || current_price < cached.price_low + margin
                        } else {
                            true
                        };

                        if needs_rebuild {
                            let new_config =
                                gridline.build_config(symbol, current_price, Some(36), Some(12));
                            cached_configs.insert(symbol.clone(), new_config.clone());
                            new_config
                        } else {
                            cached_configs.get(symbol.as_str()).unwrap().clone()
                        }
                    };

                    // Calculate multipliers with current price against the stable config
                    let multipliers =
                        gridline.calculate_multipliers(symbol, current_price, &config);

                    // Broadcast to all gridline subscribers
                    gridline.broadcast_multiplier_update(
                        symbol,
                        current_price,
                        &multipliers,
                        &config,
                    );
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
