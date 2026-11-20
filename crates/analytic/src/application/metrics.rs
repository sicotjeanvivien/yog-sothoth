//! The writer's Prometheus metrics. The Healthchecks.io check is the alarm;
//! these say on `/metrics` how the cycles go.

use std::time::Duration;

use metrics::{counter, describe_counter, describe_histogram, histogram};

const CYCLES: &str = "yog_analytic_cycles_total";
const DURATION: &str = "yog_analytic_cycle_duration_seconds";
const HOURS_FROZEN: &str = "yog_analytic_hours_frozen_total";
const TVL_MEASURES: &str = "yog_analytic_tvl_measures_total";
const TVL_DURATION: &str = "yog_analytic_tvl_measure_duration_seconds";
/// Counted by `yog_bootstrap`'s heartbeat, which is handed this name.
pub(crate) const HEARTBEAT_FAILURES: &str = "yog_analytic_heartbeat_failures_total";

pub(crate) fn register_descriptions() {
    describe_counter!(CYCLES, "Analytic cycles, by outcome=written|failed");
    describe_histogram!(DURATION, "Duration of an analytic cycle, in seconds");
    describe_counter!(HOURS_FROZEN, "Hours frozen for good");
    describe_counter!(
        TVL_MEASURES,
        "TVL measures of the last closed hour, by outcome=measured|failed"
    );
    describe_histogram!(TVL_DURATION, "Duration of a TVL measure, in seconds");
    describe_counter!(
        HEARTBEAT_FAILURES,
        "Heartbeat signals that could not be delivered, by kind"
    );
}

pub(super) fn record_cycle(outcome: &'static str, elapsed: Duration) {
    counter!(CYCLES, "outcome" => outcome).increment(1);
    histogram!(DURATION, "outcome" => outcome).record(elapsed.as_secs_f64());
}

pub(super) fn record_frozen(hours: u64) {
    counter!(HOURS_FROZEN).increment(hours);
}

pub(super) fn record_tvl(outcome: &'static str, elapsed: Duration) {
    counter!(TVL_MEASURES, "outcome" => outcome).increment(1);
    histogram!(TVL_DURATION, "outcome" => outcome).record(elapsed.as_secs_f64());
}
