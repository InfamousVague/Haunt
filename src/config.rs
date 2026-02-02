use std::env;

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
}

impl Config {
    /// Load configuration from environment variables.
    pub fn from_env() -> Self {
        Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3001),
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
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::from_env()
    }
}
