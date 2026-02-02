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
    /// Get the range from a string.
    pub fn from_str(s: &str) -> Option<Self> {
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
            ChartRange::OneHour => 60,      // 1-minute buckets
            ChartRange::FourHours => 60,    // 1-minute buckets
            ChartRange::OneDay => 300,      // 5-minute buckets
            ChartRange::OneWeek => 3600,    // 1-hour buckets
            ChartRange::OneMonth => 3600,   // 1-hour buckets
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
            ChartResolution::OneMinute => 14400,      // 4 hours (for short-term charts)
            ChartResolution::FiveMinute => 604800,    // 7 days (for daily charts)
            ChartResolution::OneHour => 7776000,      // 90 days (for long-term historical)
        }
    }
}
