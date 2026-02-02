use crate::config::Config;
use crate::services::{ChartStore, PriceCache};
use crate::sources::{
    BinanceClient, CoinGeckoClient, CoinMarketCapClient, CoinbaseWs, CryptoCompareClient,
    HuobiClient, KrakenClient, KuCoinClient, OkxClient,
};
use crate::types::{AggregatedPrice, AggregationConfig};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{error, info};

/// Coordinates multiple price sources.
pub struct MultiSourceCoordinator {
    price_cache: Arc<PriceCache>,
    chart_store: Arc<ChartStore>,
    coinbase_ws: Option<CoinbaseWs>,
    coingecko: Option<CoinGeckoClient>,
    cryptocompare: Option<CryptoCompareClient>,
    coinmarketcap: Option<CoinMarketCapClient>,
    binance: Option<BinanceClient>,
    kraken: Option<KrakenClient>,
    kucoin: Option<KuCoinClient>,
    okx: Option<OkxClient>,
    huobi: Option<HuobiClient>,
}

impl MultiSourceCoordinator {
    /// Create a new coordinator.
    pub fn new(config: &Config) -> (Arc<Self>, broadcast::Receiver<AggregatedPrice>) {
        let agg_config = AggregationConfig {
            change_threshold: config.price_change_threshold,
            throttle_ms: config.throttle_ms,
            stale_threshold_ms: config.stale_threshold_ms,
        };

        let (price_cache, rx) = PriceCache::new(agg_config);
        let chart_store = ChartStore::new();

        let coinbase_ws = Some(CoinbaseWs::new(price_cache.clone(), chart_store.clone()));

        let coingecko = Some(CoinGeckoClient::new(
            config.coingecko_api_key.clone(),
            price_cache.clone(),
            chart_store.clone(),
        ));

        let cryptocompare = config.cryptocompare_api_key.as_ref().map(|key| {
            CryptoCompareClient::new(key.clone(), price_cache.clone(), chart_store.clone())
        });

        let coinmarketcap = config.cmc_api_key.as_ref().map(|key| {
            CoinMarketCapClient::new(key.clone(), price_cache.clone(), chart_store.clone())
        });

        // New exchange sources - all work without API keys (public endpoints)
        let binance = Some(BinanceClient::new(
            config.binance_api_key.clone(),
            price_cache.clone(),
            chart_store.clone(),
        ));

        let kraken = Some(KrakenClient::new(
            config.kraken_api_key.clone(),
            price_cache.clone(),
            chart_store.clone(),
        ));

        let kucoin = Some(KuCoinClient::new(
            config.kucoin_api_key.clone(),
            price_cache.clone(),
            chart_store.clone(),
        ));

        let okx = Some(OkxClient::new(
            config.okx_api_key.clone(),
            price_cache.clone(),
            chart_store.clone(),
        ));

        let huobi = Some(HuobiClient::new(
            config.huobi_api_key.clone(),
            price_cache.clone(),
            chart_store.clone(),
        ));

        let coordinator = Arc::new(Self {
            price_cache,
            chart_store,
            coinbase_ws,
            coingecko,
            cryptocompare,
            coinmarketcap,
            binance,
            kraken,
            kucoin,
            okx,
            huobi,
        });

        (coordinator, rx)
    }

    /// Get the price cache.
    pub fn price_cache(&self) -> Arc<PriceCache> {
        self.price_cache.clone()
    }

    /// Get the chart store.
    pub fn chart_store(&self) -> Arc<ChartStore> {
        self.chart_store.clone()
    }

    /// Start all price sources.
    pub async fn start(&self) {
        info!("Starting multi-source coordinator with 9 data sources");

        // Start Coinbase WebSocket
        if let Some(ref ws) = self.coinbase_ws {
            let ws = ws.clone();
            tokio::spawn(async move {
                if let Err(e) = ws.connect().await {
                    error!("Coinbase WebSocket error: {}", e);
                }
            });
        }

        // Start CoinGecko polling
        if let Some(ref client) = self.coingecko {
            let client = client.clone();
            tokio::spawn(async move {
                client.start_polling().await;
            });
        }

        // Start CryptoCompare polling
        if let Some(ref client) = self.cryptocompare {
            let client = client.clone();
            tokio::spawn(async move {
                client.start_polling().await;
            });
        }

        // Start CoinMarketCap polling
        if let Some(ref client) = self.coinmarketcap {
            let client = client.clone();
            tokio::spawn(async move {
                client.start_polling().await;
            });
        }

        // Start Binance polling
        if let Some(ref client) = self.binance {
            let client = client.clone();
            tokio::spawn(async move {
                client.start_polling().await;
            });
        }

        // Start Kraken polling
        if let Some(ref client) = self.kraken {
            let client = client.clone();
            tokio::spawn(async move {
                client.start_polling().await;
            });
        }

        // Start KuCoin polling
        if let Some(ref client) = self.kucoin {
            let client = client.clone();
            tokio::spawn(async move {
                client.start_polling().await;
            });
        }

        // Start OKX polling
        if let Some(ref client) = self.okx {
            let client = client.clone();
            tokio::spawn(async move {
                client.start_polling().await;
            });
        }

        // Start Huobi polling
        if let Some(ref client) = self.huobi {
            let client = client.clone();
            tokio::spawn(async move {
                client.start_polling().await;
            });
        }
    }

    /// Subscribe to price updates.
    pub fn subscribe(&self) -> broadcast::Receiver<AggregatedPrice> {
        self.price_cache.subscribe()
    }

    /// Subscribe to assets via Coinbase WebSocket.
    pub async fn subscribe_assets(&self, symbols: &[String]) {
        if let Some(ref ws) = self.coinbase_ws {
            ws.subscribe(symbols).await;
        }
    }

    /// Unsubscribe from assets via Coinbase WebSocket.
    pub async fn unsubscribe_assets(&self, symbols: &[String]) {
        if let Some(ref ws) = self.coinbase_ws {
            ws.unsubscribe(symbols).await;
        }
    }
}
