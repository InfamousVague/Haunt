mod api;
mod config;
mod error;
mod services;
mod sources;
mod types;
mod websocket;

use axum::{routing::get, Router};
use config::Config;
use services::{ChartStore, MultiSourceCoordinator};
use sources::CoinMarketCapClient;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;
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

    // Connect chart store to Redis for persistence
    if let Some(ref redis_url) = config.redis_url {
        chart_store.connect_redis(redis_url).await;

        // Load existing sparkline data from Redis
        let symbols: Vec<&str> = sources::coingecko::SYMBOL_TO_ID
            .iter()
            .map(|(s, _)| *s)
            .collect();
        chart_store.load_from_redis(&symbols).await;
    }

    // Create CoinMarketCap client for API endpoints
    let cmc_client = Arc::new(CoinMarketCapClient::new(
        config.cmc_api_key.clone().unwrap_or_default(),
        price_cache,
        chart_store.clone(),
    ));

    // Create room manager for WebSocket subscriptions
    let room_manager = RoomManager::new();

    // Create application state
    let state = AppState {
        config: config.clone(),
        coordinator: coordinator.clone(),
        room_manager,
        chart_store: chart_store.clone(),
        cmc_client,
    };

    // Start the price sources
    coordinator.start().await;

    // Start periodic Redis save task
    {
        let chart_store = chart_store.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                chart_store.save_all_to_redis().await;
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
