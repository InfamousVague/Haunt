use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Trading timeframe/style for signal calculations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TradingTimeframe {
    /// Very short term: minutes to hours. Focus on momentum indicators.
    Scalping,
    /// Intraday: 1 hour to 1 day. Balanced indicator mix.
    #[default]
    DayTrading,
    /// Days to weeks. Focus on trend and volume indicators.
    SwingTrading,
    /// Weeks to months. Focus on long-term trends.
    PositionTrading,
}

impl TradingTimeframe {
    /// Parse from string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "scalping" | "scalp" => Some(Self::Scalping),
            "day_trading" | "day" | "intraday" => Some(Self::DayTrading),
            "swing_trading" | "swing" => Some(Self::SwingTrading),
            "position_trading" | "position" | "long_term" => Some(Self::PositionTrading),
            _ => None,
        }
    }

    /// Get display name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Scalping => "Scalping",
            Self::DayTrading => "Day Trading",
            Self::SwingTrading => "Swing Trading",
            Self::PositionTrading => "Position Trading",
        }
    }

    /// Get the chart range to use for indicator calculations.
    pub fn chart_range(&self) -> crate::types::ChartRange {
        match self {
            Self::Scalping => crate::types::ChartRange::FourHours,
            Self::DayTrading => crate::types::ChartRange::OneDay,
            Self::SwingTrading => crate::types::ChartRange::OneWeek,
            Self::PositionTrading => crate::types::ChartRange::OneMonth,
        }
    }

    /// Get weights for each signal category based on timeframe.
    /// Returns (trend_weight, momentum_weight, volatility_weight, volume_weight).
    pub fn category_weights(&self) -> (f64, f64, f64, f64) {
        match self {
            // Scalping: Heavy momentum focus
            Self::Scalping => (0.20, 0.50, 0.20, 0.10),
            // Day trading: Balanced
            Self::DayTrading => (0.35, 0.35, 0.15, 0.15),
            // Swing: Trend and volume focused
            Self::SwingTrading => (0.40, 0.25, 0.10, 0.25),
            // Position: Heavy trend focus
            Self::PositionTrading => (0.50, 0.20, 0.10, 0.20),
        }
    }

    /// Get prediction validation timeframes in milliseconds.
    /// Returns (short, medium, long) timeframes.
    pub fn validation_timeframes(&self) -> (i64, i64, i64) {
        match self {
            // Scalping: 15min, 1h, 4h
            Self::Scalping => (900_000, 3_600_000, 14_400_000),
            // Day trading: 1h, 4h, 24h
            Self::DayTrading => (3_600_000, 14_400_000, 86_400_000),
            // Swing: 4h, 1d, 1w
            Self::SwingTrading => (14_400_000, 86_400_000, 604_800_000),
            // Position: 1d, 1w, 1m
            Self::PositionTrading => (86_400_000, 604_800_000, 2_592_000_000),
        }
    }
}

/// Direction of a trading signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalDirection {
    StrongBuy,
    Buy,
    Neutral,
    Sell,
    StrongSell,
}

impl SignalDirection {
    /// Create direction from a score (-100 to +100).
    pub fn from_score(score: i8) -> Self {
        match score {
            s if s >= 60 => SignalDirection::StrongBuy,
            s if s >= 20 => SignalDirection::Buy,
            s if s > -20 => SignalDirection::Neutral,
            s if s > -60 => SignalDirection::Sell,
            _ => SignalDirection::StrongSell,
        }
    }

    /// Get display label for this direction.
    pub fn label(&self) -> &'static str {
        match self {
            SignalDirection::StrongBuy => "Strong Buy",
            SignalDirection::Buy => "Buy",
            SignalDirection::Neutral => "Neutral",
            SignalDirection::Sell => "Sell",
            SignalDirection::StrongSell => "Strong Sell",
        }
    }
}

/// Category of a trading signal indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalCategory {
    Trend,
    Momentum,
    Volatility,
    Volume,
}

