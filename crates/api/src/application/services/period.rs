//! What every read of a produced resource shares: the period it covers, and the buckets a
//! series is cut into.

use chrono::{DateTime, DurationRound, TimeDelta, Utc};
use yog_core::{
    RepositoryResult,
    domain::{FrozenPeriod, ProtocolActivityLookup, TranchePeriod, Window},
};

/// What a read found over its period; `period` is `None` before the first freeze, or the first
/// tranche written.
#[derive(Debug, Clone)]
pub(crate) struct Measured<T, P = FrozenPeriod> {
    pub(crate) period: Option<P>,
    pub(crate) items: T,
}

/// The bounds an envelope says of a period, frozen or through the last tranche.
pub(crate) trait PeriodBounds: Copy {
    fn from(&self) -> DateTime<Utc>;
    fn as_of(&self) -> DateTime<Utc>;
    fn covered_since(&self) -> DateTime<Utc>;
}

impl PeriodBounds for FrozenPeriod {
    fn from(&self) -> DateTime<Utc> {
        self.from
    }
    fn as_of(&self) -> DateTime<Utc> {
        self.as_of
    }
    fn covered_since(&self) -> DateTime<Utc> {
        self.covered_since
    }
}

impl PeriodBounds for TranchePeriod {
    fn from(&self) -> DateTime<Utc> {
        self.from
    }
    fn as_of(&self) -> DateTime<Utc> {
        self.as_of
    }
    fn covered_since(&self) -> DateTime<Utc> {
        self.covered_since
    }
}

/// The buckets of a series: one per hour, or one per 24 hours ending at `as_of`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BucketPeriod {
    Hour,
    Day,
}

/// `window` through the last frozen hour, moved `offset` back; `None` before the first freeze.
pub(crate) async fn build_frozen_period(
    hours: &dyn ProtocolActivityLookup,
    window: Window,
    offset: Option<Window>,
) -> RepositoryResult<Option<FrozenPeriod>> {
    Ok(hours
        .frozen_hours()
        .await?
        .map(|hours| FrozenPeriod::of(hours, window, offset)))
}

/// The first hour that starts at or after `at`: an hour measured from its start.
pub(crate) fn first_whole_hour(at: DateTime<Utc>) -> DateTime<Utc> {
    let start = at.duration_trunc(TimeDelta::hours(1)).unwrap_or(at);
    if start == at {
        at
    } else {
        start + TimeDelta::hours(1)
    }
}
