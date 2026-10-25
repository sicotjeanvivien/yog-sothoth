//! The windows a measure is read over, and the period one read covers: every
//! window ends at the close of the last frozen hour.

use chrono::{DateTime, TimeDelta, Utc};

use crate::domain::Protocol;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Window {
    Day,
    Week,
    Month,
}

impl Window {
    pub const ALL: [Window; 3] = [Window::Day, Window::Week, Window::Month];

    pub const fn hours(self) -> i32 {
        match self {
            Window::Day => 24,
            Window::Week => 168,
            Window::Month => 720,
        }
    }

    pub fn duration(self) -> TimeDelta {
        TimeDelta::hours(self.hours().into())
    }

    /// ⚠️ Also the `period` CHECKs of migrations 029 and 034: a new label fails their inserts.
    pub const fn as_str(self) -> &'static str {
        match self {
            Window::Day => "24h",
            Window::Week => "7d",
            Window::Month => "30d",
        }
    }
}

/// The first hour the analytic tables hold, and the last one frozen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrozenHours {
    pub first: DateTime<Utc>,
    pub last_frozen: DateTime<Utc>,
}

/// The hours `from..as_of` one read covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrozenPeriod {
    pub from: DateTime<Utc>,
    /// The close of its last hour: the instant the read reflects.
    pub as_of: DateTime<Utc>,
    /// The first hour the facts hold: a period starting earlier sums missing hours.
    pub covered_since: DateTime<Utc>,
}

impl FrozenPeriod {
    /// `window` through the last frozen hour, moved `offset` back.
    pub fn of(hours: FrozenHours, window: Window, offset: Option<Window>) -> Self {
        let as_of = hours.last_frozen + TimeDelta::hours(1)
            - offset.map_or(TimeDelta::zero(), Window::duration);
        Self {
            from: as_of - window.duration(),
            as_of,
            covered_since: hours.first,
        }
    }

    pub fn is_covered(&self) -> bool {
        self.from >= self.covered_since
    }

    /// The period's last hour, the one its distinct counts were written at.
    pub fn last_hour(&self) -> DateTime<Utc> {
        self.as_of - TimeDelta::hours(1)
    }
}

/// The tranches `from..as_of` one read covers, through the last one written: unlike a
/// [`FrozenPeriod`], its last hour may be open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranchePeriod {
    pub from: DateTime<Utc>,
    /// The end of the last tranche written: the instant the read reflects.
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
}

/// One protocol's measure over one bucket of a series, an hour or 24 hours from `from`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolBucket<T> {
    pub protocol: Protocol,
    pub from: DateTime<Utc>,
    pub value: T,
}

#[cfg(test)]
#[path = "window_tests.rs"]
mod tests;
