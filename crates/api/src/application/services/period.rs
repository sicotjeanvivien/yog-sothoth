//! What every read of a produced resource shares: the period it covers, closed at the last
//! frozen hour, and the buckets a series is cut into.

use yog_core::{
    RepositoryResult,
    domain::{FrozenPeriod, ProtocolActivityLookup, Window},
};

/// What a read found over its period; `period` is `None` before the first freeze.
#[derive(Debug, Clone)]
pub(crate) struct Measured<T> {
    pub(crate) period: Option<FrozenPeriod>,
    pub(crate) items: T,
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
