//! How long raw rows have waited for a continuous aggregate to materialise them.
//! ⚠️ A stalled materialisation raises no error: the aggregate just stops moving.

use chrono::{DateTime, Duration, Utc};

/// What one continuous aggregate still has to materialise, as the database
/// reports it: where its materialisation ends, and the oldest raw row past it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializationBacklog {
    /// The aggregate's view name.
    pub aggregate: String,
    /// Where its materialisation ends, `None` before its first bucket; for the
    /// diagnosis, not part of the lateness rule.
    pub watermark: Option<DateTime<Utc>>,
    /// The oldest raw row not materialised yet.
    pub oldest_pending_at: Option<DateTime<Utc>>,
}

impl MaterializationBacklog {
    /// How long the oldest pending raw row has waited at `now`.
    /// ⚠️ Not the watermark's age (it blames an indexer stop on the refresh), nor
    /// newest row minus watermark (it freezes when a table stops receiving rows).
    pub fn pending_for(&self, now: DateTime<Utc>) -> Option<Duration> {
        let oldest = self.oldest_pending_at?;
        Some((now - oldest).max(Duration::zero()))
    }

    /// The wait of the oldest pending row, **only** past `max_wait`: the one place
    /// the lateness rule is written.
    pub fn late_by(&self, now: DateTime<Utc>, max_wait: Duration) -> Option<Duration> {
        self.pending_for(now).filter(|wait| *wait > max_wait)
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
