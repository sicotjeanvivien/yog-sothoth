//! What is already known about a slot: each bank's instant, and the bank the cluster kept.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

use super::bank::Bank;

/// What a block-meta settled about its bank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SlotOutcome {
    /// The block-meta named the bank's instant.
    At(DateTime<Utc>),
    /// The block-meta came empty: nothing will ever name the instant.
    GivenUp,
}

/// What a payload of a given bank meets in the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Settlement {
    /// The kept bank, or its instant, is not known yet.
    Pending,
    /// Another bank of the slot was kept.
    Losing,
    /// The kept bank's instant.
    Settled(SlotOutcome),
}

/// One slot's facts.
#[derive(Default)]
struct Facts {
    /// One entry per bank a block-meta named; two at most in practice.
    times: Vec<(Bank, SlotOutcome)>,
    /// The bank a `Finalized` status named.
    kept: Option<Bank>,
}

impl Facts {
    /// The kept bank's instant: a found one before a given-up one.
    fn time(&self, kept: Bank) -> Option<SlotOutcome> {
        let admitted = || {
            self.times
                .iter()
                .filter(move |(bank, _)| kept.admits(*bank))
                .map(|(_, outcome)| *outcome)
        };
        admitted()
            .find(|outcome| matches!(outcome, SlotOutcome::At(_)))
            .or_else(|| admitted().next())
    }
}

/// The slots' facts, in one bounded table.
/// ⚠️ Given-up banks are kept too: a payload arriving after an empty block-meta meets the answer
/// at once instead of waiting out the window and leaving labelled `slot_bound`.
pub(super) struct KnownSlots {
    facts: BTreeMap<u64, Facts>,
    max_known_slots: usize,
}

impl KnownSlots {
    pub(super) fn new(max_known_slots: usize) -> Self {
        Self {
            facts: BTreeMap::new(),
            max_known_slots,
        }
    }

    /// What a payload of `bank` in `slot` meets.
    pub(super) fn settlement(&self, slot: u64, bank: Bank) -> Settlement {
        let Some(facts) = self.facts.get(&slot) else {
            return Settlement::Pending;
        };
        let Some(kept) = facts.kept else {
            return Settlement::Pending;
        };
        if !kept.admits(bank) {
            return Settlement::Losing;
        }
        facts
            .time(kept)
            .map_or(Settlement::Pending, Settlement::Settled)
    }

    /// The bank a `Finalized` status named for `slot`, if one came.
    pub(super) fn kept(&self, slot: u64) -> Option<Bank> {
        self.facts.get(&slot).and_then(|facts| facts.kept)
    }

    /// The kept bank's instant, once both are known.
    pub(super) fn time(&self, slot: u64) -> Option<SlotOutcome> {
        let facts = self.facts.get(&slot)?;
        facts.time(facts.kept?)
    }

    /// A bank's instant is known.
    pub(super) fn resolve(&mut self, slot: u64, bank: Bank, at: DateTime<Utc>) {
        let times = &mut self.facts.entry(slot).or_default().times;
        match times.iter_mut().find(|(known, _)| *known == bank) {
            Some((_, outcome)) => *outcome = SlotOutcome::At(at),
            None => times.push((bank, SlotOutcome::At(at))),
        }
        self.trim();
    }

    /// A bank's instant will never be known.
    /// ⚠️ Never over an instant already found: a duplicate empty block-meta would throw it away.
    /// `GivenUp` → `At` is a real correction, which [`Self::resolve`] allows.
    pub(super) fn give_up(&mut self, slot: u64, bank: Bank) {
        let times = &mut self.facts.entry(slot).or_default().times;
        if !times.iter().any(|(known, _)| *known == bank) {
            times.push((bank, SlotOutcome::GivenUp));
        }
        self.trim();
    }

    /// The cluster kept `bank`; a second status for the slot changes nothing.
    pub(super) fn keep(&mut self, slot: u64, bank: Bank) {
        self.facts.entry(slot).or_default().kept.get_or_insert(bank);
        self.trim();
    }

    /// How many slots are remembered.
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.facts.len()
    }

    /// Forget the oldest slots once too many are remembered.
    fn trim(&mut self) {
        while self.facts.len() > self.max_known_slots {
            self.facts.pop_first();
        }
    }
}
