//! Pairing a Yellowstone stream's two halves: what happened, and when.
//!
//! A transaction update carries its slot and no time; `block_time` comes on a
//! separate block-meta, keyed by slot, either before or after. Yet the time
//! may not be optional — it is in every event table's unique key and is the
//! partitioning column. This buffer holds whichever half came first until the
//! other shows up, and bounds that wait. It is generic over the payload, so
//! the reasoning about time is testable without a wire.
//!
//! ⚠️ **The bound counts slots, not seconds.** A wall clock keeps running while
//! the stream is down, so a time bound would empty the buffer during an outage
//! and destroy transactions whose block-meta was coming on reconnect. A slot
//! bound reads the stream: nothing arrives, nothing is evicted.
//!
//! ⚠️ **[`MAX_PENDING_SLOTS`] is a ceiling, not an estimate**: set far beyond
//! any plausible lag so that the eviction counter is a signal, until a real
//! stream measures the lag.
//!
//! This file orchestrates. `waiting_slots` holds what waits and decides what
//! goes; `known_times` remembers what is settled; `log` writes the lines.

mod known_times;
mod log;
mod waiting_slots;

use chrono::{DateTime, Utc};

use super::metrics::{EvictionReason, GrpcBufferMetrics};

use known_times::{KnownTimes, SlotOutcome};
use waiting_slots::WaitingSlots;

/// How far behind the stream a slot may still be waiting for its block-meta.
///
/// 256 slots is ≈ 100 s at Solana's ~400 ms per slot — see the module docs for
/// why this is a ceiling rather than an estimate, and what its counter is for.
///
/// ⚠️ **A distance from the head, not a population.** A count of pending slots
/// never fires in steady state (one or two are pending), so a slot whose
/// block-meta never comes — a fork, at `confirmed` — would stay the oldest
/// pending slot for ever, and every reconnection would resume from it, asking a
/// billed provider to replay hours.
pub(crate) const MAX_PENDING_SLOTS: u64 = 256;

/// How many payloads may be held across all pending slots.
///
/// ⚠️ Bounding slots does **not** bound memory: 256 slots times a burst per
/// slot is unbounded. This second limit closes that.
///
/// ⚠️ It bounds a **count, not bytes**: a whole transaction update is on the
/// order of 5–20 KB for a Meteora swap, so 8 192 of them is roughly **40–160 MB
/// resident in this buffer alone** — bounded, and a number the process has to
/// be sized for.
pub(crate) const MAX_PENDING_PAYLOADS: usize = 8_192;

/// How many settled slots are remembered, for payloads that arrive **after**
/// their block-meta — see `KnownTimes` for why this table needs a bound too.
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
        Self::with_bounds(MAX_PENDING_SLOTS, MAX_PENDING_PAYLOADS, MAX_KNOWN_SLOTS)
    }

    /// Build one with explicit bounds — only the tests pass anything but the
    /// constants, to reach a bound in three slots rather than 257.
    pub(crate) fn with_bounds(
        max_pending_slots: u64,
        max_pending_payloads: usize,
        max_known_slots: usize,
    ) -> Self {
        Self {
            waiting: WaitingSlots::new(max_pending_slots, max_pending_payloads),
            known: KnownTimes::new(max_known_slots),
        }
    }

    /// Take a payload whose slot is known but whose instant may not be.
    ///
    /// ⚠️ **`None` means "not resolved now", nothing more**: usually waiting,
    /// but possibly dropped on the spot, since holding it can push a bound over
    /// and it can be the one evicted. Either way it is counted and logged; what
    /// the caller must not do is read `None` as "safely waiting".
    pub(crate) fn on_payload(&mut self, slot: u64, payload: T) -> Option<Resolved<T>> {
        // A settled slot was seen when it was settled, so only a payload that
        // waits can move the head — and `push` applies the window when it does.
        match self.known.outcome(slot) {
            Some(SlotOutcome::At(at)) => return Some(Resolved { payload, at }),
            // The slot's one chance to be named came and went empty: waiting
            // for a second block-meta would spend a place in the window and end
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

    /// Take a slot's instant, releasing what was waiting for it in the order it
    /// arrived.
    ///
    /// ⚠️ **`block_time` is optional on the wire, and this takes no `Option`**:
    /// a caller with no instant has nothing to call this with, and must not
    /// substitute a default, a receive time or a neighbour's — any of those
    /// writes a wrong value into the partitioning column. It calls
    /// [`Self::on_slot_unresolvable`] instead.
    pub(crate) fn on_block_time(&mut self, slot: u64, at: DateTime<Utc>) -> Vec<Resolved<T>> {
        let released = self.waiting.release(slot);
        self.known.resolve(slot, at);

        released
            .into_iter()
            .map(|payload| Resolved { payload, at })
            .collect()
    }

    /// Give up on a slot: nothing will ever name its instant.
    ///
    /// ⚠️ **Why not let the bound handle it.** Both roads end in the same
    /// eviction, but under different labels, and the label is what a
    /// real-stream measurement reads to size the window. Left to the slot
    /// bound, such a slot waits out the whole window, leaves labelled
    /// `slot_bound`, and makes the window look too small; meanwhile it occupies
    /// a place that slots which *would* resolve need.
    ///
    /// Called for a block-meta whose `block_time` is `None` — and the entry
    /// point for a slot known abandoned by a fork, should the listener ever
    /// learn of one. A slot that is not pending is only remembered as given up,
    /// and counts nothing.
    pub(crate) fn on_slot_unresolvable(&mut self, slot: u64) {
        self.waiting.give_up(slot);
        self.known.give_up(slot);
    }

    /// The oldest slot still waiting for an instant, if any: these payloads die
    /// with the buffer, so this is the oldest slot the connection did not
    /// finish — where a reconnection resumes.
    pub(crate) fn oldest_pending_slot(&self) -> Option<u64> {
        self.waiting.oldest()
    }
}

#[cfg(test)]
#[path = "tests/slot_timestamp_buffer_tests.rs"]
mod tests;
