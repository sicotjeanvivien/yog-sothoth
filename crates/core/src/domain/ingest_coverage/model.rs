//! What one ingestion session observed: a range of slots closed by the stream, each end with the
//! chain's own time. A gap is the complement of these ranges, read in SQL, never written.

use chrono::{DateTime, Utc};

use crate::domain::Protocol;

/// The lowest and highest slots a session saw closed, with their block times.
/// ⚠️ Lowest and highest, not first and last: a replay delivers its oldest slots last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotRange {
    from: u64,
    from_at: DateTime<Utc>,
    to: u64,
    to_at: DateTime<Utc>,
}

impl SlotRange {
    /// A range holding one slot.
    pub fn starting(slot: u64, at: DateTime<Utc>) -> Self {
        Self {
            from: slot,
            from_at: at,
            to: slot,
            to_at: at,
        }
    }

    /// Widen the range to `slot`; a slot inside it changes nothing, whatever the slots skipped.
    pub fn include(&mut self, slot: u64, at: DateTime<Utc>) {
        if slot < self.from {
            self.from = slot;
            self.from_at = at;
        }
        if slot > self.to {
            self.to = slot;
            self.to_at = at;
        }
    }

    pub fn from(&self) -> u64 {
        self.from
    }

    pub fn from_at(&self) -> DateTime<Utc> {
        self.from_at
    }

    pub fn to(&self) -> u64 {
        self.to
    }

    pub fn to_at(&self) -> DateTime<Utc> {
        self.to_at
    }
}

/// One protocol's range in one session; the session's start is its key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedSlots {
    pub protocol: Protocol,
    pub session_started_at: DateTime<Utc>,
    pub range: SlotRange,
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
