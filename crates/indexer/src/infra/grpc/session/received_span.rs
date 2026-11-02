//! A span of slots received for good: its first and last slot, each with its block time.

use chrono::{DateTime, Utc};

/// First and last slot received for good, with their block times.
/// ⚠️ Lowest and highest, not first and last to arrive: a replay delivers its oldest slots last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::infra::grpc) struct ReceivedSpan {
    first: u64,
    first_at: DateTime<Utc>,
    last: u64,
    last_at: DateTime<Utc>,
}

impl ReceivedSpan {
    pub(in crate::infra::grpc) fn starting(slot: u64, at: DateTime<Utc>) -> Self {
        Self {
            first: slot,
            first_at: at,
            last: slot,
            last_at: at,
        }
    }

    /// Widen to `slot`. A slot already in the span, either end included, changes nothing,
    /// whatever the slots the network skipped.
    pub(in crate::infra::grpc) fn include(&mut self, slot: u64, at: DateTime<Utc>) {
        if (self.first..=self.last).contains(&slot) {
            return;
        }
        if slot < self.first {
            self.first = slot;
            self.first_at = at;
        } else {
            self.last = slot;
            self.last_at = at;
        }
    }

    pub(in crate::infra::grpc) fn first(&self) -> (u64, DateTime<Utc>) {
        (self.first, self.first_at)
    }

    pub(in crate::infra::grpc) fn last(&self) -> (u64, DateTime<Utc>) {
        (self.last, self.last_at)
    }
}

#[cfg(test)]
#[path = "../tests/received_span_tests.rs"]
mod tests;