impl SignalCategory {
    /// Get display name for this category.
    pub fn name(&self) -> &'static str {
        match self {
            SignalCategory::Trend => "Trend",
            SignalCategory::Momentum => "Momentum",
            SignalCategory::Volatility => "Volatility",
            SignalCategory::Volume => "Volume",
        }
    }
}

/// Output from a single signal indicator calculation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalOutput {
    /// Indicator name (e.g., "RSI", "MACD").
    pub name: String,
    /// Category of this indicator.
    pub category: SignalCategory,
    /// Raw indicator value.
    pub value: f64,
    /// Normalized score from -100 (strong sell) to +100 (strong buy).
    pub score: i8,
    /// Signal direction derived from score.
    pub direction: SignalDirection,
    /// Historical accuracy percentage (0-100), if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accuracy: Option<f64>,
    /// Number of predictions used for accuracy calculation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_size: Option<u32>,
    /// Unix timestamp (milliseconds) when calculated.
    pub timestamp: i64,
}

/// Aggregated signals for a symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolSignals {
    /// Symbol this data is for.
    pub symbol: String,
    /// Trading timeframe used for calculations.
    pub timeframe: TradingTimeframe,
    /// All individual indicator signals.
    pub signals: Vec<SignalOutput>,
    /// Trend category composite score (-100 to +100).
    pub trend_score: i8,
    /// Momentum category composite score (-100 to +100).
    pub momentum_score: i8,
    /// Volatility category composite score (-100 to +100).
    pub volatility_score: i8,
    /// Volume category composite score (-100 to +100).
    pub volume_score: i8,
    /// Overall composite score (-100 to +100).
    /// Weighted based on trading timeframe.
    pub composite_score: i8,
    /// Overall signal direction.
    pub direction: SignalDirection,
    /// Unix timestamp (milliseconds) when calculated.
    pub timestamp: i64,
}

/// Outcome of a validated prediction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PredictionOutcome {
    /// Price moved in predicted direction by threshold amount.
    Correct,
    /// Price moved opposite to prediction by threshold amount.
    Incorrect,
    /// Price didn't move significantly (within threshold).
    Neutral,
}

/// A recorded signal prediction for accuracy tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalPrediction {
    /// Unique prediction ID.
    pub id: Uuid,
    /// Symbol this prediction is for.
    pub symbol: String,
    /// Indicator that made the prediction.
    pub indicator: String,
    /// Predicted direction.
    pub direction: SignalDirection,
    /// Signal score at time of prediction (-100 to +100).
    pub score: i8,
    /// Price when prediction was made.
    pub price_at_prediction: f64,
    /// Unix timestamp (milliseconds) when prediction was made.
    pub timestamp: i64,
    /// Whether this prediction has been validated.
    pub validated: bool,
    /// Price 5 minutes after prediction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_after_5m: Option<f64>,
    /// Price 1 hour after prediction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_after_1h: Option<f64>,
    /// Price 4 hours after prediction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_after_4h: Option<f64>,
    /// Price 24 hours after prediction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_after_24h: Option<f64>,
    /// Outcome after 5 minutes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome_5m: Option<PredictionOutcome>,
    /// Outcome after 1 hour.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome_1h: Option<PredictionOutcome>,
    /// Outcome after 4 hours.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome_4h: Option<PredictionOutcome>,
    /// Outcome after 24 hours.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome_24h: Option<PredictionOutcome>,
}

