//! Signal store for computing and caching trading signals.

use crate::services::signals::indicators::all_indicators;
use crate::services::signals::{AccuracyStore, PredictionStore, Signal};
use crate::services::ChartStore;
use crate::types::{
    Recommendation, SignalCategory, SignalDirection, SignalOutput, SignalPrediction,
    SymbolSignals, TradingTimeframe,
};
use dashmap::DashMap;
use std::sync::Arc;
use tracing::debug;

/// Cache entry for computed signals.
struct CachedSignals {
    signals: SymbolSignals,
    computed_at: i64,
}

/// Store for computing and caching trading signals.
pub struct SignalStore {
    chart_store: Arc<ChartStore>,
    /// Cache key format: "{symbol}:{timeframe}"
    cache: DashMap<String, CachedSignals>,
    indicators: Vec<Box<dyn Signal>>,
    prediction_store: Arc<PredictionStore>,
    accuracy_store: Arc<AccuracyStore>,
    /// Cache TTL in milliseconds.
    cache_ttl_ms: i64,
}

impl SignalStore {
    /// Create a new signal store.
    pub fn new(
        chart_store: Arc<ChartStore>,
        prediction_store: Arc<PredictionStore>,
        accuracy_store: Arc<AccuracyStore>,
    ) -> Arc<Self> {
        Arc::new(Self {
            chart_store,
            cache: DashMap::new(),
            indicators: all_indicators(),
            prediction_store,
            accuracy_store,
            cache_ttl_ms: 30_000, // 30 seconds
        })
    }

    /// Get signals for a symbol with specified trading timeframe.
    pub async fn get_signals(
        &self,
        symbol: &str,
        timeframe: TradingTimeframe,
    ) -> Option<SymbolSignals> {
        let symbol_lower = symbol.to_lowercase();
        let cache_key = format!("{}:{:?}", symbol_lower, timeframe);
        let now = chrono::Utc::now().timestamp_millis();

        // Check cache
        if let Some(cached) = self.cache.get(&cache_key) {
            if now - cached.computed_at < self.cache_ttl_ms {
                return Some(cached.signals.clone());
            }
        }

        // Compute signals
        let signals = self.compute_signals(&symbol_lower, timeframe).await?;

        // Cache result
        self.cache.insert(
            cache_key,
            CachedSignals {
                signals: signals.clone(),
                computed_at: now,
            },
        );

        Some(signals)
    }

    /// Compute signals for a symbol with specified trading timeframe.
    async fn compute_signals(
        &self,
        symbol: &str,
        timeframe: TradingTimeframe,
    ) -> Option<SymbolSignals> {
        // Get OHLC data using timeframe-appropriate range
        let chart_range = timeframe.chart_range();
        let candles = self.chart_store.get_chart(symbol, chart_range);

        if candles.is_empty() {
            debug!(
                "No candles for symbol {} at {:?} - cannot compute signals",
                symbol, timeframe
            );
            return None;
        }

        debug!(
            "Computing {:?} signals for {} with {} candles",
            timeframe,
            symbol,
            candles.len()
        );

        let mut signals = Vec::new();
        let current_price = candles.last()?.close;

        // Calculate each indicator
        for indicator in &self.indicators {
            if candles.len() >= indicator.min_periods() {
                if let Some(mut signal) = indicator.calculate(&candles) {
                    // Add accuracy data if available
                    // Use timeframe-specific accuracy validation period
                    let accuracy_timeframe = match timeframe {
                        TradingTimeframe::Scalping => "1h",
                        TradingTimeframe::DayTrading => "4h",
                        TradingTimeframe::SwingTrading => "24h",
                        TradingTimeframe::PositionTrading => "24h",
                    };

                    if let Some(accuracy) = self
                        .accuracy_store
                        .get_accuracy(indicator.id(), symbol, accuracy_timeframe)
                        .await
                    {
                        signal.accuracy = Some(accuracy.accuracy_pct);
                        signal.sample_size = Some(accuracy.total_predictions);
                    }

                    signals.push(signal);
                }
            }
        }

        if signals.is_empty() {
            return None;
        }

        // Calculate composite scores
        let trend_score = Self::calculate_category_score(&signals, SignalCategory::Trend);
        let momentum_score = Self::calculate_category_score(&signals, SignalCategory::Momentum);
        let volatility_score = Self::calculate_category_score(&signals, SignalCategory::Volatility);
        let volume_score = Self::calculate_category_score(&signals, SignalCategory::Volume);

        // Get weights based on trading timeframe
        let (trend_w, momentum_w, volatility_w, volume_w) = timeframe.category_weights();

        // Calculate weighted composite score
        let composite_score = ((trend_score as f64 * trend_w)
            + (momentum_score as f64 * momentum_w)
            + (volatility_score as f64 * volatility_w)
            + (volume_score as f64 * volume_w)) as i8;

        let direction = SignalDirection::from_score(composite_score);
        let timestamp = chrono::Utc::now().timestamp_millis();

        let symbol_signals = SymbolSignals {
            symbol: symbol.to_uppercase(),
            timeframe,
            signals,
            trend_score,
            momentum_score,
            volatility_score,
            volume_score,
            composite_score,
            direction,
            timestamp,
        };

        // Record predictions for accuracy tracking
        self.record_predictions(&symbol_signals, current_price).await;

        Some(symbol_signals)
    }

