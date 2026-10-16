//! Metrics emitted by the JSON-RPC fetch stage; the gRPC path fetches nothing.
//! ⚠️ The family names are what dashboards query: moving the emitting type must not rename them.

use metrics::{counter, describe_counter, describe_histogram, histogram};
use yog_core::domain::Protocol;

const FETCH_FAILURES: &str = "yog_indexer_fetch_failures_total";
const FETCH_NOT_FOUND: &str = "yog_indexer_fetch_not_found_total";
const FETCH_DROPPED: &str = "yog_indexer_fetch_dropped_total";

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
        describe_counter!(
            FETCH_DROPPED,
            "Work the fetch stage discarded without reaching the consumer              (label: reason — `shutdown` and `downstream_closed` cost a request,              `shutdown_before_fetch` did not)"
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

    /// Work this stage discarded without handing it on, apart from fetch failures.
    /// ⚠️ `shutdown` and `downstream_closed` spent a request, `shutdown_before_fetch` did not.
    pub(crate) fn record_dropped(protocol: &Protocol, reason: &'static str) {
        counter!(FETCH_DROPPED, "protocol" => protocol.as_str(), "reason" => reason).increment(1);
    }
}
