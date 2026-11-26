use chrono::{DateTime, TimeDelta, Utc};

use super::Window;

/// The first tranche whose pools' facts are still held, and the end of the last one written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrittenTranches {
    pub first: DateTime<Utc>,
    pub through: DateTime<Utc>,
}

/// The tranches `from..as_of` one read covers, through the last one written: unlike a
/// [`FrozenPeriod`](super::FrozenPeriod), its last hour may be open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranchePeriod {
    pub from: DateTime<Utc>,
    /// The end of the last tranche written: the runs read started before it.
    pub as_of: DateTime<Utc>,
    /// The first tranche the read's measure holds: a period starting earlier misses some.
    pub covered_since: DateTime<Utc>,
}

impl TranchePeriod {
    /// `window` through `as_of`; covered from `measured_since`, or never before it is measured.
    pub fn of(as_of: DateTime<Utc>, window: Window, measured_since: Option<DateTime<Utc>>) -> Self {
        Self {
            from: as_of - window.duration(),
            as_of,
            covered_since: measured_since.unwrap_or(as_of),
        }
    }

    pub fn is_covered(&self) -> bool {
        self.from >= self.covered_since
    }

    /// The last hour through the last tranche written, moved `offset` back.
    pub fn hour_through(tranches: WrittenTranches, offset: Option<Window>) -> Self {
        Self::of(
            tranches.through - offset.map_or(TimeDelta::zero(), Window::duration),
            Window::Hour,
            Some(tranches.first),
        )
    }
}

#[cfg(test)]
#[path = "tranche_period_tests.rs"]
mod tests;