    /// Calculate composite score for a category.
    fn calculate_category_score(signals: &[SignalOutput], category: SignalCategory) -> i8 {
        let category_signals: Vec<&SignalOutput> =
            signals.iter().filter(|s| s.category == category).collect();

        if category_signals.is_empty() {
            return 0;
        }

        // Weighted average - give more weight to signals with better accuracy
        let mut total_weight = 0.0;
        let mut weighted_sum = 0.0;

        for signal in &category_signals {
            let weight = if let Some(accuracy) = signal.accuracy {
                // Weight by accuracy (50-100 range normalized to 0.5-1.0)
                (accuracy / 100.0).max(0.5)
            } else {
                1.0 // Default weight for new signals
            };

            weighted_sum += signal.score as f64 * weight;
            total_weight += weight;
        }

        if total_weight > 0.0 {
            (weighted_sum / total_weight) as i8
        } else {
            0
        }
    }

    /// Record predictions for accuracy tracking.
    async fn record_predictions(&self, signals: &SymbolSignals, current_price: f64) {
        for signal in &signals.signals {
            // Only record non-neutral predictions
            if signal.score.abs() >= 20 {
                let prediction = SignalPrediction::new(
                    signals.symbol.clone(),
                    signal.name.clone(),
                    signal.direction,
                    signal.score,
                    current_price,
                );
                self.prediction_store.add_prediction(prediction).await;
            }
        }
    }

    /// Get a specific signal for a symbol.
    pub async fn get_signal(
        &self,
        symbol: &str,
        indicator_id: &str,
        timeframe: TradingTimeframe,
    ) -> Option<SignalOutput> {
        let signals = self.get_signals(symbol, timeframe).await?;
        signals
            .signals
            .into_iter()
            .find(|s| s.name.to_lowercase().contains(&indicator_id.to_lowercase()))
    }

    /// Invalidate cache for a symbol (all timeframes).
    pub fn invalidate(&self, symbol: &str) {
        let symbol_lower = symbol.to_lowercase();
        self.cache.retain(|k, _| !k.starts_with(&symbol_lower));
    }

    /// Invalidate all cached signals.
    pub fn invalidate_all(&self) {
        self.cache.clear();
    }

    /// Get prediction store reference.
    pub fn prediction_store(&self) -> &Arc<PredictionStore> {
        &self.prediction_store
    }

    /// Get accuracy store reference.
    pub fn accuracy_store(&self) -> &Arc<AccuracyStore> {
        &self.accuracy_store
    }

    /// Get accuracy-weighted recommendation for a symbol.
    /// This weights signals by their historical accuracy to produce a more reliable
    /// buy/sell/hold recommendation.
    pub async fn get_recommendation(
        &self,
        symbol: &str,
        timeframe: TradingTimeframe,
    ) -> Option<Recommendation> {
        let signals = self.get_signals(symbol, timeframe).await?;

        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;
        let mut indicators_with_accuracy = 0u32;
        let mut total_accuracy = 0.0;

        for signal in &signals.signals {
            // Get accuracy weight - signals with proven accuracy get higher weight
            let (weight, has_accuracy) = if let (Some(accuracy), Some(sample_size)) =
                (signal.accuracy, signal.sample_size)
            {
                if sample_size >= 5 {
                    // Weight by accuracy squared (reward consistently accurate indicators)
                    let acc_weight = (accuracy / 100.0).powi(2);
                    // Also consider sample size (more samples = more reliable)
                    let sample_weight = (sample_size as f64 / 50.0).min(1.0);
                    (acc_weight * (0.5 + sample_weight * 0.5), true)
                } else {
                    (0.3, false) // Low weight for new indicators
                }
            } else {
                (0.3, false) // Low weight for no accuracy data
            };

            weighted_sum += signal.score as f64 * weight;
            total_weight += weight;

            if has_accuracy {
                indicators_with_accuracy += 1;
                total_accuracy += signal.accuracy.unwrap_or(0.0);
            }
        }

        let weighted_score = if total_weight > 0.0 {
            weighted_sum / total_weight
        } else {
            0.0
        };

        let average_accuracy = if indicators_with_accuracy > 0 {
            total_accuracy / indicators_with_accuracy as f64
        } else {
            50.0 // Default to 50% if no accuracy data
        };

        Some(Recommendation::from_score(
            symbol.to_uppercase(),
            weighted_score,
            indicators_with_accuracy,
            signals.signals.len() as u32,
            average_accuracy,
        ))
    }
}
