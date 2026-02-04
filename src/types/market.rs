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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MoverTimeframe {
    #[serde(rename = "1m")]
    OneMinute,
    #[serde(rename = "5m")]
    FiveMinutes,
    #[serde(rename = "15m")]
    FifteenMinutes,
    #[serde(rename = "1h")]
    #[default]
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

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // GlobalMetrics Tests
    // =========================================================================

    #[test]
    fn test_global_metrics_creation() {
        let metrics = GlobalMetrics {
            total_market_cap: 2_500_000_000_000.0,
            total_volume_24h: 100_000_000_000.0,
            btc_dominance: 52.5,
            eth_dominance: 17.3,
            active_cryptocurrencies: 10000,
            active_exchanges: 500,
            market_cap_change_24h: 2.5,
            volume_change_24h: -1.2,
            defi_volume_24h: Some(5_000_000_000.0),
            defi_market_cap: Some(80_000_000_000.0),
            stablecoin_volume_24h: Some(50_000_000_000.0),
            stablecoin_market_cap: Some(150_000_000_000.0),
            last_updated: "2024-01-01T00:00:00Z".to_string(),
        };

        assert_eq!(metrics.btc_dominance, 52.5);
        assert_eq!(metrics.active_cryptocurrencies, 10000);
    }

    #[test]
    fn test_global_metrics_serialization() {
        let metrics = GlobalMetrics {
            total_market_cap: 1_000_000_000_000.0,
            total_volume_24h: 50_000_000_000.0,
            btc_dominance: 50.0,
            eth_dominance: 18.0,
            active_cryptocurrencies: 5000,
            active_exchanges: 300,
            market_cap_change_24h: 1.0,
            volume_change_24h: 2.0,
            defi_volume_24h: None,
            defi_market_cap: None,
            stablecoin_volume_24h: None,
            stablecoin_market_cap: None,
            last_updated: "2024-01-01T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&metrics).unwrap();
        assert!(json.contains("\"totalMarketCap\":"));
        assert!(json.contains("\"btcDominance\":50"));
        // Optional None fields should not appear
        assert!(!json.contains("defiVolume24h"));
    }

    #[test]
    fn test_global_metrics_deserialization() {
        let json = r#"{
            "totalMarketCap": 1000000000000,
            "totalVolume24h": 50000000000,
            "btcDominance": 50.0,
            "ethDominance": 18.0,
            "activeCryptocurrencies": 5000,
            "activeExchanges": 300,
            "marketCapChange24h": 1.0,
            "volumeChange24h": 2.0,
            "lastUpdated": "2024-01-01T00:00:00Z"
        }"#;

        let metrics: GlobalMetrics = serde_json::from_str(json).unwrap();
        assert_eq!(metrics.btc_dominance, 50.0);
        assert_eq!(metrics.active_cryptocurrencies, 5000);
    }

    // =========================================================================
    // FearGreedData Tests
    // =========================================================================

    #[test]
    fn test_fear_greed_data_default() {
        let data = FearGreedData::default();
        // Default trait uses Rust defaults (0, ""), serde defaults are only for deserialization
        assert_eq!(data.value, 0);
        assert_eq!(data.classification, "");
    }

    #[test]
    fn test_fear_greed_data_serde_default() {
        // serde defaults are used when deserializing missing fields
        let json = r#"{"timestamp": "2024-01-01"}"#;
        let data: FearGreedData = serde_json::from_str(json).unwrap();
        assert_eq!(data.value, 50);
        assert_eq!(data.classification, "Neutral");
    }

    #[test]
    fn test_fear_greed_classify_extreme_fear() {
        assert_eq!(FearGreedData::classify(0), "Extreme Fear");
        assert_eq!(FearGreedData::classify(10), "Extreme Fear");
        assert_eq!(FearGreedData::classify(24), "Extreme Fear");
    }

    #[test]
    fn test_fear_greed_classify_fear() {
        assert_eq!(FearGreedData::classify(25), "Fear");
        assert_eq!(FearGreedData::classify(35), "Fear");
        assert_eq!(FearGreedData::classify(44), "Fear");
    }

    #[test]
    fn test_fear_greed_classify_neutral() {
        assert_eq!(FearGreedData::classify(45), "Neutral");
        assert_eq!(FearGreedData::classify(50), "Neutral");
        assert_eq!(FearGreedData::classify(55), "Neutral");
    }

    #[test]
    fn test_fear_greed_classify_greed() {
        assert_eq!(FearGreedData::classify(56), "Greed");
        assert_eq!(FearGreedData::classify(65), "Greed");
        assert_eq!(FearGreedData::classify(75), "Greed");
    }

    #[test]
    fn test_fear_greed_classify_extreme_greed() {
        assert_eq!(FearGreedData::classify(76), "Extreme Greed");
        assert_eq!(FearGreedData::classify(90), "Extreme Greed");
        assert_eq!(FearGreedData::classify(100), "Extreme Greed");
    }

    #[test]
    fn test_fear_greed_data_with_history() {
        let data = FearGreedData {
            value: 75,
            classification: "Greed".to_string(),
            timestamp: "2024-01-01".to_string(),
            previous_close: Some(70),
            previous_week: Some(60),
            previous_month: Some(45),
        };

        assert_eq!(data.value, 75);
        assert_eq!(data.previous_close, Some(70));
        assert_eq!(data.previous_week, Some(60));
        assert_eq!(data.previous_month, Some(45));
    }

    // =========================================================================
    // MoverTimeframe Tests
    // =========================================================================

    #[test]
    fn test_mover_timeframe_seconds() {
        assert_eq!(MoverTimeframe::OneMinute.seconds(), 60);
        assert_eq!(MoverTimeframe::FiveMinutes.seconds(), 300);
        assert_eq!(MoverTimeframe::FifteenMinutes.seconds(), 900);
        assert_eq!(MoverTimeframe::OneHour.seconds(), 3600);
        assert_eq!(MoverTimeframe::FourHours.seconds(), 14400);
        assert_eq!(MoverTimeframe::TwentyFourHours.seconds(), 86400);
    }

    #[test]
    fn test_mover_timeframe_default() {
        let default = MoverTimeframe::default();
        assert_eq!(default, MoverTimeframe::OneHour);
    }

    #[test]
    fn test_mover_timeframe_display() {
        assert_eq!(format!("{}", MoverTimeframe::OneMinute), "1m");
        assert_eq!(format!("{}", MoverTimeframe::FiveMinutes), "5m");
        assert_eq!(format!("{}", MoverTimeframe::FifteenMinutes), "15m");
        assert_eq!(format!("{}", MoverTimeframe::OneHour), "1h");
        assert_eq!(format!("{}", MoverTimeframe::FourHours), "4h");
        assert_eq!(format!("{}", MoverTimeframe::TwentyFourHours), "24h");
    }

    #[test]
    fn test_mover_timeframe_from_str() {
        assert_eq!(
            "1m".parse::<MoverTimeframe>().unwrap(),
            MoverTimeframe::OneMinute
        );
        assert_eq!(
            "5m".parse::<MoverTimeframe>().unwrap(),
            MoverTimeframe::FiveMinutes
        );
        assert_eq!(
            "15m".parse::<MoverTimeframe>().unwrap(),
            MoverTimeframe::FifteenMinutes
        );
        assert_eq!(
            "1h".parse::<MoverTimeframe>().unwrap(),
            MoverTimeframe::OneHour
        );
        assert_eq!(
            "4h".parse::<MoverTimeframe>().unwrap(),
            MoverTimeframe::FourHours
        );
        assert_eq!(
            "24h".parse::<MoverTimeframe>().unwrap(),
            MoverTimeframe::TwentyFourHours
        );
    }

    #[test]
    fn test_mover_timeframe_from_str_case_insensitive() {
        assert_eq!(
            "1M".parse::<MoverTimeframe>().unwrap(),
            MoverTimeframe::OneMinute
        );
        assert_eq!(
            "1H".parse::<MoverTimeframe>().unwrap(),
            MoverTimeframe::OneHour
        );
        assert_eq!(
            "24H".parse::<MoverTimeframe>().unwrap(),
            MoverTimeframe::TwentyFourHours
        );
    }

    #[test]
    fn test_mover_timeframe_from_str_error() {
        let result = "invalid".parse::<MoverTimeframe>();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown timeframe"));
    }

    #[test]
    fn test_mover_timeframe_serialization() {
        let tf = MoverTimeframe::OneHour;
        let json = serde_json::to_string(&tf).unwrap();
        assert_eq!(json, "\"1h\"");

        let parsed: MoverTimeframe = serde_json::from_str("\"4h\"").unwrap();
        assert_eq!(parsed, MoverTimeframe::FourHours);
    }

    // =========================================================================
    // Mover Tests
    // =========================================================================

    #[test]
    fn test_mover_creation() {
        let mover = Mover {
            symbol: "BTC".to_string(),
            price: 50000.0,
            change_percent: 5.5,
            volume_24h: Some(1_000_000_000.0),
        };

        assert_eq!(mover.symbol, "BTC");
        assert_eq!(mover.price, 50000.0);
        assert_eq!(mover.change_percent, 5.5);
    }

    #[test]
    fn test_mover_serialization() {
        let mover = Mover {
            symbol: "ETH".to_string(),
            price: 3000.0,
            change_percent: -2.5,
            volume_24h: None,
        };

        let json = serde_json::to_string(&mover).unwrap();
        assert!(json.contains("\"symbol\":\"ETH\""));
        assert!(json.contains("\"changePercent\":-2.5"));
        assert!(!json.contains("volume24h")); // None should be omitted
    }

    // =========================================================================
    // MoversResponse Tests
    // =========================================================================

    #[test]
    fn test_movers_response_creation() {
        let response = MoversResponse {
            timeframe: "1h".to_string(),
            gainers: vec![Mover {
                symbol: "BTC".to_string(),
                price: 50000.0,
                change_percent: 5.0,
                volume_24h: Some(1_000_000_000.0),
            }],
            losers: vec![Mover {
                symbol: "ETH".to_string(),
                price: 3000.0,
                change_percent: -3.0,
                volume_24h: Some(500_000_000.0),
            }],
            timestamp: 1704067200000,
        };

        assert_eq!(response.timeframe, "1h");
        assert_eq!(response.gainers.len(), 1);
        assert_eq!(response.losers.len(), 1);
        assert_eq!(response.gainers[0].symbol, "BTC");
        assert_eq!(response.losers[0].symbol, "ETH");
    }

    #[test]
    fn test_movers_response_serialization() {
        let response = MoversResponse {
            timeframe: "24h".to_string(),
            gainers: vec![],
            losers: vec![],
            timestamp: 1704067200000,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"timeframe\":\"24h\""));
        assert!(json.contains("\"gainers\":[]"));
        assert!(json.contains("\"losers\":[]"));
    }
}
