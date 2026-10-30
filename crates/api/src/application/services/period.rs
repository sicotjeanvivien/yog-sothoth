//! What every read of a produced resource shares: the period it covers, and the buckets a
//! series is cut into.

use chrono::{DateTime, DurationRound, TimeDelta, Utc};
use yog_core::{
    RepositoryResult,
    domain::{FrozenPeriod, ProtocolActivityLookup, TRANCHE, TranchePeriod, Window},
};

/// How far behind the end of the latest period a cursor's end stays accepted, for pages read slowly.
pub(crate) const CURSOR_END_MARGIN: TimeDelta = TimeDelta::hours(1);

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
    /// How long past `as_of` the period may stay before it runs late.
    fn late_after(&self) -> TimeDelta;
    /// What must cover the window for it to open.
    fn opening_since(&self) -> DateTime<Utc> {
        self.covered_since()
    }
}

/// Closed whatever the facts hold, until their reads are measured on the prod.
const CLOSED_WINDOWS: [Window; 1] = [Window::Month];

const FROZEN_LATE_AFTER: TimeDelta = TimeDelta::hours(2);
const TRANCHE_LATE_AFTER: TimeDelta = TimeDelta::minutes(30);

/// Whether a window is open, when it will be, and whether its figures run late.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Availability {
    pub(crate) open: bool,
    pub(crate) opens_at: Option<DateTime<Utc>>,
    pub(crate) late: bool,
}

impl Availability {
    /// `period` is `None` before the first freeze, or the first tranche: closed, nothing late.
    /// ⚠️ `period` is moved `offset` back: its end and its opening are judged at the unmoved end.
    pub(crate) fn of<P: PeriodBounds>(
        window: Window,
        offset: Option<Window>,
        period: Option<P>,
        now: DateTime<Utc>,
    ) -> Self {
        let Some(period) = period else {
            return Self {
                open: false,
                opens_at: None,
                late: false,
            };
        };
        let offset = offset.map_or(TimeDelta::zero(), Window::duration);
        let end = period.as_of() + offset;
        let late = now - end > period.late_after();
        if CLOSED_WINDOWS.contains(&window) {
            return Self {
                open: false,
                opens_at: None,
                late,
            };
        }
        let opening = period.opening_since();
        let opens_at = opening + window.duration() + offset;
        let open = end >= opens_at;
        Self {
            open,
            // ⚠️ Not measured yet, covered from `as_of` on: a date that would only slide forward.
            opens_at: (!open && opening < end).then_some(opens_at),
            late,
        }
    }
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
    fn late_after(&self) -> TimeDelta {
        FROZEN_LATE_AFTER
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
    fn late_after(&self) -> TimeDelta {
        TRANCHE_LATE_AFTER
    }
}

/// The period of a measure read over `window`: [`Window::Hour`] through the last tranche
/// written, the others through the last frozen hour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReadPeriod {
    Frozen(FrozenPeriod),
    Tranche(TranchePeriod),
}

impl PeriodBounds for ReadPeriod {
    fn from(&self) -> DateTime<Utc> {
        match self {
            Self::Frozen(p) => p.from,
            Self::Tranche(p) => p.from,
        }
    }
    fn as_of(&self) -> DateTime<Utc> {
        match self {
            Self::Frozen(p) => p.as_of,
            Self::Tranche(p) => p.as_of,
        }
    }
    fn covered_since(&self) -> DateTime<Utc> {
        match self {
            Self::Frozen(p) => p.covered_since,
            Self::Tranche(p) => p.covered_since,
        }
    }
    fn late_after(&self) -> TimeDelta {
        match self {
            Self::Frozen(p) => p.late_after(),
            Self::Tranche(p) => p.late_after(),
        }
    }
}

/// A period a series draws once the facts cover it, its own measure covering it from `period`'s
/// `covered_since`, maybe in part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DrawnPeriod {
    pub(crate) period: ReadPeriod,
    pub(crate) facts_since: DateTime<Utc>,
}

impl PeriodBounds for DrawnPeriod {
    fn from(&self) -> DateTime<Utc> {
        self.period.from()
    }
    fn as_of(&self) -> DateTime<Utc> {
        self.period.as_of()
    }
    fn covered_since(&self) -> DateTime<Utc> {
        self.period.covered_since()
    }
    fn late_after(&self) -> TimeDelta {
        self.period.late_after()
    }
    fn opening_since(&self) -> DateTime<Utc> {
        self.facts_since
    }
}

impl ReadPeriod {
    pub(crate) fn is_covered(&self) -> bool {
        match self {
            Self::Frozen(p) => p.is_covered(),
            Self::Tranche(p) => p.is_covered(),
        }
    }

