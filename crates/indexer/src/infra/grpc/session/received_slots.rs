//! What one session counts as received for good: the slots of its timed block-metas past its first
//! ones and below what it has settled, left in `RangesToRecord` for the recorder.

use std::{collections::BTreeMap, sync::Arc};

use chrono::{DateTime, Utc};
use yog_core::domain::Protocol;

use crate::infra::grpc::ranges_to_record::{RangeToRecord, RangesToRecord};

use super::{received_span::ReceivedSpan, slot_progress::HELD_BACK_SLOTS};

/// What one session has received for good.
pub(in crate::infra::grpc) struct ReceivedSlots {
    started_at: DateTime<Utc>,
    /// The protocols watched whole; a protocol watched pool by pool is not watched.
    protocols: Arc<[Protocol]>,
    span: Option<ReceivedSpan>,
    /// The lowest slot this session may count: its first block-meta's, plus [`HELD_BACK_SLOTS`].
    /// ⚠️ A live subscription can join mid-slot, after some of that slot's transactions were sent.
    counted_from: Option<u64>,
    /// Timed block-metas not settled yet: a cut would lose them.
    not_yet: BTreeMap<u64, DateTime<Utc>>,
    ranges: RangesToRecord,
}

impl ReceivedSlots {
    pub(in crate::infra::grpc) fn new(protocols: Arc<[Protocol]>, ranges: RangesToRecord) -> Self {
        Self {
            started_at: Utc::now(),
            protocols,
            span: None,
            counted_from: None,
            not_yet: BTreeMap::new(),
            ranges,
        }
    }

    /// A block-meta closed `slot` at `at`; the slots below `settled_below` are settled now.
    /// ⚠️ Only those count: one at or past it may still lose transactions to a cut, and an
    /// unwatched range declared too narrow is not allowed.
    pub(in crate::infra::grpc) fn on_block_meta(
        &mut self,
        slot: u64,
        at: DateTime<Utc>,
        settled_below: Option<u64>,
    ) {
        if self.protocols.is_empty() {
            return;
        }
        let counted_from = *self
            .counted_from
            .get_or_insert(slot.saturating_add(HELD_BACK_SLOTS));
        if slot >= counted_from {
            self.not_yet.insert(slot, at);
        }
        let Some(settled_below) = settled_below else {
            return;
        };
        let still_not_yet = self.not_yet.split_off(&settled_below);
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
