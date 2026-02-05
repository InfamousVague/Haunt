//! Exchange metrics tracking service.
//!
//! Tracks per-exchange:
//! - Request latency (min/avg/max over rolling 60s window)
//! - Volume dominance (share of total trading volume)
//! - Connection status and health
//! - Error rates

use crate::types::PriceSource;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Rolling window duration for latency calculations.
const LATENCY_WINDOW_SECS: u64 = 60;

/// Maximum number of latency samples to keep per exchange.
const MAX_LATENCY_SAMPLES: usize = 1000;

/// A single latency measurement.
#[derive(Debug, Clone)]
struct LatencySample {
    /// Latency in milliseconds.
    latency_ms: u64,
    /// When this sample was recorded.
    timestamp: Instant,
}

/// Latency statistics for an exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencyStats {
    /// Minimum latency in the window (ms).
    pub min_ms: u64,
    /// Average latency in the window (ms).
    pub avg_ms: f64,
    /// Maximum latency in the window (ms).
    pub max_ms: u64,
    /// Number of samples in the window.
    pub sample_count: usize,
    /// 50th percentile (median) latency (ms).
    pub p50_ms: u64,
    /// 95th percentile latency (ms).
    pub p95_ms: u64,
    /// 99th percentile latency (ms).
    pub p99_ms: u64,
}

impl Default for LatencyStats {
    fn default() -> Self {
        Self {
            min_ms: 0,
            avg_ms: 0.0,
            max_ms: 0,
            sample_count: 0,
            p50_ms: 0,
            p95_ms: 0,
            p99_ms: 0,
        }
    }
}

/// Volume tracking for dominance calculation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeStats {
    /// Total volume reported by this exchange (24h, USD).
    pub volume_24h_usd: f64,
    /// Share of total volume across all exchanges (0-100%).
    pub dominance_pct: f64,
    /// Number of symbols reporting volume from this exchange.
    pub symbol_count: u64,
    /// Last update timestamp (unix ms).
    pub last_update: i64,
}

impl Default for VolumeStats {
    fn default() -> Self {
        Self {
            volume_24h_usd: 0.0,
            dominance_pct: 0.0,
            symbol_count: 0,
            last_update: 0,
        }
    }
}

/// Health status for an exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeHealth {
    /// Whether the exchange is currently connected/responsive.
    pub online: bool,
    /// Number of successful requests in the window.
    pub success_count: u64,
    /// Number of failed requests in the window.
    pub error_count: u64,
    /// Success rate (0-100%).
    pub success_rate: f64,
    /// Last error message (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    /// Last successful response timestamp (unix ms).
    pub last_success: i64,
}

impl Default for ExchangeHealth {
    fn default() -> Self {
        Self {
            online: false,
            success_count: 0,
            error_count: 0,
            success_rate: 100.0,
            last_error: None,
            last_success: 0,
        }
    }
}

/// Complete metrics for an exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeMetrics {
    /// The exchange/source identifier.
    pub source: PriceSource,
    /// Latency statistics.
    pub latency: LatencyStats,
    /// Volume and dominance statistics.
    pub volume: VolumeStats,
    /// Health status.
    pub health: ExchangeHealth,
}

/// Per-exchange tracking state (internal).
struct ExchangeState {
    /// Rolling latency samples.
    latency_samples: Mutex<VecDeque<LatencySample>>,
    /// Total volume reported (24h USD).
    volume_24h: AtomicU64,
    /// Symbol count reporting volume.
    symbol_count: AtomicU64,
    /// Success counter.
    success_count: AtomicU64,
    /// Error counter.
    error_count: AtomicU64,
    /// Last success timestamp (unix ms).
    last_success: AtomicU64,
    /// Last error message.
    last_error: Mutex<Option<String>>,
    /// Last volume update timestamp.
    volume_last_update: AtomicU64,
}

impl Default for ExchangeState {
    fn default() -> Self {
        Self {
            latency_samples: Mutex::new(VecDeque::with_capacity(MAX_LATENCY_SAMPLES)),
            volume_24h: AtomicU64::new(0),
            symbol_count: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            last_success: AtomicU64::new(0),
            last_error: Mutex::new(None),
            volume_last_update: AtomicU64::new(0),
        }
    }
}