impl SignalPrediction {
    /// Create a new prediction.
    pub fn new(
        symbol: String,
        indicator: String,
        direction: SignalDirection,
        score: i8,
        price: f64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            symbol,
            indicator,
            direction,
            score,
            price_at_prediction: price,
            timestamp: chrono::Utc::now().timestamp_millis(),
            validated: false,
            price_after_5m: None,
            price_after_1h: None,
            price_after_4h: None,
            price_after_24h: None,
            outcome_5m: None,
            outcome_1h: None,
            outcome_4h: None,
            outcome_24h: None,
        }
    }

    /// Check if this prediction has been validated for a specific timeframe.
    pub fn is_validated_for(&self, timeframe: &str) -> bool {
        match timeframe {
            "5m" => self.outcome_5m.is_some(),
            "1h" => self.outcome_1h.is_some(),
            "4h" => self.outcome_4h.is_some(),
            "24h" => self.outcome_24h.is_some(),
            _ => false,
        }
    }

    /// Validate this prediction against a current price for a given timeframe.
    pub fn validate(&mut self, current_price: f64, timeframe: &str) -> PredictionOutcome {
        let price_change_pct =
            ((current_price - self.price_at_prediction) / self.price_at_prediction) * 100.0;

        // Thresholds: price must move this much % in the predicted direction
        let threshold = match timeframe {
            "5m" => 0.1,  // 0.1% for 5 minute validation
            "1h" => 0.5,  // 0.5% for 1 hour
            "4h" => 1.0,  // 1% for 4 hours
            "24h" => 2.0, // 2% for 24 hours
            _ => 1.0,
        };

        let predicted_up = matches!(
            self.direction,
            SignalDirection::Buy | SignalDirection::StrongBuy
        );
        let actually_up = price_change_pct > threshold;
        let actually_down = price_change_pct < -threshold;

        let outcome = if predicted_up && actually_up || !predicted_up && actually_down {
            PredictionOutcome::Correct
        } else if price_change_pct.abs() < threshold {
            PredictionOutcome::Neutral
        } else {
            PredictionOutcome::Incorrect
        };

        match timeframe {
            "5m" => {
                self.price_after_5m = Some(current_price);
                self.outcome_5m = Some(outcome);
            }
            "1h" => {
                self.price_after_1h = Some(current_price);
                self.outcome_1h = Some(outcome);
            }
            "4h" => {
                self.price_after_4h = Some(current_price);
                self.outcome_4h = Some(outcome);
            }
            "24h" => {
                self.price_after_24h = Some(current_price);
                self.outcome_24h = Some(outcome);
                self.validated = true; // Fully validated after 24h
            }
            _ => {}
        }

        outcome
    }
}

/// Accuracy statistics for a signal indicator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalAccuracy {
    /// Indicator name.
    pub indicator: String,
    /// Symbol (or "global" for cross-symbol stats).
    pub symbol: String,
    /// Timeframe for this accuracy ("1h", "4h", "24h").
    pub timeframe: String,
    /// Total number of predictions made.
    pub total_predictions: u32,
    /// Number of correct predictions.
    pub correct_predictions: u32,
    /// Number of incorrect predictions.
    pub incorrect_predictions: u32,
    /// Number of neutral outcomes (price didn't move enough).
    pub neutral_predictions: u32,
    /// Accuracy percentage: correct / (correct + incorrect) * 100.
    pub accuracy_pct: f64,
    /// Unix timestamp (milliseconds) when last updated.
    pub last_updated: i64,
}

impl SignalAccuracy {
    /// Create a new accuracy tracker.
    pub fn new(indicator: String, symbol: String, timeframe: String) -> Self {
        Self {
            indicator,
            symbol,
            timeframe,
            total_predictions: 0,
            correct_predictions: 0,
            incorrect_predictions: 0,
            neutral_predictions: 0,
            accuracy_pct: 0.0,
            last_updated: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// Record a prediction outcome.
    pub fn record_outcome(&mut self, outcome: PredictionOutcome) {
        self.total_predictions += 1;
        match outcome {
            PredictionOutcome::Correct => self.correct_predictions += 1,
            PredictionOutcome::Incorrect => self.incorrect_predictions += 1,
            PredictionOutcome::Neutral => self.neutral_predictions += 1,
        }
        self.recalculate_accuracy();
        self.last_updated = chrono::Utc::now().timestamp_millis();
    }

    /// Recalculate accuracy percentage.
    fn recalculate_accuracy(&mut self) {
        let decisive = self.correct_predictions + self.incorrect_predictions;
        if decisive > 0 {
            self.accuracy_pct = (self.correct_predictions as f64 / decisive as f64) * 100.0;
        } else {
            self.accuracy_pct = 0.0;
        }
    }
}

/// Response for accuracy endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccuracyResponse {
    pub symbol: String,
    pub accuracies: Vec<SignalAccuracy>,
    pub timestamp: i64,
}

/// Response for predictions endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PredictionsResponse {
    pub symbol: String,
    pub predictions: Vec<SignalPrediction>,
    pub timestamp: i64,
}

