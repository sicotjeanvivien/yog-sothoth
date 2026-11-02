//! What the stream observed, written so that a cut outlives the process: each session publishes
//! the slots it saw closed, and the writer records them a period late. A gap is never written: it
//! is the complement of the ranges, read in SQL (`ingest_coverage_gaps`).

mod log;

use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use tokio::{sync::watch, time::MissedTickBehavior};
use tokio_util::sync::CancellationToken;
use yog_core::domain::{IngestCoverageRepository, ObservedSlots, Protocol, SlotRange};

use super::metrics::{CoverageWrite, GrpcListenerMetrics};

/// How often the writer records, and how late what it records is.
pub(crate) const COVERAGE_PERIOD: Duration = Duration::from_secs(60);

/// One session's range, as the writer reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SessionRange {
    started_at: DateTime<Utc>,
    protocols: Arc<[Protocol]>,
    range: SlotRange,
}

#[cfg(test)]
impl SessionRange {
    pub(super) fn range(&self) -> SlotRange {
        self.range
    }
}

/// What a session publishes; `None` until a session has seen a timed block-meta.
pub(super) type Published = watch::Sender<Option<SessionRange>>;

/// A session's side: widens its range on each timed block-meta and publishes it.
pub(super) struct SessionCoverage {
    started_at: DateTime<Utc>,
    /// The protocols watched whole; a protocol watched pool by pool is not covered.
    protocols: Arc<[Protocol]>,
    range: Option<SlotRange>,
    publish: Published,
}

impl SessionCoverage {
    pub(super) fn new(protocols: Arc<[Protocol]>, publish: Published) -> Self {
        Self {
            started_at: Utc::now(),
            protocols,
            range: None,
            publish,
        }
    }

    /// A block-meta closed `slot` at `at`.
    pub(super) fn on_block_meta(&mut self, slot: u64, at: DateTime<Utc>) {
        if self.protocols.is_empty() {
            return;
        }
        let range = match &mut self.range {
            Some(range) => {
                range.include(slot, at);
                *range
            }
            None => *self.range.insert(SlotRange::starting(slot, at)),
        };
        self.publish.send_replace(Some(SessionRange {
            started_at: self.started_at,
            protocols: Arc::clone(&self.protocols),
            range,
        }));
    }
}

/// Records the sessions' ranges, one write per protocol and period.
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

    /// Write, at each tick, the range read at the tick before, until `stop`.
    /// ⚠️ A period late on purpose: a transaction whose block-meta came may still wait for the
    /// persistor, so a cut declares a gap too wide, never too narrow.
    pub(super) async fn run(
        self,
        mut published: watch::Receiver<Option<SessionRange>>,
        stop: CancellationToken,
    ) {
        let mut ticker = tokio::time::interval(self.period);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut due: Option<SessionRange> = None;

        loop {
            tokio::select! {
                biased;
                _ = stop.cancelled() => return,
                _ = ticker.tick() => {}
            }
            if let Some(due) = due.take() {
                self.write(&due).await;
            }
            due = published.borrow_and_update().clone();
        }
    }

    /// One row per protocol; a failure is counted and stepped over, ingestion goes on.
    async fn write(&self, due: &SessionRange) {
        for &protocol in due.protocols.iter() {
            let observed = ObservedSlots {
                protocol,
                session_started_at: due.started_at,
                range: due.range,
            };
            match self.repository.record(&observed).await {
                Ok(()) => GrpcListenerMetrics::record_coverage_write(CoverageWrite::Written),
                Err(error) => {
                    GrpcListenerMetrics::record_coverage_write(CoverageWrite::Failed);
                    log::write_failed(protocol, &error);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/coverage_tests.rs"]
mod tests;
