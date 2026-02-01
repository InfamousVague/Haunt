use serde::{Deserialize, Serialize};

/// Global cryptocurrency market metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalMetrics {
    pub total_market_cap: f64,
    pub total_volume_24h: f64,
    pub btc_dominance: f64,
    pub eth_dominance: f64,
    pub active_cryptocurrencies: i32,
    pub active_exchanges: i32,
    pub market_cap_change_24h: f64,
    pub volume_change_24h: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defi_volume_24h: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defi_market_cap: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stablecoin_volume_24h: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stablecoin_market_cap: Option<f64>,
    pub last_updated: String,
}

/// Fear & Greed Index data.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FearGreedData {
    #[serde(default = "default_fear_greed_value")]
    pub value: i32,
    #[serde(default = "default_classification")]
    pub classification: String,
    #[serde(default)]
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_close: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_week: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_month: Option<i32>,
}

fn default_fear_greed_value() -> i32 {
    50
}

fn default_classification() -> String {
    "Neutral".to_string()
}

impl FearGreedData {
    /// Get the classification for a fear & greed value.
    pub fn classify(value: i32) -> &'static str {
        match value {
            0..=24 => "Extreme Fear",
            25..=44 => "Fear",
            45..=55 => "Neutral",
            56..=75 => "Greed",
            _ => "Extreme Greed",
        }
    }
}
