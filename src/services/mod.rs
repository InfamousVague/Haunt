pub mod asset_service;
pub mod auth;
pub mod cache;
pub mod chart_store;
pub mod historical;
pub mod multi_source;
pub mod orderbook;
pub mod peer_mesh;
pub mod price_cache;
pub mod redis_store;
pub mod signals;
pub mod sqlite_store;

pub use asset_service::AssetService;
pub use auth::{AuthError, AuthService};
pub use cache::Cache;
pub use chart_store::ChartStore;
pub use historical::{HistoricalDataService, SeedStatus};
pub use multi_source::MultiSourceCoordinator;
pub use orderbook::OrderBookService;
pub use peer_mesh::PeerMesh;
// Re-export peer types from types module
pub use crate::types::{PeerConfig, PeerConnectionStatus, PeerMessage, PeerStatus};
pub use price_cache::{ExchangeStats, PriceCache, SymbolSourceStat};
pub use redis_store::RedisStore;
pub use signals::{AccuracyStore, PredictionStore, SignalStore};
pub use sqlite_store::{SqliteStore, AccuracyStats};
