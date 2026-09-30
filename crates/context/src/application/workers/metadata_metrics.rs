//! Metrics emitted by the metadata worker.

use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};

const TICK_TOTAL: &str = "yog_context_metadata_tick_total";
const TICK_DURATION: &str = "yog_context_metadata_tick_duration_seconds";
const MISSING_MINTS: &str = "yog_context_metadata_missing_mints";
const UPSERT_TOTAL: &str = "yog_context_metadata_upsert_total";

pub(crate) struct MetadataWorkerMetrics;

impl MetadataWorkerMetrics {
    /// Register once at startup, from `Daemon::new`.
    pub(crate) fn register_descriptions() {
        describe_counter!(
            TICK_TOTAL,
            "Metadata worker ticks completed (label: outcome=ok|no_work|list_failed|source_hard_error)"
        );
        describe_histogram!(
            TICK_DURATION,
            "Total duration of a metadata worker tick in seconds (label: outcome)"
        );
        describe_gauge!(
            MISSING_MINTS,
            "Number of mints missing metadata at the time of the last tick"
        );
        describe_counter!(
            UPSERT_TOTAL,
            "Token metadata upserts attempted (label: outcome=ok|error)"
        );
    }

    pub(crate) fn record_tick(outcome: &'static str, seconds: f64) {
        counter!(TICK_TOTAL, "outcome" => outcome).increment(1);
        histogram!(TICK_DURATION, "outcome" => outcome).record(seconds);
    }

    pub(crate) fn set_missing_mints(count: usize) {
        gauge!(MISSING_MINTS).set(count as f64);
    }

    pub(crate) fn record_upsert(outcome: &'static str) {
        counter!(UPSERT_TOTAL, "outcome" => outcome).increment(1);
    }
}