/// Simple recommendation: Buy, Sell, or Hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationAction {
    Buy,
    Sell,
    Hold,
}

impl RecommendationAction {
    /// Get display label.
    pub fn label(&self) -> &'static str {
        match self {
            RecommendationAction::Buy => "Buy",
            RecommendationAction::Sell => "Sell",
            RecommendationAction::Hold => "Hold",
        }
    }
}

/// Accuracy-weighted trading recommendation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    /// Symbol this recommendation is for.
    pub symbol: String,
    /// The action: Buy, Sell, or Hold.
    pub action: RecommendationAction,
    /// Confidence level (0-100). Higher = more confident.
    pub confidence: f64,
    /// Weighted score that produced this recommendation (-100 to +100).
    pub weighted_score: f64,
    /// Number of indicators with accuracy data used.
    pub indicators_with_accuracy: u32,
    /// Total number of indicators considered.
    pub total_indicators: u32,
    /// Average accuracy of indicators used.
    pub average_accuracy: f64,
    /// Description explaining the recommendation.
    pub description: String,
    /// Unix timestamp (milliseconds) when computed.
    pub timestamp: i64,
}

impl Recommendation {
    /// Create a new recommendation from weighted score.
    pub fn from_score(
        symbol: String,
        weighted_score: f64,
        indicators_with_accuracy: u32,
        total_indicators: u32,
        average_accuracy: f64,
    ) -> Self {
        let (action, confidence, description) = if weighted_score >= 30.0 {
            let conf = ((weighted_score / 100.0) * average_accuracy).min(100.0);
            (
                RecommendationAction::Buy,
                conf,
                format!(
                    "Strong buy signal based on {} indicators with {:.0}% average accuracy",
                    indicators_with_accuracy, average_accuracy
                ),
            )
        } else if weighted_score >= 10.0 {
            let conf = ((weighted_score / 100.0) * average_accuracy * 0.8).min(100.0);
            (
                RecommendationAction::Buy,
                conf,
                format!(
                    "Moderate buy signal - indicators lean bullish with {:.0}% confidence",
                    conf
                ),
            )
        } else if weighted_score <= -30.0 {
            let conf = ((weighted_score.abs() / 100.0) * average_accuracy).min(100.0);
            (
                RecommendationAction::Sell,
                conf,
                format!(
                    "Strong sell signal based on {} indicators with {:.0}% average accuracy",
                    indicators_with_accuracy, average_accuracy
                ),
            )
        } else if weighted_score <= -10.0 {
            let conf = ((weighted_score.abs() / 100.0) * average_accuracy * 0.8).min(100.0);
            (
                RecommendationAction::Sell,
                conf,
                format!(
                    "Moderate sell signal - indicators lean bearish with {:.0}% confidence",
                    conf
                ),
            )
        } else {
            (
                RecommendationAction::Hold,
                50.0 + (10.0 - weighted_score.abs()) * 2.0,
                "No clear signal - indicators are mixed or neutral".to_string(),
            )
        };

        Self {
            symbol,
            action,
            confidence,
            weighted_score,
            indicators_with_accuracy,
            total_indicators,
            average_accuracy,
            description,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}
