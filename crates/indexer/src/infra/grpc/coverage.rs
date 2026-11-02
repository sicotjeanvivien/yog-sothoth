//! What the stream observed, written so that a cut outlives the process: each session publishes
//! the slots it will not ask for again, and the writer records them a period late. A gap is never
//! written: it is the complement of the ranges, read in SQL (`ingest_coverage_gaps`).

mod log;

use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::{DateTime, Utc};
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use yog_core::domain::{IngestCoverageRepository, ObservedSlots, Protocol, SlotRange};

use super::metrics::{CoverageWrite, GrpcListenerMetrics};

/// How often the writer records, and how late what it records is.
pub(crate) const COVERAGE_PERIOD: Duration = Duration::from_secs(60);

/// One session's range, as the writer reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SessionRange {
    protocols: Arc<[Protocol]>,
    range: SlotRange,
    /// The session is over: once this range is recorded, nothing will widen it.
    ended: bool,
}

#[cfg(test)]
impl SessionRange {
    pub(super) fn range(&self) -> SlotRange {
        self.range
    }
}

/// The ranges of one run's sessions, each under its start.
/// ⚠️ One entry per session, not one slot for all: a session's last range must survive the next
/// session's first, or every reconnection reads as a gap.
#[derive(Clone, Default)]
pub(super) struct Observed(Arc<Mutex<BTreeMap<DateTime<Utc>, SessionRange>>>);

impl Observed {
    fn entries(&self) -> std::sync::MutexGuard<'_, BTreeMap<DateTime<Utc>, SessionRange>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(super) fn snapshot(&self) -> BTreeMap<DateTime<Utc>, SessionRange> {
        self.entries().clone()
    }

    fn publish(&self, session: DateTime<Utc>, range: SessionRange) {
        self.entries().insert(session, range);
    }

    fn end(&self, session: DateTime<Utc>) {
        if let Some(entry) = self.entries().get_mut(&session) {
            entry.ended = true;
        }
    }

    /// Drop an ended session whose last range is recorded.
    fn forget(&self, session: DateTime<Utc>) {
        self.entries().remove(&session);
    }
}

/// A session's side: the slots it saw closed, published up to the slot before its resume point.
pub(super) struct SessionCoverage {
    started_at: DateTime<Utc>,
    /// The protocols watched whole; a protocol watched pool by pool is not covered.
    protocols: Arc<[Protocol]>,
    range: Option<SlotRange>,
    /// Timed block-metas at or past the resume point: a reconnection would ask for them again.
    unsettled: BTreeMap<u64, DateTime<Utc>>,
    observed: Observed,
}

impl SessionCoverage {
    pub(super) fn new(protocols: Arc<[Protocol]>, observed: Observed) -> Self {
        Self {
            started_at: Utc::now(),
            protocols,
            range: None,
            unsettled: BTreeMap::new(),
            observed,
        }
    }

    /// A block-meta closed `slot` at `at`; `resume_from` is where a reconnection would start now.
    /// ⚠️ Only slots below `resume_from` count: one at or past it may still lose transactions to
    /// a cut whose replay is refused, and a gap declared too narrow is the one defect not allowed.
    pub(super) fn on_block_meta(&mut self, slot: u64, at: DateTime<Utc>, resume_from: Option<u64>) {
        if self.protocols.is_empty() {
            return;
        }
        self.unsettled.insert(slot, at);
        let Some(resume_from) = resume_from else {
            return;
        };
        let still_unsettled = self.unsettled.split_off(&resume_from);
        let settled = std::mem::replace(&mut self.unsettled, still_unsettled);
        if settled.is_empty() {
            return;
        }
        for (slot, at) in settled {
            match &mut self.range {
                Some(range) => range.include(slot, at),
                None => self.range = Some(SlotRange::starting(slot, at)),
            }
        }
        if let Some(range) = self.range {
            self.observed.publish(
                self.started_at,
                SessionRange {
                    protocols: Arc::clone(&self.protocols),
                    range,
                    ended: false,
                },
            );
        }
    }
}

impl Drop for SessionCoverage {
    fn drop(&mut self) {
        self.observed.end(self.started_at);
    }
}

/// Records the sessions' ranges, one write per protocol and changed range.
#[derive(Clone)]
pub(crate) struct CoverageWriter {
    repository: Arc<dyn IngestCoverageRepository>,
    period: Duration,
}

impl CoverageWriter {
    /// [`COVERAGE_PERIOD`] in production; a parameter so a test can tick in milliseconds.
    pub(crate) fn new(repository: Arc<dyn IngestCoverageRepository>, period: Duration) -> Self {
        Self { repository, period }
    }

    /// Write, at each tick and at `stop`, the ranges read at the tick before.
    /// ⚠️ A period late on purpose: a transaction released by a slot may still wait for the
    /// persistor, so a cut declares a gap too wide, never too narrow.
    pub(super) async fn run(self, observed: Observed, stop: CancellationToken) {
        let mut ticker = tokio::time::interval(self.period);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut written: HashMap<DateTime<Utc>, SlotRange> = HashMap::new();
        let mut due = BTreeMap::new();

        loop {
            let stopping = tokio::select! {
                biased;
                _ = stop.cancelled() => true,
                _ = ticker.tick() => false,
            };
            self.write(&due, &mut written, &observed).await;
            if stopping {
                return;
            }
            due = observed.snapshot();
        }
    }

    /// Each range not yet recorded as it is; a failure is counted and stepped over, and the
    /// range stays due for the next tick.
    async fn write(
        &self,
        due: &BTreeMap<DateTime<Utc>, SessionRange>,
        written: &mut HashMap<DateTime<Utc>, SlotRange>,
        observed: &Observed,
    ) {
        for (&session, entry) in due {
            if written.get(&session) != Some(&entry.range) {
                if !self.record(session, entry).await {
                    continue;
                }
                written.insert(session, entry.range);
            }
            if entry.ended {
                observed.forget(session);
                written.remove(&session);
            }
        }
    }

    /// One row per protocol; `true` when every one was recorded.
    async fn record(&self, session: DateTime<Utc>, entry: &SessionRange) -> bool {
        let mut recorded = true;
        for &protocol in entry.protocols.iter() {
            let observed = ObservedSlots {
                protocol,
                session_started_at: session,
                range: entry.range,
            };
            match self.repository.record(&observed).await {
                Ok(()) => GrpcListenerMetrics::record_coverage_write(CoverageWrite::Written),
                Err(error) => {
                    GrpcListenerMetrics::record_coverage_write(CoverageWrite::Failed);
                    log::write_failed(protocol, &error);
                    recorded = false;
                }
            }
        }
        recorded
    }
}

#[cfg(test)]
#[path = "tests/coverage_tests.rs"]
mod tests;
