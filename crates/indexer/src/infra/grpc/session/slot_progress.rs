//! Which slots a session has finished: when a transaction can leave, and where
//! a reconnection resumes.

use chrono::DateTime;
use yellowstone_grpc_proto::prelude::{SubscribeUpdateBlockMeta, SubscribeUpdateTransaction};
use yog_core::domain::Protocol;

use crate::infra::grpc::slot_timestamp_buffer::{Resolved, SlotTimestampBuffer};

use super::log;

/// How many slots a reconnection asks for again, on top of what this session
/// did not finish.
///
/// ⚠️ A **deliberate overlap**: a block-meta closing slot *N* does not promise
/// all of *N*'s transactions have arrived, so the closed mark can be one short.
/// Re-asking is cheap — every event table's unique key skips a row that comes
/// twice — while a row that never comes leaves a hole nothing notices. Two is
/// what the reference client uses.
pub(super) const REWIND_SLOTS: u64 = 2;

/// A transaction waiting for its slot's block time.
///
/// Holds the whole update: `from_grpc` reads `slot` off it, and translating
/// before the instant is known would mean inventing one.
pub(super) struct PendingTransaction {
    pub(super) protocol: Protocol,
    pub(super) update: SubscribeUpdateTransaction,
}

/// The slots one session has seen, closed, or is still waiting on.
///
/// Dies with its session, on purpose: a replay delivers the oldest slots
/// **last**, so a buffer still holding the pre-cut backlog would evict each
/// replayed arrival on entry. What dies here is asked for again through
/// [`Self::resume_from`].
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

        // ⚠️ `block_time` is optional on the wire, and **nothing** substitutes
        // for it — not the receive time, not a neighbour's, not `created_at`.
        // It is in every event table's unique key and is the partitioning
        // column, so a plausible wrong value is one nothing will question. The
        // slot is given up instead.
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
    ///
    /// ⚠️ **The oldest slot this session did not finish**, not the highest it
    /// saw: a transaction names a slot still in flight, whose payloads die with
    /// the buffer, so resuming past it would skip exactly what the break
    /// destroyed. That is the oldest slot still pending, or else the last one a
    /// block-meta closed — rewound by [`REWIND_SLOTS`], since "closed" is not
    /// "complete".
    pub(super) fn resume_from(&self) -> Option<u64> {
        let unfinished = self
            .buffer
            .oldest_pending_slot()
            .or(self.highest_meta_slot)?;
        Some(unfinished.saturating_sub(REWIND_SLOTS))
    }
}
