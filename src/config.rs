use std::env;

/// Peer server configuration for mesh networking.
#[derive(Debug, Clone)]
pub struct PeerServerConfig {
    /// Unique server ID.
    pub id: String,
    /// Server region/location name.
    pub region: String,
    /// WebSocket URL (ws:// or wss://).
    pub ws_url: String,
    /// HTTP API URL.
    pub api_url: String,
}

/// Bootstrap server configuration for initial mesh discovery.
#[derive(Debug, Clone)]
pub struct BootstrapServerConfig {
    /// Server ID.
    pub id: String,
    /// Server host:port.
    pub address: String,
}

/// Mesh authentication configuration.
#[derive(Debug, Clone)]
pub struct MeshAuthConfig {
    /// Shared secret key for peer authentication.
    pub shared_key: String,
    /// Whether to require authentication for peer connections.
    pub require_auth: bool,
}

/// Storage management configuration.
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// Maximum database size in MB (0 = unlimited).
    pub limit_mb: u64,
    /// Warning threshold percentage (0-100).
    pub warning_threshold_pct: u8,
    /// Critical threshold percentage (0-100, triggers auto-cleanup).
    pub critical_threshold_pct: u8,
    /// Enable automatic cleanup when threshold reached.
    pub auto_cleanup_enabled: bool,
    /// Retention periods for various data types (in days).
    pub retention: RetentionConfig,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            limit_mb: 30 * 1024, // 30 GB default
            warning_threshold_pct: 80,
            critical_threshold_pct: 95,
            auto_cleanup_enabled: true,
            retention: RetentionConfig::default(),
        }
    }
}

/// Data retention periods in days.
#[derive(Debug, Clone)]
pub struct RetentionConfig {
    /// Trade history retention (default: 90 days).
    pub trades_days: u32,
    /// Portfolio snapshots retention (default: 180 days).
    pub portfolio_snapshots_days: u32,
    /// Prediction history retention (default: 30 days).
    pub prediction_history_days: u32,
    /// Completed sync queue items retention (default: 7 days).
    pub sync_queue_days: u32,
    /// Node metrics retention (default: 1 day).
    pub node_metrics_days: u32,
    /// Funding payments retention (default: 365 days).
    pub funding_payments_days: u32,
    /// Margin history retention (default: 90 days).
    pub margin_history_days: u32,
    /// Sync progress/checkpoints retention (default: 7 days).
    pub sync_data_days: u32,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            trades_days: 90,
            portfolio_snapshots_days: 180,
            prediction_history_days: 30,
            sync_queue_days: 7,
            node_metrics_days: 1,
            funding_payments_days: 365,
            margin_history_days: 90,
            sync_data_days: 7,
        }
    }
}

/// Application configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// Server host address.
    pub host: String,
    /// Server port.
    pub port: u16,
    /// Redis URL for persistent caching.
    pub redis_url: Option<String>,
    /// CoinMarketCap API key.
    pub cmc_api_key: Option<String>,
    /// CoinGecko API key (optional, for pro tier).
    pub coingecko_api_key: Option<String>,
    /// CryptoCompare API key.
    pub cryptocompare_api_key: Option<String>,
    /// Binance API key (optional, public endpoints work without).
    pub binance_api_key: Option<String>,
    /// Kraken API key (optional).
    pub kraken_api_key: Option<String>,
    /// KuCoin API key (optional, public endpoints work without).
    pub kucoin_api_key: Option<String>,
    /// OKX API key (optional).
    pub okx_api_key: Option<String>,
    /// Huobi API key (optional).
    pub huobi_api_key: Option<String>,
    /// Finnhub API key for stock/ETF data.
    pub finnhub_api_key: Option<String>,
    /// Alpha Vantage API key for historical stock data.
    pub alpha_vantage_api_key: Option<String>,
    /// Alpaca API key for real-time stock data.
    pub alpaca_api_key: Option<String>,
    /// Alpaca API secret.
    pub alpaca_api_secret: Option<String>,
    /// Tiingo API key for real-time stock data.
    pub tiingo_api_key: Option<String>,
    /// Price change threshold for updates (percentage).
    pub price_change_threshold: f64,
    /// Throttle interval for price updates (ms).
    pub throttle_ms: u64,
    /// Stale threshold for price sources (ms).
    pub stale_threshold_ms: u64,
    /// This server's unique ID for peer mesh.
    pub server_id: String,
    /// This server's region/location.
    pub server_region: String,
    /// Peer servers for mesh connectivity.
    pub peer_servers: Vec<PeerServerConfig>,
    /// Bootstrap servers for initial mesh discovery.
    pub bootstrap_servers: Vec<BootstrapServerConfig>,
    /// This server's public WebSocket URL (for announcements).
    pub public_ws_url: String,
    /// This server's public API URL (for announcements).
    pub public_api_url: String,
    /// Mesh authentication configuration.
    pub mesh_auth: MeshAuthConfig,
    /// Storage management configuration.
    pub storage: StorageConfig,
}

