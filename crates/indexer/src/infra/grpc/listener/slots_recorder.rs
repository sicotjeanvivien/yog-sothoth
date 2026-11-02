//! Writes the slots the sessions received for good, a period late, so that a cut outlives the
//! process as a range nobody watched.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::Duration,
};

use chrono::{DateTime, Utc};
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use yog_core::domain::ObservedSlotsRepository;

use crate::infra::grpc::{
    metrics::{GrpcListenerMetrics, ObservedSlotsWrite},
    session::received_slots::{RangeToRecord, RangesToRecord, ReceivedSpan},
};

use super::log;

/// How often the recorder writes, and how late what it writes is.
pub(crate) const RECORD_PERIOD: Duration = Duration::from_secs(60);

/// Writes each session's span, one row per protocol and changed span.
#[derive(Clone)]
pub(crate) struct SlotsRecorder {
    repository: Arc<dyn ObservedSlotsRepository>,
    period: Duration,
}

impl SlotsRecorder {
    /// [`RECORD_PERIOD`] in production; a parameter so a test can tick in milliseconds.
    pub(crate) fn new(repository: Arc<dyn ObservedSlotsRepository>, period: Duration) -> Self {
        Self { repository, period }
    }

    /// Write, at each tick and at `stop`, the spans read at the tick before.
    /// ⚠️ A period late on purpose: a transaction released by a slot may still wait for the
    /// persistor, so a cut declares an unwatched range too wide, never too narrow.
    pub(super) async fn run(self, ranges: RangesToRecord, stop: CancellationToken) {
        let mut ticker = tokio::time::interval(self.period);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut written: HashMap<DateTime<Utc>, ReceivedSpan> = HashMap::new();
        let mut due = BTreeMap::new();

        loop {
            let stopping = tokio::select! {
                biased;
                _ = stop.cancelled() => true,
                _ = ticker.tick() => false,
            };
            self.write(&due, &mut written, &ranges).await;
            if stopping {
                return;
            }
            due = ranges.snapshot();
        }
    }

    /// Each span not yet written as it is; a failure is counted and stepped over, and the span
    /// stays due for the next tick.
    async fn write(
        &self,
        due: &BTreeMap<DateTime<Utc>, RangeToRecord>,
        written: &mut HashMap<DateTime<Utc>, ReceivedSpan>,
        ranges: &RangesToRecord,
    ) {
        for (&session, range) in due {
            if written.get(&session) != Some(&range.span) {
                if !self.record(session, range).await {
                    continue;
                }
                written.insert(session, range.span);
            }
            if range.ended {
                ranges.forget(session);
                written.remove(&session);
            }
        }
    }

    /// `true` when every protocol's row was written.
    async fn record(&self, session: DateTime<Utc>, range: &RangeToRecord) -> bool {
        let mut recorded = true;
        for row in range.rows(session) {
            match self.repository.record(&row).await {
                Ok(()) => {
                    GrpcListenerMetrics::record_observed_slots_write(ObservedSlotsWrite::Written)
                }
                Err(error) => {
                    GrpcListenerMetrics::record_observed_slots_write(ObservedSlotsWrite::Failed);
                    log::slots_not_recorded(row.protocol, &error);
                    recorded = false;
                }
            }
        }
        recorded
    }
}

#[cfg(test)]
#[path = "../tests/slots_recorder_tests.rs"]
mod tests;
