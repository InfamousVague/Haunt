pub mod crypto;
pub mod health;
pub mod market;

use axum::Router;
use crate::AppState;

/// Create the API router.
pub fn router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .nest("/api/crypto", crypto::router())
        .nest("/api/market", market::router())
}