    /// The step its end moves by: an hour frozen, a tranche through the last one written.
    pub(crate) fn step(&self) -> TimeDelta {
        match self {
            Self::Frozen(_) => TimeDelta::hours(1),
            Self::Tranche(_) => TRANCHE,
        }
    }

    /// The same window ended at `as_of`: a later page read over its first page's end.
    pub(crate) fn ending_at(self, as_of: DateTime<Utc>) -> Self {
        match self {
            Self::Frozen(p) => Self::Frozen(FrozenPeriod {
                from: as_of - (p.as_of - p.from),
                as_of,
                ..p
            }),
            Self::Tranche(p) => Self::Tranche(TranchePeriod {
                from: as_of - (p.as_of - p.from),
                as_of,
                ..p
            }),
        }
    }

    /// Covered from `since` on, a frozen period from its first whole hour; before a measure starts,
    /// covered from `as_of`, so nothing reads as covered.
    pub(crate) fn measured_from(self, since: Option<DateTime<Utc>>) -> Self {
        match self {
            Self::Frozen(p) => Self::Frozen(measured_from(p, since.map(first_whole_hour))),
            Self::Tranche(p) => Self::Tranche(TranchePeriod {
                covered_since: since.map_or(p.as_of, |since| since.max(p.covered_since)),
                ..p
            }),
        }
    }
}

/// `window` moved `offset` back, read as [`ReadPeriod`]; `None` before the first freeze, or the
/// first tranche for [`Window::Hour`].
pub(crate) async fn build_read_period(
    hours: &dyn ProtocolActivityLookup,
    window: Window,
    offset: Option<Window>,
) -> RepositoryResult<Option<ReadPeriod>> {
    Ok(match window {
        Window::Hour => hours
            .written_tranches()
            .await?
            .map(|tranches| ReadPeriod::Tranche(TranchePeriod::hour_through(tranches, offset))),
        _ => build_frozen_period(hours, window, offset)
            .await?
            .map(ReadPeriod::Frozen),
    })
}

/// The last hour through the last tranche written; `None` before the first.
pub(crate) async fn build_last_hour(
    hours: &dyn ProtocolActivityLookup,
) -> RepositoryResult<Option<TranchePeriod>> {
    Ok(hours
        .written_tranches()
        .await?
        .map(|tranches| TranchePeriod::hour_through(tranches, None)))
}

/// The step of a `…/series`: 5 minutes over the last hour only, 6 hours over a week only, an hour
/// or a day over the other windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SeriesInterval {
    FiveMinutes,
    Hour,
    SixHours,
    Day,
}

impl SeriesInterval {
    /// The step a window is read at when the request names none.
    pub(crate) fn of(window: Window) -> Self {
        match window {
            Window::Hour => Self::FiveMinutes,
            Window::Day => Self::Hour,
            Window::Week => Self::SixHours,
            Window::Month => Self::Day,
        }
    }

    /// Whether `window` is cut at this step: 5 minutes for 1h, which nothing else is cut into, and
    /// 6 hours for a week.
    pub(crate) fn fits(self, window: Window) -> bool {
        match self {
            Self::FiveMinutes => window == Window::Hour,
            Self::SixHours => window == Window::Week,
            Self::Hour | Self::Day => window != Window::Hour,
        }
    }

    /// The frozen buckets this step reads; `None` for 5 minutes, read from the tranches.
    pub(crate) fn frozen_bucket(self) -> Option<BucketPeriod> {
        match self {
            Self::FiveMinutes => None,
            Self::Hour => Some(BucketPeriod::Hour),
            Self::SixHours => Some(BucketPeriod::SixHours),
            Self::Day => Some(BucketPeriod::Day),
        }
    }
}

/// The buckets of a series: one per hour, or one per 6 or 24 hours ending at `as_of`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BucketPeriod {
    Hour,
    SixHours,
    Day,
}

impl BucketPeriod {
    pub(crate) fn width(self) -> TimeDelta {
        match self {
            Self::Hour => TimeDelta::hours(1),
            Self::SixHours => TimeDelta::hours(6),
            Self::Day => TimeDelta::days(1),
        }
    }
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

/// `period` covered from `since`, the first hour a resource was measured, and no earlier than it.
pub(crate) fn measured_from(period: FrozenPeriod, since: Option<DateTime<Utc>>) -> FrozenPeriod {
    // ⚠️ Not measured yet: covered from `as_of` on, so no period reads as covered.
    let covered_since = since.map_or(period.as_of, |since| since.max(period.covered_since));
    FrozenPeriod {
        covered_since,
        ..period
    }
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

#[cfg(test)]
#[path = "tests/period_tests.rs"]
mod tests;
