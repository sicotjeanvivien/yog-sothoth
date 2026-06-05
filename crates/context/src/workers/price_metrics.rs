//! Metrics emitted by the price worker.

use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};

const TICK_TOTAL: &str = "yog_context_price_tick_total";
const TICK_DURATION: &str = "yog_context_price_tick_duration_seconds";
const KNOWN_MINTS: &str = "yog_context_price_known_mints";
const INSERTED_TOTAL: &str = "yog_context_price_inserted_total";

pub(crate) struct PriceWorkerMetrics;

impl PriceWorkerMetrics {
    pub(crate) fn register_descriptions() {
        describe_counter!(
            TICK_TOTAL,
            "Price worker ticks completed (label: outcome=ok|no_work|list_failed|source_hard_error|no_prices|insert_failed)"
        );
        describe_histogram!(
            TICK_DURATION,
            "Total duration of a price worker tick in seconds (label: outcome)"
        );
        describe_gauge!(
            KNOWN_MINTS,
            "Number of known mints submitted to the price source at the last tick"
        );
        describe_counter!(
            INSERTED_TOTAL,
            "Token prices successfully inserted (cumulative count of rows)"
        );
    }

    pub(crate) fn record_tick(outcome: &'static str, seconds: f64) {
        counter!(TICK_TOTAL, "outcome" => outcome).increment(1);
        histogram!(TICK_DURATION, "outcome" => outcome).record(seconds);
    }

    pub(crate) fn set_known_mints(count: usize) {
        gauge!(KNOWN_MINTS).set(count as f64);
    }

    pub(crate) fn record_inserted(count: usize) {
        counter!(INSERTED_TOTAL).increment(count as u64);
    }
}
