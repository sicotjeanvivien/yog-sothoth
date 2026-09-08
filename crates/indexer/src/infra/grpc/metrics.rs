//! Counters for the gRPC ingestion path.
//!
//! Same shape as `infra/rpc/dispatcher/metrics.rs`: names declared as
//! constants, descriptions registered once at startup, increments behind named
//! methods so a call site never spells a metric name.

use metrics::{counter, describe_counter};

/// Payloads dropped because their slot never received a block time.
///
/// ⚠️ **This is the number the next slice goes looking for.** The buffer's
/// bounds are ceilings picked without a measurement — see
/// `slot_timestamp_buffer`'s module docs — so this counter staying at zero is
/// what says the guess was generous, and any movement is what says it was not.
/// `02 - backlog/pre-v02/flux-grpc-reel-mesures.md` replaces the guess by
/// reading it.
const PAYLOADS_EVICTED: &str = "yog_indexer_grpc_untimestamped_payloads_total";

/// Which bound forced the eviction, on every increment of [`PAYLOADS_EVICTED`].
///
/// ⚠️ **Not decoration — without it the counter misleads the ticket that reads
/// it.** Two very different evictions share this metric: a slot that waited
/// past `MAX_PENDING_SLOTS`, and a burst that hit `MAX_PENDING_PAYLOADS` while
/// the slot may have been one message from resolving. The two ceilings cross at
/// **32 payloads per slot** (8 192 / 256), so above that rate the payload bound
/// is the binding one: at ~100 payloads per slot it fires at ~81 slots, not the
/// 256 the docs advertise as the wait window. An unlabelled counter would be
/// read as "the slot ceiling was too small", and raising it would change
/// nothing. Found in review, 8 September 2026.
#[derive(Debug, Clone, Copy)]
pub(crate) enum EvictionReason {
    /// Too many slots waiting: this one is the oldest.
    SlotBound,
    /// Too many payloads held across all pending slots.
    PayloadBound,
}

impl EvictionReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SlotBound => "slot_bound",
            Self::PayloadBound => "payload_bound",
        }
    }
}

pub struct GrpcBufferMetrics;

impl GrpcBufferMetrics {
    /// Call once at startup to register the descriptions with the Prometheus
    /// exporter.
    pub(crate) fn register_descriptions() {
        describe_counter!(
            PAYLOADS_EVICTED,
            "Payloads dropped because their slot never received a block time, \
             labelled by the bound that forced it"
        );
    }

    pub(crate) fn record_evicted(count: usize, reason: EvictionReason) {
        counter!(PAYLOADS_EVICTED, "reason" => reason.as_str()).increment(count as u64);
    }
}
