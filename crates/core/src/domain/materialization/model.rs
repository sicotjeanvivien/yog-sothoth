//! How long raw rows have waited for a continuous aggregate to materialise them.
//!
//! A stalled materialisation raises no error anywhere: the aggregate simply
//! stops moving, and every reader keeps serving hours that no longer change —
//! from 16 June to 10 August 2026, all four aggregates sat that way unnoticed.
//! Deciding when a wait has become a fault is a business rule, so it lives
//! here.

use chrono::{DateTime, Duration, Utc};

/// One continuous aggregate, as the database reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateMaterialization {
    /// The aggregate's view name.
    pub aggregate: String,
    /// Where its materialisation ends. `None` while it has never materialised
    /// a single bucket. Reported for the diagnosis; the verdict does not read
    /// it.
    pub watermark: Option<DateTime<Utc>>,
    /// The oldest raw row the aggregate has not materialised yet. `None` when
    /// nothing is waiting.
    pub oldest_pending_at: Option<DateTime<Utc>>,
}

impl AggregateMaterialization {
    /// How long the oldest pending raw row has been waiting at `now`, or
    /// `None` when nothing waits.
    ///
    /// ⚠️ **The oldest pending row, not the newest row nor the watermark's
    /// age.** The watermark's age against the clock grows when the *indexer*
    /// stops — no bucket fills — and would blame the materialisation. The
    /// newest row minus the watermark freezes when a table stops receiving
    /// rows, however long those rows then wait: on 29 September 2026 it read
    /// 28 minutes for six `claim_reward` rows that had waited eight days. A
    /// pending row only grows old if a refresh did not run — or if the indexer
    /// wrote it late: the wait runs from its block time, so rows caught up
    /// after an outage arrive already old, until the next refresh takes them.
    pub fn pending_for(&self, now: DateTime<Utc>) -> Option<Duration> {
        let oldest = self.oldest_pending_at?;
        Some((now - oldest).max(Duration::zero()))
    }

    /// How long the oldest pending row has waited, **only** when that is longer
    /// than `max_wait` — the one place the lateness rule is written. An
    /// aggregate with nothing pending is never late.
    pub fn late_by(&self, now: DateTime<Utc>, max_wait: Duration) -> Option<Duration> {
        self.pending_for(now).filter(|wait| *wait > max_wait)
    }

    /// Whether a row has waited longer than `max_wait` at `now`.
    pub fn is_late(&self, now: DateTime<Utc>, max_wait: Duration) -> bool {
        self.late_by(now, max_wait).is_some()
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