/// Exchange metrics tracking service.
pub struct ExchangeMetricsService {
    /// Per-exchange state.
    exchanges: DashMap<PriceSource, ExchangeState>,
}

impl ExchangeMetricsService {
    /// Create a new exchange metrics service.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            exchanges: DashMap::new(),
        })
    }

    /// Record a latency measurement for an exchange.
    pub fn record_latency(&self, source: PriceSource, latency_ms: u64) {
        // First, ensure the entry exists
        self.exchanges.entry(source).or_default();

        // Then get a reference and work with it
        if let Some(state) = self.exchanges.get(&source) {
            let sample = LatencySample {
                latency_ms,
                timestamp: Instant::now(),
            };

            if let Ok(mut samples) = state.latency_samples.lock() {
                // Add new sample
                samples.push_back(sample);

                // Remove old samples outside the window
                let cutoff = Instant::now() - Duration::from_secs(LATENCY_WINDOW_SECS);
                while let Some(front) = samples.front() {
                    if front.timestamp < cutoff {
                        samples.pop_front();
                    } else {
                        break;
                    }
                }

                // Cap at max samples
                while samples.len() > MAX_LATENCY_SAMPLES {
                    samples.pop_front();
                }
            }
        }
    }

    /// Record a successful request.
    pub fn record_success(&self, source: PriceSource) {
        // Ensure entry exists
        self.exchanges.entry(source).or_default();

        if let Some(state) = self.exchanges.get(&source) {
            state.success_count.fetch_add(1, Ordering::Relaxed);
            state
                .last_success
                .store(chrono::Utc::now().timestamp_millis() as u64, Ordering::Relaxed);

            // Clear last error on success
            if let Ok(mut err) = state.last_error.lock() {
                *err = None;
            }
        }
    }

    /// Record a failed request with error message.
    pub fn record_error(&self, source: PriceSource, error: &str) {
        // Ensure entry exists
        self.exchanges.entry(source).or_default();

        if let Some(state) = self.exchanges.get(&source) {
            state.error_count.fetch_add(1, Ordering::Relaxed);

            if let Ok(mut err) = state.last_error.lock() {
                *err = Some(error.to_string());
            }
        }
    }

    /// Update volume data for an exchange.
    pub fn update_volume(&self, source: PriceSource, volume_24h_usd: f64, symbol_count: u64) {
        // Ensure entry exists
        self.exchanges.entry(source).or_default();

        if let Some(state) = self.exchanges.get(&source) {
            // Store volume as u64 bits (f64::to_bits)
            state
                .volume_24h
                .store(volume_24h_usd.to_bits(), Ordering::Relaxed);
            state.symbol_count.store(symbol_count, Ordering::Relaxed);
            state
                .volume_last_update
                .store(chrono::Utc::now().timestamp_millis() as u64, Ordering::Relaxed);
        }
    }

    /// Get latency statistics for an exchange.
    pub fn get_latency_stats(&self, source: PriceSource) -> LatencyStats {
        let Some(state) = self.exchanges.get(&source) else {
            return LatencyStats::default();
        };

        let Ok(samples) = state.latency_samples.lock() else {
            return LatencyStats::default();
        };

        // Filter to only samples in the current window
        let cutoff = Instant::now() - Duration::from_secs(LATENCY_WINDOW_SECS);
        let recent: Vec<u64> = samples
            .iter()
            .filter(|s| s.timestamp >= cutoff)
            .map(|s| s.latency_ms)
            .collect();

        if recent.is_empty() {
            return LatencyStats::default();
        }

        let mut sorted = recent.clone();
        sorted.sort_unstable();

        let sample_count = sorted.len();
        let min_ms = *sorted.first().unwrap_or(&0);
        let max_ms = *sorted.last().unwrap_or(&0);
        let sum: u64 = sorted.iter().sum();
        let avg_ms = sum as f64 / sample_count as f64;

        // Percentiles
        let p50_idx = (sample_count as f64 * 0.50).floor() as usize;
        let p95_idx = (sample_count as f64 * 0.95).floor() as usize;
        let p99_idx = (sample_count as f64 * 0.99).floor() as usize;

        let p50_ms = sorted.get(p50_idx.min(sample_count - 1)).copied().unwrap_or(0);
        let p95_ms = sorted.get(p95_idx.min(sample_count - 1)).copied().unwrap_or(0);
        let p99_ms = sorted.get(p99_idx.min(sample_count - 1)).copied().unwrap_or(0);

        LatencyStats {
            min_ms,
            avg_ms,
            max_ms,
            sample_count,
            p50_ms,
            p95_ms,
            p99_ms,
        }
    }

    /// Get health status for an exchange.
    pub fn get_health(&self, source: PriceSource) -> ExchangeHealth {
        let Some(state) = self.exchanges.get(&source) else {
            return ExchangeHealth::default();
        };

        let success_count = state.success_count.load(Ordering::Relaxed);
        let error_count = state.error_count.load(Ordering::Relaxed);
        let total = success_count + error_count;
        let success_rate = if total > 0 {
            (success_count as f64 / total as f64) * 100.0
        } else {
            100.0
        };

        let last_success = state.last_success.load(Ordering::Relaxed) as i64;
        let now_ms = chrono::Utc::now().timestamp_millis();

        // Consider online if had a successful update in the last 5 minutes
        let online = last_success > 0 && (now_ms - last_success) < 300_000;

        let last_error = state.last_error.lock().ok().and_then(|e| e.clone());

        ExchangeHealth {
            online,
            success_count,
            error_count,
            success_rate,
            last_error,
            last_success,
        }
    }

    /// Get volume statistics for all exchanges with dominance calculation.
    pub fn get_all_volume_stats(&self) -> Vec<(PriceSource, VolumeStats)> {
        // First pass: collect raw volumes
        let raw_volumes: Vec<(PriceSource, f64, u64, i64)> = self
            .exchanges
            .iter()
            .map(|entry| {
                let source = *entry.key();
                let vol = f64::from_bits(entry.volume_24h.load(Ordering::Relaxed));
                let count = entry.symbol_count.load(Ordering::Relaxed);
                let last_update = entry.volume_last_update.load(Ordering::Relaxed) as i64;
                (source, vol, count, last_update)
            })
            .collect();

        // Calculate total volume
        let total_volume: f64 = raw_volumes.iter().map(|(_, v, _, _)| *v).sum();

        // Build stats with dominance
        raw_volumes
            .into_iter()
            .map(|(source, volume, symbol_count, last_update)| {
                let dominance_pct = if total_volume > 0.0 {
                    (volume / total_volume) * 100.0
                } else {
                    0.0
                };

                (
                    source,
                    VolumeStats {
                        volume_24h_usd: volume,
                        dominance_pct,
                        symbol_count,
                        last_update,
                    },
                )
            })
            .collect()
    }

    /// Get complete metrics for an exchange.
    pub fn get_metrics(&self, source: PriceSource) -> ExchangeMetrics {
        let latency = self.get_latency_stats(source);
        let health = self.get_health(source);

        // Get volume with dominance
        let all_volumes = self.get_all_volume_stats();
        let volume = all_volumes
            .into_iter()
            .find(|(s, _)| *s == source)
            .map(|(_, v)| v)
            .unwrap_or_default();

        ExchangeMetrics {
            source,
            latency,
            volume,
            health,
        }
    }

    /// Get metrics for all exchanges.
    pub fn get_all_metrics(&self) -> Vec<ExchangeMetrics> {
        // Get all volumes with dominance calculated
        let all_volumes = self.get_all_volume_stats();
        let volume_map: std::collections::HashMap<PriceSource, VolumeStats> =
            all_volumes.into_iter().collect();

        self.exchanges
            .iter()
            .map(|entry| {
                let source = *entry.key();
                let latency = self.get_latency_stats(source);
                let health = self.get_health(source);
                let volume = volume_map.get(&source).cloned().unwrap_or_default();

                ExchangeMetrics {
                    source,
                    latency,
                    volume,
                    health,
                }
            })
            .collect()
    }

    /// Get aggregated latency summary across all exchanges.
    pub fn get_aggregated_latency(&self) -> LatencyStats {
        let all_latencies: Vec<u64> = self
            .exchanges
            .iter()
            .flat_map(|entry| {
                let source = *entry.key();
                let stats = self.get_latency_stats(source);
                if stats.sample_count > 0 {
                    vec![stats.avg_ms as u64]
                } else {
                    vec![]
                }
            })
            .collect();

        if all_latencies.is_empty() {
            return LatencyStats::default();
        }

        let mut sorted = all_latencies.clone();
        sorted.sort_unstable();

        let sample_count = sorted.len();
        let min_ms = *sorted.first().unwrap_or(&0);
        let max_ms = *sorted.last().unwrap_or(&0);
        let sum: u64 = sorted.iter().sum();
        let avg_ms = sum as f64 / sample_count as f64;

        let p50_idx = (sample_count as f64 * 0.50).floor() as usize;
        let p95_idx = (sample_count as f64 * 0.95).floor() as usize;
        let p99_idx = (sample_count as f64 * 0.99).floor() as usize;

        let p50_ms = sorted.get(p50_idx.min(sample_count - 1)).copied().unwrap_or(0);
        let p95_ms = sorted.get(p95_idx.min(sample_count - 1)).copied().unwrap_or(0);
        let p99_ms = sorted.get(p99_idx.min(sample_count - 1)).copied().unwrap_or(0);

        LatencyStats {
            min_ms,
            avg_ms,
            max_ms,
            sample_count,
            p50_ms,
            p95_ms,
            p99_ms,
        }
    }

    /// Reset counters for an exchange (useful for rolling windows).
    pub fn reset_counters(&self, source: PriceSource) {
        if let Some(state) = self.exchanges.get(&source) {
            state.success_count.store(0, Ordering::Relaxed);
            state.error_count.store(0, Ordering::Relaxed);
        }
    }

    /// Get total number of tracked exchanges.
    pub fn exchange_count(&self) -> usize {
        self.exchanges.len()
    }

    /// Get list of tracked exchange sources.
    pub fn tracked_sources(&self) -> Vec<PriceSource> {
        self.exchanges.iter().map(|e| *e.key()).collect()
    }
}

