//! The engine's metrics: one tick per detector loop, and what it persisted.
//!
//! Cumulative counters on the Prometheus `/metrics` endpoint the binary
//! installs. Emitted through the `metrics` facade — a no-op without a
//! recorder, so unit tests need no exporter; the daemon calls
//! [`EngineMetrics::register_descriptions`] once at startup.

use metrics::{counter, describe_counter};

const TICK_TOTAL: &str = "yog_signals_tick_total";
const EMITTED_TOTAL: &str = "yog_signals_emitted_total";

/// Counters for the engine's per-detector poll loops.
pub(crate) struct EngineMetrics;

impl EngineMetrics {
    /// Register human-readable descriptions. Call once, before any tick.
    pub(crate) fn register_descriptions() {
        describe_counter!(
            TICK_TOTAL,
            "Detector ticks completed (labels: detector, \
             outcome=ok|suppressed|eval_failed|dedup_failed|persist_failed)"
        );
        describe_counter!(
            EMITTED_TOTAL,
            "Signals persisted, cumulative (label: detector)"
        );
    }

    /// Record one completed tick with its outcome.
    pub(crate) fn record_tick(detector: &'static str, outcome: &'static str) {
        counter!(TICK_TOTAL, "detector" => detector, "outcome" => outcome).increment(1);
    }

    /// Record signals successfully persisted on a tick.
    pub(crate) fn record_emitted(detector: &'static str, count: usize) {
        counter!(EMITTED_TOTAL, "detector" => detector).increment(count as u64);
    }
}
