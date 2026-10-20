//! What one cycle of yog-analytic writes: the 5-minute tranches it values once,
//! the hours it rewrites from them, and the ones it freezes for good.

use chrono::{DateTime, DurationRound, TimeDelta, Utc};

/// How long after its close an hour is still rewritten.
pub const FREEZE_MARGIN: TimeDelta = TimeDelta::minutes(10);

/// The most hours one cycle rewrites: hours missed during an outage are caught
/// up over the following cycles, oldest first.
pub const MAX_HOURS_PER_CYCLE: i64 = 6;

const TRANCHE_MINUTES: i64 = 5;

/// The grain prices and swaps are valued at, once each.
pub const TRANCHE: TimeDelta = TimeDelta::minutes(TRANCHE_MINUTES);

/// The most tranches one cycle values: the hours of [`MAX_HOURS_PER_CYCLE`].
pub const MAX_TRANCHES_PER_CYCLE: i32 = (MAX_HOURS_PER_CYCLE * 60 / TRANCHE_MINUTES) as i32;

/// How far the analytic tables go.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AnalyticProgress {
    /// The first hour ever written, `None` before the first cycle.
    pub first_hour: Option<DateTime<Utc>>,
    pub last_frozen: Option<DateTime<Utc>>,
    pub last_tranche: Option<DateTime<Utc>>,
}

/// One cycle: the tranches `tranches_from..=tranches_through`, each valued once,
/// the hours `from..=through`, each rewritten whole — none when the end is
/// before the start — and the TVL measured at `at`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CycleWrite {
    pub tranches_from: DateTime<Utc>,
    pub tranches_through: DateTime<Utc>,
    pub from: DateTime<Utc>,
    pub through: DateTime<Utc>,
    /// Never past `through`, nor past an hour whose last tranche is unwritten.
    pub freeze_through: Option<DateTime<Utc>>,
    /// The hour of `at`, `None` when that hour is already frozen.
    pub tvl_hour: Option<DateTime<Utc>>,
    pub at: DateTime<Utc>,
}

impl CycleWrite {
    /// The cycle to run at `now`. The tranche `T` is valued on the first cycle
    /// at or after `T + TRANCHE + FREEZE_MARGIN`, the hour `H` frozen on the
    /// first at or after `H + 1 h + FREEZE_MARGIN`; nothing before the first hour.
    pub fn plan(now: DateTime<Utc>, progress: AnalyticProgress) -> Self {
        let current = hour_of(now);
        let from = match (progress.last_frozen, progress.first_hour) {
            (Some(frozen), _) => frozen + TimeDelta::hours(1),
            (None, Some(first)) => first,
            (None, None) => current,
        };
        let through = current.min(from + TimeDelta::hours(MAX_HOURS_PER_CYCLE - 1));
        let closed = hour_of(now - TimeDelta::hours(1) - FREEZE_MARGIN);
        let tranches_from = progress.last_tranche.map_or(from, |last| last + TRANCHE);
        let tranches_through = tranche_of(now - TRANCHE - FREEZE_MARGIN)
            .min(tranches_from + TRANCHE * (MAX_TRANCHES_PER_CYCLE - 1));
        Self {
            tranches_from,
            tranches_through,
            from,
            through,
            freeze_through: Some(closed),
            tvl_hour: Some(current),
            at: now,
        }
        .without_written(progress)
    }

    /// This cycle without what `written` already holds — the hours up to its
    /// last frozen, the tranches up to its last — and without a freeze past
    /// what the cycle writes: the one place these rules are applied.
    pub fn without_written(self, written: AnalyticProgress) -> Self {
        let from = match written.last_frozen {
            Some(frozen) => self.from.max(frozen + TimeDelta::hours(1)),
            None => self.from,
        };
        let tranches_from = [written.last_tranche.map(|last| last + TRANCHE), Some(from)]
            .into_iter()
            .flatten()
            .fold(self.tranches_from, DateTime::max);
        let tranches_written = Some(self.tranches_through)
            .filter(|through| *through >= tranches_from)
            .max(written.last_tranche);
        let complete = tranches_written.map(|last| hour_of(last + TRANCHE) - TimeDelta::hours(1));
        Self {
            tranches_from,
            from,
            freeze_through: self
                .freeze_through
                .zip(complete)
                .map(|(hour, complete)| hour.min(self.through).min(complete))
                .filter(|hour| *hour >= from),
            tvl_hour: self
                .tvl_hour
                .filter(|hour| written.last_frozen.is_none_or(|frozen| *hour > frozen)),
            ..self
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CycleOutcome {
    /// The cycle as written, once the hours frozen since it was planned are out.
    pub written: CycleWrite,
    pub tranches_written: u64,
    /// The (pool, hour) facts upserted.
    pub pool_hours: u64,
    pub hours_frozen: u64,
}

fn hour_of(at: DateTime<Utc>) -> DateTime<Utc> {
    at.duration_trunc(TimeDelta::hours(1))
        .expect("an hour divides every instant chrono can represent in nanoseconds")
}

fn tranche_of(at: DateTime<Utc>) -> DateTime<Utc> {
    at.duration_trunc(TRANCHE)
        .expect("five minutes divide every instant chrono can represent in nanoseconds")
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
