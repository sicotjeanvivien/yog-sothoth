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

    /// Work this stage discarded without handing it on.
    ///
    /// ⚠️ **Separate from a fetch failure, and it has to be.** These are not
    /// requests that went wrong; they are results thrown away because the
    /// process is stopping or the consumer is gone. Left uncounted they would
    /// be the one loss in this stage with no trace at all — the crate's rule is
    /// *counted* and stepped over, not merely stepped over.
    ///
    /// ⚠️ **The `reason` label separates two different losses, and summing the
    /// family conflates them.** `shutdown` and `downstream_closed` are
    /// transactions the RPC answered: the request was made and the quota spent,
    /// and only the result is wasted. `shutdown_before_fetch` is a signature
    /// dropped while waiting for a permit — nothing was requested and nothing
    /// was billed. Reading the total as "quota wasted" over-counts by exactly
    /// the queued backlog at shutdown; that reading needs the label.
    pub(crate) fn record_dropped(protocol: &Protocol, reason: &'static str) {
        counter!(FETCH_DROPPED, "protocol" => protocol.as_str(), "reason" => reason).increment(1);
    }
}
