pub mod auth;
pub mod bots;
pub mod crypto;
pub mod health;
pub mod market;
pub mod orderbook;
pub mod peers;
pub mod signals;
pub mod sync;
pub mod trading;

use crate::AppState;
use axum::Router;

/// Create the API router.
pub fn router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .nest("/api/crypto", crypto::router())
        .nest("/api/market", market::router())
        .nest("/api/signals", signals::router())
        .nest("/api/auth", auth::router())
        .nest("/api/orderbook", orderbook::router())
        .nest("/api/peers", peers::router())
        .nest("/api/mesh", peers::mesh_router())
        .nest("/api/sync", sync::router())
        .nest("/api/trading", trading::router())
        .nest("/api/bots", bots::router())
}
