//! Pairs a Yellowstone stream's two halves: a transaction carries its slot and no time, a
//! block-meta carries the slot's `block_time`, before or after. Holds whichever came first, bounded.
//! ⚠️ The bound counts slots, not seconds: a wall clock keeps running through an outage and
//! would destroy transactions whose block-meta was coming on reconnect.

mod known_times;
mod log;
mod waiting_slots;

use chrono::{DateTime, Utc};

use super::metrics::{EvictionReason, GrpcBufferMetrics, WAITING_PEAKS, WaitingPeaks};

use known_times::{KnownTimes, SlotOutcome};
use waiting_slots::WaitingSlots;

/// How far behind the stream a slot may wait for its block-meta (≈ 100 s): a ceiling, not an estimate.
/// ⚠️ A distance from the head, not a count: a fork's slot whose meta never comes would otherwise
/// stay oldest pending for ever, and every reconnection would replay from it.
pub(crate) const MAX_PENDING_SLOTS: u64 = 256;

/// How many payloads may be held across all pending slots: bounding slots does not bound memory.
/// ⚠️ A count, not bytes: at ~5–20 KB per update, the bound is ~40–160 MB resident.
pub(crate) const MAX_PENDING_PAYLOADS: usize = 8_192;

/// How many settled slots are remembered, for payloads that arrive **after** their block-meta.
pub(crate) const MAX_KNOWN_SLOTS: usize = 256;

/// A payload and the block time that was found for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Resolved<T> {
    pub(crate) payload: T,
    pub(crate) at: DateTime<Utc>,
}

/// Holds one half of the stream until the other half names its instant.
pub(crate) struct SlotTimestampBuffer<T> {
    waiting: WaitingSlots<T>,
    known: KnownTimes,
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
            known: KnownTimes::new(max_known_slots),
        }
    }

    /// Take a payload whose slot is known but whose instant may not be.
    /// ⚠️ `None` is "not resolved now", not "safely waiting": the payload may have been evicted on
    /// the spot, counted and logged.
    pub(crate) fn on_payload(&mut self, slot: u64, payload: T) -> Option<Resolved<T>> {
        // A settled slot was seen when it was settled, so only a payload that
        // waits can move the head — and `push` applies the window when it does.
        match self.known.outcome(slot) {
            Some(SlotOutcome::At(at)) => return Some(Resolved { payload, at }),
            // Its one chance to be named came empty: waiting for a second block-meta would end
            // in an eviction blamed on the window's size.
            Some(SlotOutcome::GivenUp) => {
                GrpcBufferMetrics::record_evicted(1, EvictionReason::Unresolvable);
                log::late_for_given_up_slot(slot);
                return None;
            }
            None => {}
        }

        self.waiting.push(slot, payload);
        None
    }

    /// Take a slot's instant, releasing what waited for it in arrival order.
    /// ⚠️ No `Option`: a caller without an instant calls [`Self::on_slot_unresolvable`], never a substitute.
    pub(crate) fn on_block_time(&mut self, slot: u64, at: DateTime<Utc>) -> Vec<Resolved<T>> {
        let released = self.waiting.release(slot);
        self.known.resolve(slot, at);

        released
            .into_iter()
            .map(|payload| Resolved { payload, at })
            .collect()
    }

    /// Give up on a slot whose block-meta carried no `block_time`; one not pending is only remembered.
    /// ⚠️ Not left to the slot bound: it would leave labelled `slot_bound`, make the window look
    /// too small, and hold a place meanwhile.
    pub(crate) fn on_slot_unresolvable(&mut self, slot: u64) {
        self.waiting.give_up(slot);
        self.known.give_up(slot);
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
