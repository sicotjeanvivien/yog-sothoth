//! Metrics emitted by the network status reporter: what remains of a failed tick, which stops nothing.

use metrics::{counter, describe_counter};

const TICK_FAILURES: &str = "yog_indexer_network_status_tick_failures_total";

pub(crate) struct NetworkStatusReporterMetrics;

impl NetworkStatusReporterMetrics {
    pub(crate) fn register_descriptions() {
        describe_counter!(
            TICK_FAILURES,
            "Network status ticks that recorded no snapshot and were skipped \
             (label: reason — `rpc` or `persistence`)"
        );
    }

    /// A tick that recorded nothing; not a sign that ingestion is down.
    /// ⚠️ The only signal of a frozen `network_status.observed_at`: the dashboard reads freshness
    /// from the last indexed event.
    pub(crate) fn record_tick_failure(reason: &'static str) {
        counter!(TICK_FAILURES, "reason" => reason).increment(1);
    }
}
