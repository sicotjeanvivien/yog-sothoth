//! Metrics emitted by the metadata worker.

use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};

const TICK_TOTAL: &str = "yog_context_metadata_tick_total";
const TICK_DURATION: &str = "yog_context_metadata_tick_duration_seconds";
const MISSING_MINTS: &str = "yog_context_metadata_missing_mints";
const UPSERT_TOTAL: &str = "yog_context_metadata_upsert_total";

pub(crate) struct MetadataWorkerMetrics;

impl MetadataWorkerMetrics {
    /// Register once at startup, from `Daemon::connect`.
    pub(crate) fn register_descriptions() {
        describe_counter!(
            TICK_TOTAL,
            "Metadata worker ticks completed (labels: outcome=ok|no_work|list_failed|source_hard_error, \
             scope=recent_pools|all_pools)"
        );
        describe_histogram!(
            TICK_DURATION,
            "Total duration of a metadata worker tick in seconds (labels: outcome, scope)"
        );
        describe_gauge!(
            MISSING_MINTS,
            "Number of mints missing metadata at the last tick of each scope (label: scope)"
        );
        describe_counter!(
            UPSERT_TOTAL,
            "Token metadata upserts attempted (label: outcome=ok|error)"
        );
    }

    pub(crate) fn record_tick(outcome: &'static str, scope: &'static str, seconds: f64) {
        counter!(TICK_TOTAL, "outcome" => outcome, "scope" => scope).increment(1);
        histogram!(TICK_DURATION, "outcome" => outcome, "scope" => scope).record(seconds);
    }

    pub(crate) fn set_missing_mints(scope: &'static str, count: usize) {
        gauge!(MISSING_MINTS, "scope" => scope).set(count as f64);
    }

    pub(crate) fn record_upsert(outcome: &'static str) {
        counter!(UPSERT_TOTAL, "outcome" => outcome).increment(1);
    }
}
