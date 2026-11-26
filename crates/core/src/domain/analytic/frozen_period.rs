use chrono::{DateTime, TimeDelta, Utc};

use super::Window;

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

#[cfg(test)]
#[path = "frozen_period_tests.rs"]
mod tests;