impl Default for ExchangeMetricsService {
    fn default() -> Self {
        Self {
            exchanges: DashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_recording() {
        let service = ExchangeMetricsService::new();

        // Record some latencies
        service.record_latency(PriceSource::Binance, 50);
        service.record_latency(PriceSource::Binance, 100);
        service.record_latency(PriceSource::Binance, 75);

        let stats = service.get_latency_stats(PriceSource::Binance);
        assert_eq!(stats.sample_count, 3);
        assert_eq!(stats.min_ms, 50);
        assert_eq!(stats.max_ms, 100);
    }

    #[test]
    fn test_health_tracking() {
        let service = ExchangeMetricsService::new();

        service.record_success(PriceSource::Coinbase);
        service.record_success(PriceSource::Coinbase);
        service.record_error(PriceSource::Coinbase, "Connection timeout");

        let health = service.get_health(PriceSource::Coinbase);
        assert_eq!(health.success_count, 2);
        assert_eq!(health.error_count, 1);
        assert!((health.success_rate - 66.67).abs() < 1.0);
    }

    #[test]
    fn test_volume_dominance() {
        let service = ExchangeMetricsService::new();

        service.update_volume(PriceSource::Binance, 1_000_000.0, 10);
        service.update_volume(PriceSource::Coinbase, 500_000.0, 8);
        service.update_volume(PriceSource::Kraken, 500_000.0, 5);

        let volumes = service.get_all_volume_stats();

        let binance = volumes.iter().find(|(s, _)| *s == PriceSource::Binance);
        assert!(binance.is_some());
        let (_, binance_stats) = binance.unwrap();
        assert!((binance_stats.dominance_pct - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_complete_metrics() {
        let service = ExchangeMetricsService::new();

        service.record_latency(PriceSource::Binance, 100);
        service.record_success(PriceSource::Binance);
        service.update_volume(PriceSource::Binance, 1_000_000.0, 10);

        let metrics = service.get_metrics(PriceSource::Binance);
        assert_eq!(metrics.source, PriceSource::Binance);
        assert_eq!(metrics.latency.sample_count, 1);
        assert_eq!(metrics.health.success_count, 1);
        assert!(metrics.volume.volume_24h_usd > 0.0);
    }
}
