//! The lines the slot/time buffer writes, one function each.

use tracing::warn;

use crate::infra::grpc::metrics::EvictionReason;

/// A pending slot was evicted: its payloads can never be timestamped.
pub(super) fn evicted(slot: u64, payloads: usize, reason: EvictionReason) {
    warn!(
        slot,
        payloads,
        bound = reason.as_str(),
        "evicting a slot that never received its block time — its \
         payloads cannot be timestamped, so they are dropped"
    );
}

/// A payload arrived for a slot whose block-meta came empty.
pub(super) fn late_for_given_up_slot(slot: u64) {
    warn!(
        slot,
        "a payload arrived for a slot already given up — its block \
         time will never come, so it is dropped"
    );
}
