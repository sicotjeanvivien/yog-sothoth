//! Metrics emitted by the indexer worker: what it loses after a source paid for it.

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

    /// A transaction a source delivered and this worker never processed; the mirror, one stage
    /// down, of `yog_indexer_fetch_dropped_total`. Nothing re-requests it.
    pub(crate) fn record_dropped(protocol: &Protocol, reason: &'static str) {
        counter!(INGESTED_DROPPED, "protocol" => protocol.as_str(), "reason" => reason)
            .increment(1);
    }
}
