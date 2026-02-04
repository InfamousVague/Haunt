use serde::{Deserialize, Serialize};

/// Chart time range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChartRange {
    #[serde(rename = "1h")]
    OneHour,
    #[serde(rename = "4h")]
    FourHours,
    #[serde(rename = "1d")]
    OneDay,
    #[serde(rename = "1w")]
    OneWeek,
    #[serde(rename = "1m")]
    OneMonth,
}

impl ChartRange {
    /// Parse from a string.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "1h" => Some(ChartRange::OneHour),
            "4h" => Some(ChartRange::FourHours),
            "1d" => Some(ChartRange::OneDay),
            "1w" => Some(ChartRange::OneWeek),
            "1m" => Some(ChartRange::OneMonth),
            _ => None,
        }
    }

    /// Get the bucket size in seconds for this range.
    pub fn bucket_seconds(&self) -> i64 {
        match self {
            ChartRange::OneHour => 60,    // 1-minute buckets
            ChartRange::FourHours => 60,  // 1-minute buckets
            ChartRange::OneDay => 300,    // 5-minute buckets
            ChartRange::OneWeek => 3600,  // 1-hour buckets
            ChartRange::OneMonth => 3600, // 1-hour buckets
        }
    }

    /// Get the total duration in seconds for this range.
    pub fn duration_seconds(&self) -> i64 {
        match self {
            ChartRange::OneHour => 3600,
            ChartRange::FourHours => 14400,
            ChartRange::OneDay => 86400,
            ChartRange::OneWeek => 604800,
            ChartRange::OneMonth => 2592000,
        }
    }
}

/// OHLC (Open, High, Low, Close) data point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OhlcPoint {
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<f64>,
}

/// Chart data response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartData {
    pub symbol: String,
    pub range: String,
    pub data: Vec<OhlcPoint>,
    /// Whether historical data is currently being seeded for this chart.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seeding: Option<bool>,
    /// Detailed seeding status: "not_started", "in_progress", "complete", "failed"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seeding_status: Option<String>,
    /// Seeding progress percentage (0-100)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seeding_progress: Option<u8>,
    /// Data completeness for the requested range (0-100)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_completeness: Option<u8>,
    /// Expected number of points for the requested range
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_points: Option<u32>,
}

/// Chart resolution for internal storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChartResolution {
    OneMinute,
    FiveMinute,
    OneHour,
}

impl ChartResolution {
    /// Get the bucket size in seconds.
    pub fn seconds(&self) -> i64 {
        match self {
            ChartResolution::OneMinute => 60,
            ChartResolution::FiveMinute => 300,
            ChartResolution::OneHour => 3600,
        }
    }

    /// Get the retention duration in seconds.
    /// Extended to support historical data seeding.
    pub fn retention_seconds(&self) -> i64 {
        match self {
            ChartResolution::OneMinute => 14400, // 4 hours (for short-term charts)
            ChartResolution::FiveMinute => 604800, // 7 days (for daily charts)
            ChartResolution::OneHour => 7776000, // 90 days (for long-term historical)
        }
    }
}

/// Chart candle for backtesting and storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartCandle {
    /// Timestamp in milliseconds
    pub timestamp: i64,
    /// Opening price
    pub open: f64,
    /// Highest price
    pub high: f64,
    /// Lowest price
    pub low: f64,
    /// Closing price
    pub close: f64,
    /// Trading volume
    pub volume: f64,
}

impl ChartCandle {
    /// Create a new candle.
    pub fn new(timestamp: i64, open: f64, high: f64, low: f64, close: f64, volume: f64) -> Self {
        Self { timestamp, open, high, low, close, volume }
    }

    /// Convert from OhlcPoint.
    pub fn from_ohlc(point: &OhlcPoint) -> Self {
        Self {
            timestamp: point.time,
            open: point.open,
            high: point.high,
            low: point.low,
            close: point.close,
            volume: point.volume.unwrap_or(0.0),
        }
    }

