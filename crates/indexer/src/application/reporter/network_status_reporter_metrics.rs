//! Metrics emitted by the network status reporter.
//!
//! One family, and it exists because a failed tick no longer stops anything.
//! Until 11 September 2026 the first failed `getSlot` took the daemon down with
//! it, which was loud; now it is skipped, and this counter is what remains of
//! the noise.

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

    /// A tick that recorded nothing.
    ///
    /// ⚠️ **Not a sign that ingestion is down.** The probe and the data path
    /// share an endpoint and a network, so they usually fail together — but
    /// the decision to stop belongs to the data path alone, and this series
    /// rising while `yog_indexer_*` keeps advancing says the probe is the only
    /// thing hurting. A `network_status.observed_at` that stops moving is the
    /// same fact, seen from the dashboard.
    pub(crate) fn record_tick_failure(reason: &'static str) {
        counter!(TICK_FAILURES, "reason" => reason).increment(1);
    }
}
