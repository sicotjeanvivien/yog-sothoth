//! What one cycle of yog-analytic writes: the hours it rewrites, and the ones
//! it freezes for good.

use chrono::{DateTime, DurationRound, TimeDelta, Utc};

/// How long after its close an hour still takes the events written late.
pub const FREEZE_MARGIN: TimeDelta = TimeDelta::minutes(10);

/// The most hours one cycle rewrites: hours missed during an outage are caught
/// up over the following cycles, oldest first.
pub const MAX_HOURS_PER_CYCLE: i64 = 6;

/// How far the analytic tables go.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AnalyticProgress {
    /// The first hour ever written, `None` before the first cycle.
    pub first_hour: Option<DateTime<Utc>>,
    pub last_frozen: Option<DateTime<Utc>>,
}

/// One cycle: the hours `from..=through`, each rewritten whole — none when
/// `through` is before `from` — and the TVL measured at `at`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CycleWrite {
    pub from: DateTime<Utc>,
    pub through: DateTime<Utc>,
    /// Never past `through`: an hour freezes only on the cycle that rewrote it.
    pub freeze_through: Option<DateTime<Utc>>,
    /// The hour of `at`, `None` when that hour is already frozen.
    pub tvl_hour: Option<DateTime<Utc>>,
    pub at: DateTime<Utc>,
}

impl CycleWrite {
    /// The cycle to run at `now`. The hour `H` freezes on the first cycle at
    /// or after `H + 1 h + FREEZE_MARGIN`; nothing before the first hour.
    pub fn plan(now: DateTime<Utc>, progress: AnalyticProgress) -> Self {
        let current = hour_of(now);
        let from = match (progress.last_frozen, progress.first_hour) {
            (Some(frozen), _) => frozen + TimeDelta::hours(1),
            (None, Some(first)) => first,
            (None, None) => current,
        };
        let through = current.min(from + TimeDelta::hours(MAX_HOURS_PER_CYCLE - 1));
        let closed = hour_of(now - TimeDelta::hours(1) - FREEZE_MARGIN);
        Self {
            from,
            through,
            freeze_through: Some(closed),
            tvl_hour: Some(current),
            at: now,
        }
        .without_frozen(progress.last_frozen)
    }

    /// This cycle without the hours up to `last_frozen`, and without a freeze
    /// past the hours it rewrites: the one place both rules are applied.
    pub fn without_frozen(self, last_frozen: Option<DateTime<Utc>>) -> Self {
        let from = match last_frozen {
            Some(frozen) => self.from.max(frozen + TimeDelta::hours(1)),
            None => self.from,
        };
        Self {
            from,
            freeze_through: self
                .freeze_through
                .map(|hour| hour.min(self.through))
                .filter(|hour| *hour >= from),
            tvl_hour: self
                .tvl_hour
                .filter(|hour| last_frozen.is_none_or(|frozen| *hour > frozen)),
            ..self
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CycleOutcome {
    /// The cycle as written, once the hours frozen since it was planned are out.
    pub written: CycleWrite,
    /// The (pool, hour) facts upserted.
    pub pool_hours: u64,
    pub hours_frozen: u64,
}

fn hour_of(at: DateTime<Utc>) -> DateTime<Utc> {
    at.duration_trunc(TimeDelta::hours(1))
        .expect("an hour divides every instant chrono can represent in nanoseconds")
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
