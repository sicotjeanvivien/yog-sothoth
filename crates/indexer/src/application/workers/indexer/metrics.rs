//! Metrics emitted by the indexer worker.
//!
//! One family, and it exists because the worker can lose work that a source
//! already paid for. Everything else the worker does is measured a stage below,
//! by `TransactionProcessorMetrics`.

use metrics::{counter, describe_counter};
use yog_core::domain::Protocol;

const INGESTED_DROPPED: &str = "yog_indexer_ingested_dropped_total";

pub(crate) struct IndexerWorkerMetrics;

impl IndexerWorkerMetrics {
    pub(crate) fn register_descriptions() {
        describe_counter!(
            INGESTED_DROPPED,
            "Delivered transactions the worker discarded without processing \
             (label: reason)"
        );
    }

    /// A transaction a source delivered and this worker never processed.
    ///
    /// ⚠️ **The mirror of `yog_indexer_fetch_dropped_total`, one stage down**,
    /// and it was missing until 10 September 2026 — the producer counted its
    /// shutdown losses carefully while the consumer of the same channel dropped
    /// up to a thousand more without a trace. The same rule applied to one site
    /// in two, which is this repository's most frequent defect and was worth
    /// one more file to stop repeating.
    ///
    /// On the JSON-RPC path these are transactions that cost a request; on the
    /// gRPC path they cost bandwidth. Either way they are gone: nothing
    /// re-requests them, and the next start resumes from wherever its source
    /// resumes, not from here.
    pub(crate) fn record_dropped(protocol: &Protocol, reason: &'static str) {
        counter!(INGESTED_DROPPED, "protocol" => protocol.as_str(), "reason" => reason)
            .increment(1);
    }
}
