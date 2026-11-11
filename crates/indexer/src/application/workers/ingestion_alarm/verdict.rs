//! How one check of the ingestion ends, and what it says: the verdict the
//! heartbeat is told, and the text that goes to `/fail` and to the logs.

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use yog_core::domain::FreshnessStatus;

/// How one check ended — and what the heartbeat is told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// The last event is recent: success.
    Live,
    /// The dashboard no longer shows the ingestion live: failure.
    Delayed { last_event_at: DateTime<Utc> },
    /// Nothing indexed for too long, or ever (`None`): failure.
    Stale {
        last_event_at: Option<DateTime<Utc>>,
    },
    /// The last event could not be read; the text says why: failure.
    Unreadable(String),
}

impl Verdict {
    /// Judge the last event at `now`, by the rule `core` holds.
    pub(super) fn judge(last_event_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> Self {
        match (
            FreshnessStatus::from_last_event(last_event_at, now),
            last_event_at,
        ) {
            (FreshnessStatus::Live, _) => Self::Live,
            (FreshnessStatus::Delayed, Some(at)) => Self::Delayed { last_event_at: at },
            (FreshnessStatus::Delayed, None) | (FreshnessStatus::Stale, _) => {
                Self::Stale { last_event_at }
            }
        }
    }

    /// The `outcome` label of the checks counter.
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Live => FreshnessStatus::Live.as_str(),
            Self::Delayed { .. } => FreshnessStatus::Delayed.as_str(),
            Self::Stale { .. } => FreshnessStatus::Stale.as_str(),
            Self::Unreadable(_) => "unreadable",
        }
    }

    /// What `/fail` and the logs say, or `None` when the check succeeds.
    pub(super) fn failure_reason(&self, now: DateTime<Utc>) -> Option<String> {
        match self {
            Self::Live => None,
            Self::Delayed { last_event_at: at }
            | Self::Stale {
                last_event_at: Some(at),
            } => Some(format!(
                "{}: last event {} ago, at {}",
                self.label(),
                hours_minutes(now - *at),
                at.format("%Y-%m-%d %H:%M UTC")
            )),
            Self::Stale {
                last_event_at: None,
            } => Some("stale: no event has ever been indexed".to_string()),
            Self::Unreadable(error) => Some(format!("unreadable: {error}")),
        }
    }
}

/// A duration as `4h05m`, the form the reason uses.
fn hours_minutes(duration: ChronoDuration) -> String {
    let minutes = duration.num_minutes();
    format!("{}h{:02}m", minutes / 60, minutes % 60)
}
