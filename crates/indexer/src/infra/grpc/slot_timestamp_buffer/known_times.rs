//! What is already settled about a slot, for the payload that arrives after
//! its block-meta.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

/// What a block-meta settled about its slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SlotOutcome {
    /// The block-meta named the slot's instant.
    At(DateTime<Utc>),
    /// The block-meta came empty: nothing will ever name the instant.
    GivenUp,
}

/// The settled slots, bounded.
///
/// ⚠️ **Given-up slots live in the same table as resolved ones**, and that is
/// what keeps the reverse order honest both ways: a payload arriving after its
/// slot's empty block-meta meets the answer at once, instead of waiting out the
/// window and leaving labelled `slot_bound`. One table also means one bound.
///
/// ⚠️ **The table nobody thinks to bound.** Without it the reverse order loses
/// everything; without a bound on it, memory grows for as long as the process
/// runs. Trimming it needs no counter: a forgotten slot only costs something if
/// a payload for it turns up later, and *that* loss is counted where it happens.
///
/// ⚠️ **On a replay**, a block time for an older slot arriving into a full
/// table is inserted and trimmed in the same call. The payloads already waiting
/// for it are released first — the buffer drains before it trims — so only a
/// *later* payload for that slot loses its answer.
pub(super) struct KnownTimes {
    outcomes: BTreeMap<u64, SlotOutcome>,
    max_known_slots: usize,
}

impl KnownTimes {
    pub(super) fn new(max_known_slots: usize) -> Self {
        Self {
            outcomes: BTreeMap::new(),
            max_known_slots,
        }
    }

    /// What is settled about `slot`, if anything.
    pub(super) fn outcome(&self, slot: u64) -> Option<SlotOutcome> {
        self.outcomes.get(&slot).copied()
    }

    /// The slot's instant is known.
    pub(super) fn resolve(&mut self, slot: u64, at: DateTime<Utc>) {
        self.outcomes.insert(slot, SlotOutcome::At(at));
        self.trim();
    }

    /// The slot's instant will never be known.
    ///
    /// ⚠️ **Never over an instant already found.** A second block-meta for the
    /// same slot can come empty — a duplicate, a re-emission after a fork — and
    /// overwriting `At` would throw a known timestamp away, then drop every
    /// late payload for the slot. The other direction, `GivenUp` → `At`, is
    /// what a real correction looks like, and [`Self::resolve`] allows it.
    pub(super) fn give_up(&mut self, slot: u64) {
        self.outcomes.entry(slot).or_insert(SlotOutcome::GivenUp);
        self.trim();
    }

    /// How many slots are remembered.
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.outcomes.len()
    }

    /// Forget the oldest slots once too many are remembered.
    fn trim(&mut self) {
        while self.outcomes.len() > self.max_known_slots {
            self.outcomes.pop_first();
        }
    }
}
