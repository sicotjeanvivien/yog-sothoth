//! The slots a session has received for good, and the ranges every session of a run waits to have
//! recorded: what lets an unwatched range outlive the process.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, MutexGuard},
};

use chrono::{DateTime, Utc};
use yog_core::domain::{ObservedSlots, Protocol};

/// First and last slot received for good, with their block times.
/// ⚠️ Lowest and highest, not first and last to arrive: a replay delivers its oldest slots last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::infra::grpc) struct ReceivedSpan {
    first: u64,
    first_at: DateTime<Utc>,
    last: u64,
    last_at: DateTime<Utc>,
}

impl ReceivedSpan {
    fn starting(slot: u64, at: DateTime<Utc>) -> Self {
        Self {
            first: slot,
            first_at: at,
            last: slot,
            last_at: at,
        }
    }

    /// Widen to `slot`; a slot inside the span changes nothing, whatever the slots skipped.
    fn include(&mut self, slot: u64, at: DateTime<Utc>) {
        if slot < self.first {
            self.first = slot;
            self.first_at = at;
        }
        if slot > self.last {
            self.last = slot;
            self.last_at = at;
        }
    }

    #[cfg(test)]
    pub(in crate::infra::grpc) fn slots(&self) -> (u64, u64) {
        (self.first, self.last)
    }
}

/// One session's span, as the recorder reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::infra::grpc) struct RangeToRecord {
    pub(in crate::infra::grpc) protocols: Arc<[Protocol]>,
    pub(in crate::infra::grpc) span: ReceivedSpan,
    /// The session is over: once this span is recorded, nothing will widen it.
    pub(in crate::infra::grpc) ended: bool,
}

impl RangeToRecord {
    /// One row per protocol watched whole.
    pub(in crate::infra::grpc) fn rows(
        &self,
        session_started_at: DateTime<Utc>,
    ) -> impl Iterator<Item = ObservedSlots> + '_ {
        self.protocols.iter().map(move |&protocol| ObservedSlots {
            protocol,
            session_started_at,
            first_slot: self.span.first,
            first_slot_at: self.span.first_at,
            last_slot: self.span.last,
            last_slot_at: self.span.last_at,
        })
    }
}

/// Every session's latest span, under its start, until the recorder has written its last.
/// ⚠️ One entry per session, not one for all: a session's last span must survive the next
/// session's first, or every reconnection reads as an unwatched range.
#[derive(Clone, Default)]
pub(in crate::infra::grpc) struct RangesToRecord(
    Arc<Mutex<BTreeMap<DateTime<Utc>, RangeToRecord>>>,
);

impl RangesToRecord {
    fn entries(&self) -> MutexGuard<'_, BTreeMap<DateTime<Utc>, RangeToRecord>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(in crate::infra::grpc) fn snapshot(&self) -> BTreeMap<DateTime<Utc>, RangeToRecord> {
        self.entries().clone()
    }

    pub(in crate::infra::grpc) fn publish(&self, session: DateTime<Utc>, range: RangeToRecord) {
        self.entries().insert(session, range);
    }

    fn end(&self, session: DateTime<Utc>) {
        if let Some(range) = self.entries().get_mut(&session) {
            range.ended = true;
        }
    }

    /// Drop an ended session whose last span is recorded.
    pub(in crate::infra::grpc) fn forget(&self, session: DateTime<Utc>) {
        self.entries().remove(&session);
    }
}

/// What one session has received for good: the timed block-metas below its resume point.
pub(in crate::infra::grpc) struct ReceivedSlots {
    started_at: DateTime<Utc>,
    /// The protocols watched whole; a protocol watched pool by pool is not watched.
    protocols: Arc<[Protocol]>,
    span: Option<ReceivedSpan>,
    /// Timed block-metas at or past the resume point: a reconnection would ask for them again.
    not_yet: BTreeMap<u64, DateTime<Utc>>,
    ranges: RangesToRecord,
}

impl ReceivedSlots {
    pub(in crate::infra::grpc) fn new(protocols: Arc<[Protocol]>, ranges: RangesToRecord) -> Self {
        Self {
            started_at: Utc::now(),
            protocols,
            span: None,
            not_yet: BTreeMap::new(),
            ranges,
        }
    }

    /// A block-meta closed `slot` at `at`; `resume_from` is where a reconnection would start now.
    /// ⚠️ Only slots below `resume_from` count: one at or past it may still lose transactions to
    /// a cut whose replay is refused, and an unwatched range declared too narrow is the one defect not allowed.
    pub(in crate::infra::grpc) fn on_block_meta(
        &mut self,
        slot: u64,
        at: DateTime<Utc>,
        resume_from: Option<u64>,
    ) {
        if self.protocols.is_empty() {
            return;
        }
        self.not_yet.insert(slot, at);
        let Some(resume_from) = resume_from else {
            return;
        };
        let still_not_yet = self.not_yet.split_off(&resume_from);
        let received = std::mem::replace(&mut self.not_yet, still_not_yet);
        if received.is_empty() {
            return;
        }
        for (slot, at) in received {
            match &mut self.span {
                Some(span) => span.include(slot, at),
                None => self.span = Some(ReceivedSpan::starting(slot, at)),
            }
        }
        if let Some(span) = self.span {
            self.ranges.publish(
                self.started_at,
                RangeToRecord {
                    protocols: Arc::clone(&self.protocols),
                    span,
                    ended: false,
                },
            );
        }
    }
}

impl Drop for ReceivedSlots {
    fn drop(&mut self) {
        self.ranges.end(self.started_at);
    }
}

#[cfg(test)]
#[path = "../tests/received_slots_tests.rs"]
mod tests;
