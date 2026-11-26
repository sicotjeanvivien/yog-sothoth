use chrono::{DateTime, TimeDelta, Utc};

use super::cycle::hour_of;

/// What a TVL measure did: one per hour, the hour's first measure kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TvlMeasure {
    Measured,
    /// The hour already had its TVL: nothing written.
    AlreadyMeasured,
}

/// The hour a TVL measured at `now` is written to: the last closed one, read from the
/// pools' current state.
pub fn last_closed_hour(now: DateTime<Utc>) -> DateTime<Utc> {
    hour_of(now) - TimeDelta::hours(1)
}

#[cfg(test)]
#[path = "tvl_measure_tests.rs"]
mod tests;