impl Config {
    /// Load configuration from environment variables.
    pub fn from_env() -> Self {
        // Parse peer servers from PEER_SERVERS env var
        // Format: "id|region|ws_url|api_url,id2|region2|ws_url2|api_url2"
        let peer_servers = env::var("PEER_SERVERS")
            .ok()
            .map(|s| {
                s.split(',')
                    .filter_map(|peer| {
                        let parts: Vec<&str> = peer.split('|').collect();
                        if parts.len() >= 4 {
                            Some(PeerServerConfig {
                                id: parts[0].to_string(),
                                region: parts[1].to_string(),
                                ws_url: parts[2].to_string(),
                                api_url: parts[3].to_string(),
                            })
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Parse bootstrap servers from MESH_BOOTSTRAP_SERVERS env var
        // Format: "id|host:port,id2|host2:port2"
        let bootstrap_servers = env::var("MESH_BOOTSTRAP_SERVERS")
            .ok()
            .map(|s| {
                s.split(',')
                    .filter_map(|server| {
                        let parts: Vec<&str> = server.split('|').collect();
                        if parts.len() >= 2 {
                            Some(BootstrapServerConfig {
                                id: parts[0].to_string(),
                                address: parts[1].to_string(),
                            })
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port: u16 = env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3001);

        // Public URLs for mesh announcements (defaults to local address)
        let public_ws_url =
            env::var("PUBLIC_WS_URL").unwrap_or_else(|_| format!("ws://{}:{}/ws", host, port));
        let public_api_url =
            env::var("PUBLIC_API_URL").unwrap_or_else(|_| format!("http://{}:{}", host, port));

        Self {
            host,
            port,
            redis_url: env::var("REDIS_URL")
                .ok()
                .or_else(|| Some("redis://127.0.0.1:6379".to_string())),
            cmc_api_key: env::var("CMC_API_KEY").ok(),
            coingecko_api_key: env::var("COINGECKO_API_KEY").ok(),
            cryptocompare_api_key: env::var("CRYPTOCOMPARE_API_KEY").ok(),
            binance_api_key: env::var("BINANCE_API_KEY").ok(),
            kraken_api_key: env::var("KRAKEN_API_KEY").ok(),
            kucoin_api_key: env::var("KUCOIN_API_KEY").ok(),
            okx_api_key: env::var("OKX_API_KEY").ok(),
            huobi_api_key: env::var("HUOBI_API_KEY").ok(),
            finnhub_api_key: env::var("FINNHUB_API_KEY").ok(),
            alpha_vantage_api_key: env::var("ALPHA_VANTAGE_API_KEY").ok(),
            alpaca_api_key: env::var("ALPACA_API_KEY").ok(),
            alpaca_api_secret: env::var("ALPACA_API_SECRET").ok(),
            tiingo_api_key: env::var("TIINGO_API_KEY").ok(),
            price_change_threshold: env::var("PRICE_CHANGE_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.01),
            throttle_ms: env::var("THROTTLE_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100),
            stale_threshold_ms: env::var("STALE_THRESHOLD_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(120_000),
            server_id: env::var("SERVER_ID").unwrap_or_else(|_| {
                // Generate a random ID if not specified
                uuid::Uuid::new_v4().to_string()
            }),
            server_region: env::var("SERVER_REGION").unwrap_or_else(|_| "unknown".to_string()),
            peer_servers,
            bootstrap_servers,
            public_ws_url,
            public_api_url,
            mesh_auth: MeshAuthConfig {
                shared_key: env::var("MESH_SHARED_KEY").unwrap_or_default(),
                require_auth: env::var("MESH_REQUIRE_AUTH")
                    .ok()
                    .map(|v| v == "true" || v == "1")
                    .unwrap_or(false),
            },
            storage: StorageConfig {
                limit_mb: env::var("STORAGE_LIMIT_MB")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(30 * 1024), // 30 GB default
                warning_threshold_pct: env::var("STORAGE_WARNING_PCT")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(80),
                critical_threshold_pct: env::var("STORAGE_CRITICAL_PCT")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(95),
                auto_cleanup_enabled: env::var("STORAGE_AUTO_CLEANUP")
                    .ok()
                    .map(|v| v == "true" || v == "1")
                    .unwrap_or(true),
                retention: RetentionConfig {
                    trades_days: env::var("RETENTION_TRADES_DAYS")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(90),
                    portfolio_snapshots_days: env::var("RETENTION_SNAPSHOTS_DAYS")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(180),
                    prediction_history_days: env::var("RETENTION_PREDICTIONS_DAYS")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(30),
                    sync_queue_days: env::var("RETENTION_SYNC_QUEUE_DAYS")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(7),
                    node_metrics_days: env::var("RETENTION_METRICS_DAYS")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(1),
                    funding_payments_days: env::var("RETENTION_FUNDING_DAYS")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(365),
                    margin_history_days: env::var("RETENTION_MARGIN_DAYS")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(90),
                    sync_data_days: env::var("RETENTION_SYNC_DATA_DAYS")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(7),
                },
            },
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::from_env()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // PeerServerConfig Tests
    // =========================================================================

    #[test]
    fn test_peer_server_config_creation() {
        let config = PeerServerConfig {
            id: "tokyo".to_string(),
            region: "Asia Pacific".to_string(),
            ws_url: "wss://tokyo.example.com/ws".to_string(),
            api_url: "https://tokyo.example.com".to_string(),
        };

        assert_eq!(config.id, "tokyo");
        assert_eq!(config.region, "Asia Pacific");
        assert!(config.ws_url.starts_with("wss://"));
    }

    #[test]
    fn test_peer_server_config_clone() {
        let config = PeerServerConfig {
            id: "test".to_string(),
            region: "Test".to_string(),
            ws_url: "ws://test".to_string(),
            api_url: "http://test".to_string(),
        };

        let cloned = config.clone();
        assert_eq!(cloned.id, config.id);
        assert_eq!(cloned.region, config.region);
    }

    // =========================================================================
    // BootstrapServerConfig Tests
    // =========================================================================

    #[test]
    fn test_bootstrap_server_config_creation() {
        let config = BootstrapServerConfig {
            id: "bootstrap1".to_string(),
            address: "192.168.1.1:3001".to_string(),
        };

        assert_eq!(config.id, "bootstrap1");
        assert!(config.address.contains(":"));
    }

    // =========================================================================
    // MeshAuthConfig Tests
    // =========================================================================

    #[test]
    fn test_mesh_auth_config_creation() {
        let config = MeshAuthConfig {
            shared_key: "secret123".to_string(),
            require_auth: true,
        };

        assert_eq!(config.shared_key, "secret123");
        assert!(config.require_auth);
    }

    #[test]
    fn test_mesh_auth_config_disabled() {
        let config = MeshAuthConfig {
            shared_key: String::new(),
            require_auth: false,
        };

        assert!(config.shared_key.is_empty());
        assert!(!config.require_auth);
    }

    // =========================================================================
    // Config Tests
    // =========================================================================

    #[test]
    fn test_config_default_values() {
        // Note: This test may be affected by environment variables
        // In a clean environment, these defaults should apply
        let config = Config {
            host: "0.0.0.0".to_string(),
            port: 3001,
            redis_url: Some("redis://127.0.0.1:6379".to_string()),
            cmc_api_key: None,
            coingecko_api_key: None,
            cryptocompare_api_key: None,
            binance_api_key: None,
            kraken_api_key: None,
            kucoin_api_key: None,
            okx_api_key: None,
            huobi_api_key: None,
            finnhub_api_key: None,
            alpha_vantage_api_key: None,
            alpaca_api_key: None,
            alpaca_api_secret: None,
            tiingo_api_key: None,
            price_change_threshold: 0.01,
            throttle_ms: 100,
            stale_threshold_ms: 120_000,
            server_id: "test-server".to_string(),
            server_region: "unknown".to_string(),
            peer_servers: vec![],
            bootstrap_servers: vec![],
            public_ws_url: "ws://0.0.0.0:3001/ws".to_string(),
            public_api_url: "http://0.0.0.0:3001".to_string(),
            mesh_auth: MeshAuthConfig {
                shared_key: String::new(),
                require_auth: false,
            },
            storage: StorageConfig::default(),
        };

        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 3001);
        assert_eq!(config.price_change_threshold, 0.01);
        assert_eq!(config.throttle_ms, 100);
        assert_eq!(config.stale_threshold_ms, 120_000);
    }

    #[test]
    fn test_config_with_api_keys() {
        let config = Config {
            host: "localhost".to_string(),
            port: 8080,
            redis_url: None,
            cmc_api_key: Some("cmc-key".to_string()),
            coingecko_api_key: Some("gecko-key".to_string()),
            cryptocompare_api_key: Some("cc-key".to_string()),
            binance_api_key: Some("binance-key".to_string()),
            kraken_api_key: Some("kraken-key".to_string()),
            kucoin_api_key: None,
            okx_api_key: None,
            huobi_api_key: None,
            finnhub_api_key: Some("finnhub-key".to_string()),
            alpha_vantage_api_key: None,
            alpaca_api_key: None,
            alpaca_api_secret: None,
            tiingo_api_key: None,
            price_change_threshold: 0.05,
            throttle_ms: 200,
            stale_threshold_ms: 60_000,
            server_id: "prod-server".to_string(),
            server_region: "US East".to_string(),
            peer_servers: vec![],
            bootstrap_servers: vec![],
            public_ws_url: "wss://api.example.com/ws".to_string(),
            public_api_url: "https://api.example.com".to_string(),
            mesh_auth: MeshAuthConfig {
                shared_key: "production-secret".to_string(),
                require_auth: true,
            },
            storage: StorageConfig::default(),
        };

        assert_eq!(config.cmc_api_key, Some("cmc-key".to_string()));
        assert_eq!(config.finnhub_api_key, Some("finnhub-key".to_string()));
        assert!(config.mesh_auth.require_auth);
    }

    #[test]
    fn test_config_with_peer_servers() {
        let config = Config {
            host: "0.0.0.0".to_string(),
            port: 3001,
            redis_url: None,
            cmc_api_key: None,
            coingecko_api_key: None,
            cryptocompare_api_key: None,
            binance_api_key: None,
            kraken_api_key: None,
            kucoin_api_key: None,
            okx_api_key: None,
            huobi_api_key: None,
            finnhub_api_key: None,
            alpha_vantage_api_key: None,
            alpaca_api_key: None,
            alpaca_api_secret: None,
            tiingo_api_key: None,
            price_change_threshold: 0.01,
            throttle_ms: 100,
            stale_threshold_ms: 120_000,
            server_id: "us-east".to_string(),
            server_region: "US East".to_string(),
            peer_servers: vec![
                PeerServerConfig {
                    id: "eu-west".to_string(),
                    region: "EU West".to_string(),
                    ws_url: "wss://eu.example.com/ws".to_string(),
                    api_url: "https://eu.example.com".to_string(),
                },
                PeerServerConfig {
                    id: "asia".to_string(),
                    region: "Asia Pacific".to_string(),
                    ws_url: "wss://asia.example.com/ws".to_string(),
                    api_url: "https://asia.example.com".to_string(),
                },
            ],
            bootstrap_servers: vec![],
            public_ws_url: "wss://us.example.com/ws".to_string(),
            public_api_url: "https://us.example.com".to_string(),
            mesh_auth: MeshAuthConfig {
                shared_key: String::new(),
                require_auth: false,
            },
            storage: StorageConfig::default(),
        };

        assert_eq!(config.peer_servers.len(), 2);
        assert_eq!(config.peer_servers[0].id, "eu-west");
        assert_eq!(config.peer_servers[1].region, "Asia Pacific");
    }

    #[test]
    fn test_config_clone() {
        let config = Config {
            host: "test".to_string(),
            port: 1234,
            redis_url: Some("redis://test".to_string()),
            cmc_api_key: None,
            coingecko_api_key: None,
            cryptocompare_api_key: None,
            binance_api_key: None,
            kraken_api_key: None,
            kucoin_api_key: None,
            okx_api_key: None,
            huobi_api_key: None,
            finnhub_api_key: None,
            alpha_vantage_api_key: None,
            alpaca_api_key: None,
            alpaca_api_secret: None,
            tiingo_api_key: None,
            price_change_threshold: 0.01,
            throttle_ms: 100,
            stale_threshold_ms: 120_000,
            server_id: "test".to_string(),
            server_region: "test".to_string(),
            peer_servers: vec![],
            bootstrap_servers: vec![],
            public_ws_url: "ws://test/ws".to_string(),
            public_api_url: "http://test".to_string(),
            mesh_auth: MeshAuthConfig {
                shared_key: String::new(),
                require_auth: false,
            },
            storage: StorageConfig::default(),
        };

        let cloned = config.clone();
        assert_eq!(cloned.host, config.host);
        assert_eq!(cloned.port, config.port);
        assert_eq!(cloned.server_id, config.server_id);
    }
}
