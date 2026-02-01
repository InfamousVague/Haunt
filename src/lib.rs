//! Haunt - High-performance cryptocurrency price aggregation server

pub mod config;
pub mod error;
pub mod types;

/// Services module for library use (excludes multi_source which depends on sources)
pub mod services {
    //! Core services for price aggregation and caching

    mod cache_impl {
        use dashmap::DashMap;
        use std::time::{Duration, Instant};

        /// A thread-safe cache with TTL support.
        pub struct Cache<V> {
            data: DashMap<String, CacheEntry<V>>,
            default_ttl: Duration,
        }

        struct CacheEntry<V> {
            value: V,
            expires_at: Instant,
        }

        impl<V: Clone> Cache<V> {
            /// Create a new cache with the given default TTL.
            pub fn new(default_ttl: Duration) -> Self {
                Self {
                    data: DashMap::new(),
                    default_ttl,
                }
            }

            /// Get a value from the cache.
            pub fn get(&self, key: &str) -> Option<V> {
                let entry = self.data.get(key)?;
                if entry.expires_at > Instant::now() {
                    Some(entry.value.clone())
                } else {
                    drop(entry);
                    self.data.remove(key);
                    None
                }
            }

            /// Set a value in the cache with the default TTL.
            pub fn set(&self, key: String, value: V) {
                self.set_with_ttl(key, value, self.default_ttl);
            }

            /// Set a value in the cache with a custom TTL.
            pub fn set_with_ttl(&self, key: String, value: V, ttl: Duration) {
                self.data.insert(
                    key,
                    CacheEntry {
                        value,
                        expires_at: Instant::now() + ttl,
                    },
                );
            }

            /// Check if a key exists and is not expired.
            pub fn contains(&self, key: &str) -> bool {
                self.get(key).is_some()
            }

            /// Remove a value from the cache.
            pub fn remove(&self, key: &str) -> Option<V> {
                self.data.remove(key).map(|(_, entry)| entry.value)
            }

            /// Clear all entries from the cache.
            pub fn clear(&self) {
                self.data.clear();
            }

            /// Remove all expired entries from the cache.
            pub fn cleanup(&self) {
                let now = Instant::now();
                self.data.retain(|_, entry| entry.expires_at > now);
            }

            /// Get the number of entries in the cache (including expired).
            pub fn len(&self) -> usize {
                self.data.len()
            }

            /// Check if the cache is empty.
            pub fn is_empty(&self) -> bool {
                self.data.is_empty()
            }
        }
    }

    mod chart_store_impl {
        use crate::types::{ChartRange, ChartResolution, OhlcPoint};
        use dashmap::DashMap;
        use std::collections::VecDeque;
        use std::sync::Arc;

        /// OHLC bucket for a time period.
        #[derive(Debug, Clone)]
        struct OhlcBucket {
            time: i64,
            open: f64,
            high: f64,
            low: f64,
            close: f64,
            volume: f64,
        }

        impl OhlcBucket {
            fn new(time: i64, price: f64) -> Self {
                Self {
                    time,
                    open: price,
                    high: price,
                    low: price,
                    close: price,
                    volume: 0.0,
                }
            }

            fn update(&mut self, price: f64, volume: Option<f64>) {
                self.high = self.high.max(price);
                self.low = self.low.min(price);
                self.close = price;
                if let Some(v) = volume {
                    self.volume += v;
                }
            }

            fn to_ohlc_point(&self) -> OhlcPoint {
                OhlcPoint {
                    time: self.time,
                    open: self.open,
                    high: self.high,
                    low: self.low,
                    close: self.close,
                    volume: if self.volume > 0.0 { Some(self.volume) } else { None },
                }
            }
        }

        /// Time series data for a single resolution.
        #[derive(Debug)]
        struct TimeSeries {
            resolution: ChartResolution,
            buckets: VecDeque<OhlcBucket>,
            max_buckets: usize,
        }

        impl TimeSeries {
            fn new(resolution: ChartResolution) -> Self {
                let retention_seconds = resolution.retention_seconds();
                let bucket_seconds = resolution.seconds();
                let max_buckets = (retention_seconds / bucket_seconds) as usize;

                Self {
                    resolution,
                    buckets: VecDeque::with_capacity(max_buckets),
                    max_buckets,
                }
            }

            fn add_price(&mut self, price: f64, volume: Option<f64>, timestamp: i64) {
                let bucket_time = (timestamp / 1000) / self.resolution.seconds() * self.resolution.seconds();

                if let Some(last) = self.buckets.back_mut() {
                    if last.time == bucket_time {
                        last.update(price, volume);
                        return;
                    }
                }

                let bucket = OhlcBucket::new(bucket_time, price);
                self.buckets.push_back(bucket);

                while self.buckets.len() > self.max_buckets {
                    self.buckets.pop_front();
                }
            }

            fn get_data(&self, start_time: i64) -> Vec<OhlcPoint> {
                self.buckets
                    .iter()
                    .filter(|b| b.time >= start_time)
                    .map(|b| b.to_ohlc_point())
                    .collect()
            }
        }

        /// Symbol-specific chart data.
        #[derive(Debug)]
        struct SymbolChartData {
            one_minute: TimeSeries,
            five_minute: TimeSeries,
            one_hour: TimeSeries,
        }

        impl Default for SymbolChartData {
            fn default() -> Self {
                Self {
                    one_minute: TimeSeries::new(ChartResolution::OneMinute),
                    five_minute: TimeSeries::new(ChartResolution::FiveMinute),
                    one_hour: TimeSeries::new(ChartResolution::OneHour),
                }
            }
        }

        /// Chart data store with multiple resolutions.
        pub struct ChartStore {
            data: DashMap<String, SymbolChartData>,
        }

        impl ChartStore {
            /// Create a new chart store.
            pub fn new() -> Arc<Self> {
                Arc::new(Self {
                    data: DashMap::new(),
                })
            }

            /// Add a price point for a symbol.
            pub fn add_price(&self, symbol: &str, price: f64, volume: Option<f64>, timestamp: i64) {
                let symbol_lower = symbol.to_lowercase();
                let mut entry = self.data.entry(symbol_lower).or_default();
                let chart_data = entry.value_mut();

                chart_data.one_minute.add_price(price, volume, timestamp);
                chart_data.five_minute.add_price(price, volume, timestamp);
                chart_data.one_hour.add_price(price, volume, timestamp);
            }

            /// Get chart data for a symbol and range.
            pub fn get_chart(&self, symbol: &str, range: ChartRange) -> Vec<OhlcPoint> {
                let symbol_lower = symbol.to_lowercase();
                let entry = match self.data.get(&symbol_lower) {
                    Some(e) => e,
                    None => return Vec::new(),
                };

                let now = chrono::Utc::now().timestamp();
                let start_time = now - range.duration_seconds();

                match range {
                    ChartRange::OneHour | ChartRange::FourHours => {
                        entry.one_minute.get_data(start_time)
                    }
                    ChartRange::OneDay => {
                        entry.five_minute.get_data(start_time)
                    }
                    ChartRange::OneWeek | ChartRange::OneMonth => {
                        entry.one_hour.get_data(start_time)
                    }
                }
            }
        }

        impl Default for ChartStore {
            fn default() -> Self {
                Self {
                    data: DashMap::new(),
                }
            }
        }
    }

    pub use cache_impl::Cache;
    pub use chart_store_impl::ChartStore;
}

// Re-export commonly used types
pub use types::*;
pub use services::{Cache, ChartStore};
