//! Where the sessions leave the spans they received and the recorder takes them: the hand-over
//! between `session` and `listener`.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, MutexGuard},
};

use chrono::{DateTime, Utc};
use yog_core::domain::{ObservedSlots, Protocol};

use super::session::received_span::ReceivedSpan;

/// One session's span, as the recorder reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RangeToRecord {
    pub(super) protocols: Arc<[Protocol]>,
    pub(super) span: ReceivedSpan,
    /// The session is over: once this span is written, nothing will widen it.
    pub(super) ended: bool,
}

impl RangeToRecord {
    /// One row per protocol watched whole.
    pub(super) fn rows(
        &self,
        session_started_at: DateTime<Utc>,
    ) -> impl Iterator<Item = ObservedSlots> + '_ {
        let (first_slot, first_slot_at) = self.span.first();
        let (last_slot, last_slot_at) = self.span.last();
        self.protocols.iter().map(move |&protocol| ObservedSlots {
            protocol,
            session_started_at,
            first_slot,
            first_slot_at,
            last_slot,
            last_slot_at,
        })
    }
}

/// Every session's latest span, under its start, until the recorder has written its last.
/// ⚠️ One entry per session, not one for all: a session's last span must survive the next
/// session's first, or every reconnection reads as an unwatched range.
#[derive(Clone, Default)]
pub(super) struct RangesToRecord(Arc<Mutex<BTreeMap<DateTime<Utc>, RangeToRecord>>>);

impl RangesToRecord {
    fn entries(&self) -> MutexGuard<'_, BTreeMap<DateTime<Utc>, RangeToRecord>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(super) fn snapshot(&self) -> BTreeMap<DateTime<Utc>, RangeToRecord> {
        self.entries().clone()
    }

    pub(super) fn publish(&self, session: DateTime<Utc>, range: RangeToRecord) {
        self.entries().insert(session, range);
    }

    pub(super) fn end(&self, session: DateTime<Utc>) {
        if let Some(range) = self.entries().get_mut(&session) {
            range.ended = true;
        }
    }

    /// Drop an ended session whose last span is written.
    pub(super) fn forget(&self, session: DateTime<Utc>) {
        self.entries().remove(&session);
    }
}

#[cfg(test)]
#[path = "tests/ranges_to_record_tests.rs"]
mod tests;
