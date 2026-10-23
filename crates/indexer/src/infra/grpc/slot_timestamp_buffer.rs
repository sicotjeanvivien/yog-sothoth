//! Pairs a Yellowstone transaction, which carries its slot and bank but no time, with its bank's
//! `block_time` and the `Finalized` status naming the bank kept, in any order; bounded.
//! ⚠️ The bound counts slots, not seconds: a wall clock keeps running through an outage and
//! would destroy transactions whose block-meta was coming on reconnect.

mod bank;
mod known_slots;
mod log;
mod waiting_slots;

use chrono::{DateTime, Utc};

use super::metrics::{EvictionReason, GrpcBufferMetrics, WAITING_PEAKS, WaitingPeaks};

pub(crate) use bank::Bank;
use known_slots::{KnownSlots, Settlement, SlotOutcome};
use waiting_slots::WaitingSlots;

/// How far behind the stream a slot may wait for its block-meta (≈ 100 s): a ceiling, not an estimate.
/// ⚠️ A distance from the head, not a count: a fork's slot whose meta never comes would otherwise
/// stay oldest pending for ever, and every reconnection would replay from it.
pub(crate) const MAX_PENDING_SLOTS: u64 = 256;

/// How many payloads may be held across all pending slots: bounding slots does not bound memory.
/// ⚠️ A count, not bytes: at ~5–20 KB per update, the bound is ~40–160 MB resident.
pub(crate) const MAX_PENDING_PAYLOADS: usize = 8_192;

/// How many settled slots are remembered, for payloads that arrive **after** their block-meta and status.
pub(crate) const MAX_KNOWN_SLOTS: usize = 256;

/// A payload and the block time that was found for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Resolved<T> {
    pub(crate) payload: T,
    pub(crate) at: DateTime<Utc>,
}

/// A payload waiting, with the bank it was produced in.
struct Held<T> {
    bank: Bank,
    payload: T,
}

/// Holds each payload until its slot's kept bank and that bank's instant are both known.
pub(crate) struct SlotTimestampBuffer<T> {
    waiting: WaitingSlots<Held<T>>,
    known: KnownSlots,
}

impl<T> SlotTimestampBuffer<T> {
    pub(crate) fn new() -> Self {
        Self::with_bounds(
            MAX_PENDING_SLOTS,
            MAX_PENDING_PAYLOADS,
            MAX_KNOWN_SLOTS,
            &WAITING_PEAKS,
        )
    }

    /// Build one with explicit bounds and peaks; only the tests pass other values.
    pub(crate) fn with_bounds(
        max_pending_slots: u64,
        max_pending_payloads: usize,
        max_known_slots: usize,
        peaks: &'static WaitingPeaks,
    ) -> Self {
        Self {
            waiting: WaitingSlots::new(max_pending_slots, max_pending_payloads, peaks),
            known: KnownSlots::new(max_known_slots),
        }
    }

    /// Take a payload whose slot and bank are known but whose instant may not be.
    /// ⚠️ `None` is "not resolved now", not "safely waiting": the payload may have been dropped on
    /// the spot, counted and logged.
    pub(crate) fn on_payload(&mut self, slot: u64, bank: Bank, payload: T) -> Option<Resolved<T>> {
        // A settled slot was seen when it was settled, so only a payload that
        // waits can move the head — and `push` applies the window when it does.
        match self.known.settlement(slot, bank) {
            Settlement::Settled(SlotOutcome::At(at)) => return Some(Resolved { payload, at }),
            // Its one chance to be named came empty: waiting for a second block-meta would end
            // in an eviction blamed on the window's size.
            Settlement::Settled(SlotOutcome::GivenUp) => {
                GrpcBufferMetrics::record_evicted(1, EvictionReason::Unresolvable);
                log::late_for_given_up_slot(slot);
                return None;
            }
            Settlement::Losing => {
                GrpcBufferMetrics::record_losing_bank(1);
                log::late_for_losing_bank(slot);
                return None;
            }
            Settlement::Pending => {}
        }

        self.waiting.push(slot, Held { bank, payload });
        None
    }

    /// Take a bank's instant, releasing what waited for it in arrival order once the bank is kept.
    /// ⚠️ No `Option`: a caller without an instant calls [`Self::on_slot_unresolvable`], never a substitute.
    pub(crate) fn on_block_time(
        &mut self,
        slot: u64,
        bank: Bank,
        at: DateTime<Utc>,
    ) -> Vec<Resolved<T>> {
        self.known.resolve(slot, bank, at);
        self.settle(slot)
    }

    /// Give up on a bank whose block-meta carried no `block_time`, or release another admitted bank's.
    /// ⚠️ Not left to the slot bound: it would leave labelled `slot_bound`, make the window look
    /// too small, and hold a place meanwhile.
    pub(crate) fn on_slot_unresolvable(&mut self, slot: u64, bank: Bank) -> Vec<Resolved<T>> {
        self.known.give_up(slot, bank);
        self.settle(slot)
    }

    /// The cluster finalized `bank` in `slot`: what waited in another bank is dropped, the rest
    /// leaves if its instant is known.
    pub(crate) fn on_finalized(&mut self, slot: u64, bank: Bank) -> Vec<Resolved<T>> {
        self.known.keep(slot, bank);
        self.settle(slot)
    }

    /// Apply what is known about `slot` to what waits for it.
    fn settle(&mut self, slot: u64) -> Vec<Resolved<T>> {
        let Some(kept) = self.known.kept(slot) else {
            self.waiting.observe(slot);
            return Vec::new();
        };
        self.waiting
            .drop_losing(slot, |held| !kept.admits(held.bank));

        match self.known.time(slot) {
            Some(SlotOutcome::At(at)) => self
                .waiting
                .release(slot)
                .into_iter()
                .map(|held| Resolved {
                    payload: held.payload,
                    at,
                })
                .collect(),
            Some(SlotOutcome::GivenUp) => {
                self.waiting.give_up(slot);
                Vec::new()
            }
            None => {
                self.waiting.observe(slot);
                Vec::new()
            }
        }
    }

    /// The oldest slot still waiting for an instant: its payloads die with the buffer, so a
    /// reconnection resumes there.
    pub(crate) fn oldest_pending_slot(&self) -> Option<u64> {
        self.waiting.oldest()
    }
}

#[cfg(test)]
#[path = "tests/slot_timestamp_buffer_tests.rs"]
mod tests;