    /// Convert to OhlcPoint.
    pub fn to_ohlc(&self) -> OhlcPoint {
        OhlcPoint {
            time: self.timestamp,
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            volume: Some(self.volume),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // ChartRange Tests
    // =========================================================================

    #[test]
    fn test_chart_range_from_str() {
        assert_eq!(ChartRange::parse("1h"), Some(ChartRange::OneHour));
        assert_eq!(ChartRange::parse("4h"), Some(ChartRange::FourHours));
        assert_eq!(ChartRange::parse("1d"), Some(ChartRange::OneDay));
        assert_eq!(ChartRange::parse("1w"), Some(ChartRange::OneWeek));
        assert_eq!(ChartRange::parse("1m"), Some(ChartRange::OneMonth));
        assert_eq!(ChartRange::parse("invalid"), None);
    }

    #[test]
    fn test_chart_range_bucket_seconds() {
        assert_eq!(ChartRange::OneHour.bucket_seconds(), 60);
        assert_eq!(ChartRange::FourHours.bucket_seconds(), 60);
        assert_eq!(ChartRange::OneDay.bucket_seconds(), 300);
        assert_eq!(ChartRange::OneWeek.bucket_seconds(), 3600);
        assert_eq!(ChartRange::OneMonth.bucket_seconds(), 3600);
    }

    #[test]
    fn test_chart_range_duration_seconds() {
        assert_eq!(ChartRange::OneHour.duration_seconds(), 3600);
        assert_eq!(ChartRange::FourHours.duration_seconds(), 14400);
        assert_eq!(ChartRange::OneDay.duration_seconds(), 86400);
        assert_eq!(ChartRange::OneWeek.duration_seconds(), 604800);
        assert_eq!(ChartRange::OneMonth.duration_seconds(), 2592000);
    }

    #[test]
    fn test_chart_range_serialization() {
        let range = ChartRange::OneHour;
        let json = serde_json::to_string(&range).unwrap();
        assert_eq!(json, "\"1h\"");

        let parsed: ChartRange = serde_json::from_str("\"1w\"").unwrap();
        assert_eq!(parsed, ChartRange::OneWeek);
    }

    // =========================================================================
    // OhlcPoint Tests
    // =========================================================================

    #[test]
    fn test_ohlc_point_creation() {
        let point = OhlcPoint {
            time: 1704067200000,
            open: 50000.0,
            high: 50500.0,
            low: 49500.0,
            close: 50200.0,
            volume: Some(1000.0),
        };

        assert_eq!(point.time, 1704067200000);
        assert_eq!(point.open, 50000.0);
        assert_eq!(point.high, 50500.0);
        assert_eq!(point.low, 49500.0);
        assert_eq!(point.close, 50200.0);
    }

    #[test]
    fn test_ohlc_point_without_volume() {
        let point = OhlcPoint {
            time: 1704067200000,
            open: 100.0,
            high: 105.0,
            low: 98.0,
            close: 103.0,
            volume: None,
        };

        assert!(point.volume.is_none());
    }

    #[test]
    fn test_ohlc_point_serialization() {
        let point = OhlcPoint {
            time: 1704067200000,
            open: 100.0,
            high: 105.0,
            low: 98.0,
            close: 103.0,
            volume: Some(500.0),
        };

        let json = serde_json::to_string(&point).unwrap();
        assert!(json.contains("\"time\":1704067200000"));
        assert!(json.contains("\"open\":100"));
        assert!(json.contains("\"volume\":500"));
    }

    #[test]
    fn test_ohlc_point_serialization_skips_none_volume() {
        let point = OhlcPoint {
            time: 1704067200000,
            open: 100.0,
            high: 105.0,
            low: 98.0,
            close: 103.0,
            volume: None,
        };

        let json = serde_json::to_string(&point).unwrap();
        assert!(!json.contains("volume"));
    }

    // =========================================================================
    // ChartData Tests
    // =========================================================================

    #[test]
    fn test_chart_data_creation() {
        let data = ChartData {
            symbol: "BTC".to_string(),
            range: "1h".to_string(),
            data: vec![OhlcPoint {
                time: 1704067200000,
                open: 50000.0,
                high: 50500.0,
                low: 49500.0,
                close: 50200.0,
                volume: Some(1000.0),
            }],
            seeding: None,
            seeding_status: None,
            seeding_progress: None,
            data_completeness: None,
            expected_points: None,
        };

        assert_eq!(data.symbol, "BTC");
        assert_eq!(data.range, "1h");
        assert_eq!(data.data.len(), 1);
    }

    #[test]
    fn test_chart_data_with_seeding_info() {
        let data = ChartData {
            symbol: "ETH".to_string(),
            range: "1d".to_string(),
            data: vec![],
            seeding: Some(true),
            seeding_status: Some("in_progress".to_string()),
            seeding_progress: Some(50),
            data_completeness: Some(25),
            expected_points: Some(288),
        };

        assert_eq!(data.seeding, Some(true));
        assert_eq!(data.seeding_status, Some("in_progress".to_string()));
        assert_eq!(data.seeding_progress, Some(50));
    }

    #[test]
    fn test_chart_data_serialization() {
        let data = ChartData {
            symbol: "BTC".to_string(),
            range: "1h".to_string(),
            data: vec![],
            seeding: Some(false),
            seeding_status: Some("complete".to_string()),
            seeding_progress: Some(100),
            data_completeness: Some(100),
            expected_points: Some(60),
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"symbol\":\"BTC\""));
        assert!(json.contains("\"seedingStatus\":\"complete\""));
        assert!(json.contains("\"dataCompleteness\":100"));
    }

    #[test]
    fn test_chart_data_serialization_skips_none() {
        let data = ChartData {
            symbol: "BTC".to_string(),
            range: "1h".to_string(),
            data: vec![],
            seeding: None,
            seeding_status: None,
            seeding_progress: None,
            data_completeness: None,
            expected_points: None,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(!json.contains("seeding"));
        assert!(!json.contains("seedingStatus"));
        assert!(!json.contains("dataCompleteness"));
    }

    // =========================================================================
    // ChartResolution Tests
    // =========================================================================

    #[test]
    fn test_chart_resolution_seconds() {
        assert_eq!(ChartResolution::OneMinute.seconds(), 60);
        assert_eq!(ChartResolution::FiveMinute.seconds(), 300);
        assert_eq!(ChartResolution::OneHour.seconds(), 3600);
    }

    #[test]
    fn test_chart_resolution_retention_seconds() {
        assert_eq!(ChartResolution::OneMinute.retention_seconds(), 14400); // 4 hours
        assert_eq!(ChartResolution::FiveMinute.retention_seconds(), 604800); // 7 days
        assert_eq!(ChartResolution::OneHour.retention_seconds(), 7776000); // 90 days
    }

    #[test]
    fn test_chart_resolution_equality() {
        assert_eq!(ChartResolution::OneMinute, ChartResolution::OneMinute);
        assert_ne!(ChartResolution::OneMinute, ChartResolution::FiveMinute);
    }
}
