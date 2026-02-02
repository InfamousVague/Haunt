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

/// Time frame for top movers calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MoverTimeframe {
    #[serde(rename = "1m")]
    OneMinute,
    #[serde(rename = "5m")]
    FiveMinutes,
    #[serde(rename = "15m")]
    FifteenMinutes,
    #[serde(rename = "1h")]
    OneHour,
    #[serde(rename = "4h")]
    FourHours,
    #[serde(rename = "24h")]
    TwentyFourHours,
}

impl MoverTimeframe {
    /// Get the number of seconds for this timeframe.
    pub fn seconds(&self) -> i64 {
        match self {
            Self::OneMinute => 60,
            Self::FiveMinutes => 300,
            Self::FifteenMinutes => 900,
            Self::OneHour => 3600,
            Self::FourHours => 14400,
            Self::TwentyFourHours => 86400,
        }
    }
}

impl Default for MoverTimeframe {
    fn default() -> Self {
        Self::OneHour
    }
}

impl std::fmt::Display for MoverTimeframe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OneMinute => write!(f, "1m"),
            Self::FiveMinutes => write!(f, "5m"),
            Self::FifteenMinutes => write!(f, "15m"),
            Self::OneHour => write!(f, "1h"),
            Self::FourHours => write!(f, "4h"),
            Self::TwentyFourHours => write!(f, "24h"),
        }
    }
}

impl std::str::FromStr for MoverTimeframe {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "1m" => Ok(Self::OneMinute),
            "5m" => Ok(Self::FiveMinutes),
            "15m" => Ok(Self::FifteenMinutes),
            "1h" => Ok(Self::OneHour),
            "4h" => Ok(Self::FourHours),
            "24h" => Ok(Self::TwentyFourHours),
            _ => Err(format!("Unknown timeframe: {}", s)),
        }
    }
}

/// A single mover entry (gainer or loser).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mover {
    pub symbol: String,
    pub price: f64,
    pub change_percent: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_24h: Option<f64>,
}

/// Response for top movers endpoint.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoversResponse {
    pub timeframe: String,
    pub gainers: Vec<Mover>,
    pub losers: Vec<Mover>,
    pub timestamp: i64,
}
