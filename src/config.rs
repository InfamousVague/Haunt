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
        let public_ws_url = env::var("PUBLIC_WS_URL")
            .unwrap_or_else(|_| format!("ws://{}:{}/ws", host, port));
        let public_api_url = env::var("PUBLIC_API_URL")
            .unwrap_or_else(|_| format!("http://{}:{}", host, port));

        Self {
            host,
            port,
            redis_url: env::var("REDIS_URL").ok().or_else(|| Some("redis://127.0.0.1:6379".to_string())),
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
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::from_env()
    }
}
