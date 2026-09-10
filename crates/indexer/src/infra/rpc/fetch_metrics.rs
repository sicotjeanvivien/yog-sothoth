//! Metrics emitted by the JSON-RPC fetch stage.
//!
//! ⚠️ **The three family names are unchanged, deliberately.** They were emitted
//! by `TransactionProcessorMetrics` until the fetch moved into the source that
//! needs one; the type that emits them is an implementation detail, the name a
//! Prometheus query is written against is not. A rename here would break
//! dashboards to record a refactor.
//!
//! They live on this path only. There is nothing to fetch on the gRPC path, so
//! a `yog_indexer_fetch_*` series that stops advancing after a switch of
//! `INGEST_SOURCE` says exactly what happened.

use metrics::{counter, describe_counter, describe_histogram, histogram};
use yog_core::domain::Protocol;

const FETCH_FAILURES: &str = "yog_indexer_fetch_failures_total";
const FETCH_NOT_FOUND: &str = "yog_indexer_fetch_not_found_total";

const FETCH_DURATION: &str = "yog_indexer_fetch_duration_seconds";

pub(crate) struct FetchMetrics;

impl FetchMetrics {
    pub(crate) fn register_descriptions() {
        describe_counter!(
            FETCH_FAILURES,
            "Failures fetching a transaction from the RPC (label: reason)"
        );
        describe_counter!(
            FETCH_NOT_FOUND,
            "Transactions not found by the RPC after all retries"
        );
        describe_histogram!(
            FETCH_DURATION,
            "Duration of fetch_transaction in seconds (includes retries)"
        );
    }

    pub(crate) fn record_failure(protocol: &Protocol, reason: &'static str) {
        counter!(FETCH_FAILURES, "protocol" => protocol.as_str(), "reason" => reason).increment(1);
    }

    pub(crate) fn record_not_found(protocol: &Protocol) {
        counter!(FETCH_NOT_FOUND, "protocol" => protocol.as_str()).increment(1);
    }

    pub(crate) fn record_duration(protocol: &Protocol, seconds: f64) {
        histogram!(FETCH_DURATION, "protocol" => protocol.as_str()).record(seconds);
    }
}
