//! What one session counts as received for good: the slots of its timed block-metas below its
//! resume point, left in `RangesToRecord` for the recorder.

use std::{collections::BTreeMap, sync::Arc};

use chrono::{DateTime, Utc};
use yog_core::domain::Protocol;

use crate::infra::grpc::ranges_to_record::{RangeToRecord, RangesToRecord};

use super::received_span::ReceivedSpan;

/// What one session has received for good.
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
    /// a cut whose replay is refused, and an unwatched range declared too narrow is not allowed.
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
