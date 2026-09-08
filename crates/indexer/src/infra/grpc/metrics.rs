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

pub struct GrpcBufferMetrics;

impl GrpcBufferMetrics {
    /// Call once at startup to register the descriptions with the Prometheus
    /// exporter.
    pub(crate) fn register_descriptions() {
        describe_counter!(
            PAYLOADS_EVICTED,
            "Payloads dropped because their slot never received a block time"
        );
    }

    pub(crate) fn record_evicted(count: usize) {
        counter!(PAYLOADS_EVICTED).increment(count as u64);
    }
}
