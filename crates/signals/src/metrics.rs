//! Metrics emitted by the signal engine.
//!
//! Mirrors the other daemons: cumulative counters exposed on the
//! Prometheus `/metrics` endpoint the binary installs. The lib emits
//! through the `metrics` facade (a no-op if no recorder is installed, so
//! unit tests need no exporter); the binary installs the exporter and
//! calls [`EngineMetrics::register_descriptions`] once at startup.

use metrics::{counter, describe_counter};

const TICK_TOTAL: &str = "yog_signals_tick_total";
const EMITTED_TOTAL: &str = "yog_signals_emitted_total";

/// Counters for the engine's per-detector poll loops.
pub struct EngineMetrics;

impl EngineMetrics {
    /// Register human-readable descriptions. Call once, before any tick.
    pub fn register_descriptions() {
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
