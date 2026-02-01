use crate::types::{AggregatedPrice, AggregationConfig, PriceSource, SourcePrice};
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;

/// Cached price data for a symbol.
#[derive(Debug, Clone)]
struct SymbolPrice {
    /// Prices from each source.
    sources: Vec<SourcePrice>,
    /// Last aggregated price.
    last_aggregated: Option<f64>,
    /// Last update time for throttling.
    last_update_time: Instant,
}

impl Default for SymbolPrice {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            last_aggregated: None,
            last_update_time: Instant::now(),
        }
    }
}

/// Multi-source price aggregation cache.
pub struct PriceCache {
    /// Price data keyed by symbol.
    prices: DashMap<String, SymbolPrice>,
    /// Aggregation configuration.
    config: AggregationConfig,
    /// Broadcast channel for price updates.
    tx: broadcast::Sender<AggregatedPrice>,
}

impl PriceCache {
    /// Create a new price cache.
    pub fn new(config: AggregationConfig) -> (Arc<Self>, broadcast::Receiver<AggregatedPrice>) {
        let (tx, rx) = broadcast::channel(1024);
        let cache = Arc::new(Self {
            prices: DashMap::new(),
            config,
            tx,
        });
        (cache, rx)
    }

    /// Subscribe to price updates.
    pub fn subscribe(&self) -> broadcast::Receiver<AggregatedPrice> {
        self.tx.subscribe()
    }

    /// Update a price from a source.
    pub fn update_price(&self, symbol: &str, source: PriceSource, price: f64, volume_24h: Option<f64>) {
        let now = Instant::now();
        let timestamp = chrono::Utc::now().timestamp_millis();
        let symbol_lower = symbol.to_lowercase();

        let mut entry = self.prices.entry(symbol_lower.clone()).or_default();
        let symbol_price = entry.value_mut();

        // Update or add source price
        let source_price = SourcePrice {
            source,
            price,
            timestamp,
            volume_24h,
        };

        if let Some(existing) = symbol_price.sources.iter_mut().find(|s| s.source == source) {
            *existing = source_price;
        } else {
            symbol_price.sources.push(source_price);
        }

        // Remove stale sources
        let stale_threshold = timestamp - self.config.stale_threshold_ms as i64;
        symbol_price.sources.retain(|s| s.timestamp > stale_threshold);

        // Check throttle
        let elapsed_ms = now.duration_since(symbol_price.last_update_time).as_millis() as u64;
        if elapsed_ms < self.config.throttle_ms {
            return;
        }

        // Calculate weighted average price
        let aggregated = self.aggregate(&symbol_price.sources);

        // Check change threshold
        if let Some(last_price) = symbol_price.last_aggregated {
            let change_pct = ((aggregated - last_price) / last_price * 100.0).abs();
            if change_pct < self.config.change_threshold {
                return;
            }
        }

        // Emit update
        let primary_source = symbol_price
            .sources
            .iter()
            .max_by_key(|s| s.source.weight())
            .map(|s| s.source)
            .unwrap_or(source);

        let sources: Vec<PriceSource> = symbol_price.sources.iter().map(|s| s.source).collect();

        let update = AggregatedPrice {
            id: symbol_lower.clone(),
            symbol: symbol_lower,
            price: aggregated,
            previous_price: symbol_price.last_aggregated,
            change_24h: None, // Would need 24h historical data
            volume_24h,
            source: primary_source,
            sources,
            timestamp,
        };

        symbol_price.last_aggregated = Some(aggregated);
        symbol_price.last_update_time = now;

        drop(entry);

        // Broadcast update (ignore errors if no receivers)
        let _ = self.tx.send(update);
    }

    /// Calculate weighted average price from sources.
    fn aggregate(&self, sources: &[SourcePrice]) -> f64 {
        if sources.is_empty() {
            return 0.0;
        }

        let mut total_weight = 0u32;
        let mut weighted_sum = 0.0;

        for source in sources {
            let weight = source.source.weight();
            total_weight += weight;
            weighted_sum += source.price * weight as f64;
        }

        if total_weight == 0 {
            sources[0].price
        } else {
            weighted_sum / total_weight as f64
        }
    }

    /// Get the current aggregated price for a symbol.
    pub fn get_price(&self, symbol: &str) -> Option<f64> {
        let entry = self.prices.get(&symbol.to_lowercase())?;
        entry.last_aggregated
    }

    /// Get all current prices.
    pub fn get_all_prices(&self) -> Vec<(String, f64)> {
        self.prices
            .iter()
            .filter_map(|entry| {
                entry.last_aggregated.map(|price| (entry.key().clone(), price))
            })
            .collect()
    }
}
