//! Which slots a session has finished: when a transaction can leave, and where
//! a reconnection resumes.

use chrono::DateTime;
use yellowstone_grpc_proto::prelude::{SubscribeUpdateBlockMeta, SubscribeUpdateTransaction};
use yog_core::domain::Protocol;

use crate::infra::grpc::slot_timestamp_buffer::{Resolved, SlotTimestampBuffer};

use super::log;

/// How many slots a reconnection asks for again, on top of what this session did not finish.
/// ⚠️ A deliberate overlap: a block-meta closing slot N does not promise all of N's transactions
/// arrived; a row asked twice is skipped by the unique keys, a row never asked is a silent hole.
pub(super) const REWIND_SLOTS: u64 = 2;

/// A transaction waiting for its slot's block time, untranslated: `from_grpc` needs the instant.
pub(super) struct PendingTransaction {
    pub(super) protocol: Protocol,
    pub(super) update: SubscribeUpdateTransaction,
}

/// The slots one session has seen, closed, or is still waiting on.
/// ⚠️ Dies with its session: a replay delivers the oldest slots last, so a buffer holding the
/// pre-cut backlog would evict each replayed arrival; [`Self::resume_from`] asks for them again.
pub(super) struct SlotProgress {
    buffer: SlotTimestampBuffer<PendingTransaction>,
    /// The highest slot a block-meta closed. Advanced by block-metas alone: a
    /// transaction names a slot still in flight.
    highest_meta_slot: Option<u64>,
}

impl SlotProgress {
    pub(super) fn new() -> Self {
        Self {
            buffer: SlotTimestampBuffer::new(),
            highest_meta_slot: None,
        }
    }

    /// A routable transaction arrived: it leaves now if its slot's time is
    /// known, and waits otherwise.
    pub(super) fn on_transaction(
        &mut self,
        pending: PendingTransaction,
    ) -> Option<Resolved<PendingTransaction>> {
        // Waiting for its block-meta, or dropped by a bound on the way in —
        // `on_payload` counts and logs that case itself.
        self.buffer.on_payload(pending.update.slot, pending)
    }

    /// A block-meta closed its slot: the transactions it releases.
    pub(super) fn on_block_meta(
        &mut self,
        meta: SubscribeUpdateBlockMeta,
    ) -> Vec<Resolved<PendingTransaction>> {
        let slot = meta.slot;
        self.highest_meta_slot = Some(self.highest_meta_slot.map_or(slot, |seen| seen.max(slot)));

        // ⚠️ Nothing substitutes for a missing `block_time`: in every unique key and the partitioning
        // column, a plausible wrong value is one nothing will question. The slot is given up.
        let Some(at) = meta
            .block_time
            .and_then(|time| DateTime::from_timestamp(time.timestamp, 0))
        else {
            log::no_block_time(slot);
            self.buffer.on_slot_unresolvable(slot);
            return Vec::new();
        };

        self.buffer.on_block_time(slot, at)
    }

    /// Where a reconnection should resume from, or `None` when nothing arrived.
    /// ⚠️ The oldest slot not finished (oldest pending, else the last closed), not the highest
    /// seen, rewound by [`REWIND_SLOTS`]: resuming past it would skip what the break destroyed.
    pub(super) fn resume_from(&self) -> Option<u64> {
        let unfinished = self
            .buffer
            .oldest_pending_slot()
            .or(self.highest_meta_slot)?;
        Some(unfinished.saturating_sub(REWIND_SLOTS))
    }
}
