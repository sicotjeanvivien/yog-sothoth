//! Which slots a session has finished: when a transaction can leave, and where
//! a reconnection resumes.

use chrono::DateTime;
use yellowstone_grpc_proto::prelude::{
    SlotStatus, SubscribeUpdateBlockMeta, SubscribeUpdateSlot, SubscribeUpdateTransaction,
};
use yog_core::domain::Protocol;

use crate::infra::grpc::{
    metrics::GrpcListenerMetrics,
    slot_timestamp_buffer::{Bank, Resolved, SlotTimestampBuffer},
};

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
    /// What this session has reported of its server's bank ids; see [`Self::bank`].
    bank_ids: Option<BankIds>,
}

/// Whether the server names banks, as the session last reported it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BankIds {
    Named,
    Missing,
}

impl SlotProgress {
    pub(super) fn new() -> Self {
        Self {
            buffer: SlotTimestampBuffer::new(),
            highest_meta_slot: None,
            bank_ids: None,
        }
    }

    /// A routable transaction arrived: it leaves now if its bank is kept and
    /// timed, and waits otherwise.
    pub(super) fn on_transaction(
        &mut self,
        pending: PendingTransaction,
    ) -> Option<Resolved<PendingTransaction>> {
        // Waiting for its status or block-meta, or dropped on the way in —
        // `on_payload` counts and logs that case itself.
        let bank = self.bank(pending.update.slot, Bank::from_wire(pending.update.bank_id));
        self.buffer.on_payload(pending.update.slot, bank, pending)
    }

    /// A block-meta closed its slot: the transactions it releases.
    pub(super) fn on_block_meta(
        &mut self,
        meta: SubscribeUpdateBlockMeta,
    ) -> Vec<Resolved<PendingTransaction>> {
        let slot = meta.slot;
        let bank = self.bank(slot, Bank::from_wire(meta.bank_id));
        self.highest_meta_slot = Some(self.highest_meta_slot.map_or(slot, |seen| seen.max(slot)));

        // ⚠️ Nothing substitutes for a missing `block_time`: in every unique key and the partitioning
        // column, a plausible wrong value is one nothing will question. The bank is given up.
        let Some(at) = meta
            .block_time
            .and_then(|time| DateTime::from_timestamp(time.timestamp, 0))
        else {
            log::no_block_time(slot);
            return self.buffer.on_slot_unresolvable(slot, bank);
        };

        self.buffer.on_block_time(slot, bank, at)
    }

    /// A slot status came: a `Finalized` one names the bank kept, and what it releases.
    /// ⚠️ Any other level is stepped over: settling on a `Confirmed` bank would re-open the window
    /// `finalized` closes. The request asks for `Finalized` only; a server may not comply.
    pub(super) fn on_slot_status(
        &mut self,
        status: SubscribeUpdateSlot,
    ) -> Vec<Resolved<PendingTransaction>> {
        if status.status() != SlotStatus::SlotFinalized {
            return Vec::new();
        }
        let bank = self.bank(status.slot, Bank::from_status(status.bank_id));
        self.buffer.on_finalized(status.slot, bank)
    }

    /// Pass a message's bank through, reporting once per change whether the server names banks.
    /// ⚠️ `Missing` is sticky for the session: one bank-less message is a slot paired by slot alone.
    fn bank(&mut self, slot: u64, bank: Bank) -> Bank {
        let seen = match bank {
            Bank::Id(_) => BankIds::Named,
            Bank::Unknown => BankIds::Missing,
        };
        let changed = match self.bank_ids {
            None => true,
            Some(BankIds::Named) => seen == BankIds::Missing,
            Some(BankIds::Missing) => false,
        };
        if changed {
            self.bank_ids = Some(seen);
            GrpcListenerMetrics::record_bank_id_missing(seen == BankIds::Missing);
            if seen == BankIds::Missing {
                log::no_bank_id(slot);
            }
        }
        bank
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
