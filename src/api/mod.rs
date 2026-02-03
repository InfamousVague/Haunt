pub mod auth;
pub mod crypto;
pub mod health;
pub mod market;
pub mod orderbook;
pub mod signals;
pub mod sync;

use axum::Router;
use crate::AppState;

/// Create the API router.
pub fn router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .nest("/api/crypto", crypto::router())
        .nest("/api/market", market::router())
        .nest("/api/signals", signals::router())
        .nest("/api/auth", auth::router())
        .nest("/api/orderbook", orderbook::router())
        .nest("/api/sync", sync::router())
}
