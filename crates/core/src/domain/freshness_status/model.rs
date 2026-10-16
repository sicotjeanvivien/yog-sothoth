//! Ingestion freshness: the age of the last indexed event as Live / Delayed / Stale.

use chrono::{DateTime, Duration, Utc};

/// Below this age, ingestion is "Live".
const LIVE_THRESHOLD: Duration = Duration::minutes(2);

/// Below this age, "Delayed"; above it, "Stale".
const STALE_THRESHOLD: Duration = Duration::minutes(15);

/// Freshness verdict for the indexed data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreshnessStatus {
    /// Last event is recent — ingestion is keeping up.
    Live,
    /// Last event is older than expected — ingestion may be lagging.
    Delayed,
    /// Last event is far in the past — ingestion looks stopped.
    Stale,
}

impl FreshnessStatus {
    /// The verdict for the most recent indexed event at `now`; none at all is `Stale`.
    pub fn from_last_event(last_event_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> Self {
        let Some(last) = last_event_at else {
            return FreshnessStatus::Stale;
        };

        let age = now - last;

        if age <= LIVE_THRESHOLD {
            FreshnessStatus::Live
        } else if age <= STALE_THRESHOLD {
            FreshnessStatus::Delayed
        } else {
            FreshnessStatus::Stale
        }
    }

    /// Stable tag of the API boundary.
    pub fn as_str(&self) -> &'static str {
        match self {
            FreshnessStatus::Live => "live",
            FreshnessStatus::Delayed => "delayed",
            FreshnessStatus::Stale => "stale",
        }
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
