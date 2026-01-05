use chrono::{DateTime, Duration, TimeZone, Utc};
use serde::{Deserialize, Serialize};

/// Common rollup resolutions.
///
/// Keep this minimal (open-source friendly).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    Minute,
    Hour,
    Day,
}

impl Resolution {
    pub fn duration(self) -> Duration {
        match self {
            Resolution::Minute => Duration::minutes(1),
            Resolution::Hour => Duration::hours(1),
            Resolution::Day => Duration::days(1),
        }
    }

    /// Bucket start timestamp for a given resolution.
    /// This is the canonical bucketing rule used by rollup and storage sharding.
    pub fn bucket_start(self, t: DateTime<Utc>) -> DateTime<Utc> {
        let ts = t.timestamp(); // seconds
        match self {
            Resolution::Minute => {
                let b = ts - (ts % 60);
                Utc.timestamp_opt(b, 0).single().expect("valid timestamp")
            }
            Resolution::Hour => {
                let b = ts - (ts % 3600);
                Utc.timestamp_opt(b, 0).single().expect("valid timestamp")
            }
            Resolution::Day => {
                // midnight UTC boundary
                let date = t.date_naive();
                Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).expect("valid hms"))
            }
        }
    }
}

/// Rollup direction: from `source` to `target`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollupWindow {
    pub source: Resolution,
    pub target: Resolution,
}

impl RollupWindow {
    pub fn new(source: Resolution, target: Resolution) -> Self {
        Self { source, target }
    }

    /// Validate rollup direction.
    /// We allow: Minute->Hour, Hour->Day, Minute->Day (optional shortcut).
    pub fn validate(self) -> Result<(), &'static str> {
        use Resolution::*;
        match (self.source, self.target) {
            (Minute, Hour) | (Hour, Day) | (Minute, Day) => Ok(()),
            (a, b) if a == b => Err("source and target resolutions are the same"),
            _ => Err("unsupported rollup direction"),
        }
    }

    /// Compute the target bucket start for a source timestamp.
    #[inline]
    pub fn target_bucket_start(self, source_ts: DateTime<Utc>) -> DateTime<Utc> {
        self.target.bucket_start(source_ts)
    }
}
