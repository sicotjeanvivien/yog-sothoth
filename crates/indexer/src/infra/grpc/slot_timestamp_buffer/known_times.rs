//! What is already settled about a slot, for the payload that arrives after its block-meta.

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

/// The settled slots, given up or resolved, in one bounded table.
/// ⚠️ Given-up slots are kept too: a payload arriving after an empty block-meta meets the answer
/// at once instead of waiting out the window and leaving labelled `slot_bound`.
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
    /// ⚠️ Never over an instant already found: a duplicate empty block-meta would throw it away.
    /// `GivenUp` → `At` is a real correction, which [`Self::resolve`] allows.
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
