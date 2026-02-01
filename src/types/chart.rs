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
pub struct ChartData {
    pub symbol: String,
    pub range: String,
    pub data: Vec<OhlcPoint>,
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
    pub fn retention_seconds(&self) -> i64 {
        match self {
            ChartResolution::OneMinute => 3600,      // 1 hour
            ChartResolution::FiveMinute => 86400,    // 24 hours
            ChartResolution::OneHour => 2592000,     // 30 days
        }
    }
}
