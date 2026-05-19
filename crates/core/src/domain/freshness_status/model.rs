//! Ingestion freshness — how recent the indexed data is.
//!
//! The "Solana Live" panel needs to tell the user whether the data
//! they are looking at is current. That judgement — turning the age
//! of the last indexed event into a Live / Delayed / Stale verdict —
//! is a business rule, so it lives here in the domain, not in the
//! API layer.

use chrono::{DateTime, Duration, Utc};

/// Below this age, ingestion is considered healthy ("Live").
///
/// The indexer watches a small allowlist of pools, so short lulls
/// with no events are normal — the threshold is deliberately
/// generous rather than alarmist.
const LIVE_THRESHOLD: Duration = Duration::minutes(2);

/// Below this age (and above `LIVE_THRESHOLD`), ingestion is
/// "Delayed". Above it, "Stale".
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
    /// Derive the freshness verdict from the timestamp of the most
    /// recent indexed event, evaluated against `now`.
    ///
    /// `last_event_at` is `None` when no event has ever been indexed
    /// (empty database) — treated as `Stale`, since there is no
    /// evidence of a live flow.
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

    /// Stable lowercase tag, for serialization on the API boundary.
    pub fn as_str(&self) -> &'static str {
        match self {
            FreshnessStatus::Live => "live",
            FreshnessStatus::Delayed => "delayed",
            FreshnessStatus::Stale => "stale",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(minutes_ago: i64, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        Some(now - Duration::minutes(minutes_ago))
    }

    #[test]
    fn recent_event_is_live() {
        let now = Utc::now();
        assert_eq!(
            FreshnessStatus::from_last_event(at(1, now), now),
            FreshnessStatus::Live
        );
    }

    #[test]
    fn mid_age_event_is_delayed() {
        let now = Utc::now();
        assert_eq!(
            FreshnessStatus::from_last_event(at(5, now), now),
            FreshnessStatus::Delayed
        );
    }

    #[test]
    fn old_event_is_stale() {
        let now = Utc::now();
        assert_eq!(
            FreshnessStatus::from_last_event(at(30, now), now),
            FreshnessStatus::Stale
        );
    }

    #[test]
    fn no_event_is_stale() {
        let now = Utc::now();
        assert_eq!(
            FreshnessStatus::from_last_event(None, now),
            FreshnessStatus::Stale
        );
    }

    #[test]
    fn exact_live_boundary_is_live() {
        let now = Utc::now();
        // Exactly 2 minutes old — inclusive lower bound.
        assert_eq!(
            FreshnessStatus::from_last_event(at(2, now), now),
            FreshnessStatus::Live
        );
    }
}
