//! The alarm's metrics. Nothing scrapes them in production — the
//! Healthchecks.io check is the alarm — but they say on `/metrics` how long
//! each aggregate has had a row waiting, without reading the logs.

use metrics::{counter, describe_counter, describe_gauge, gauge};

const PENDING: &str = "yog_signals_materialization_pending_seconds";
const CHECKS: &str = "yog_signals_materialization_checks_total";
/// Counted by `yog_bootstrap`'s heartbeat, which is handed this name.
pub(crate) const HEARTBEAT_FAILURES: &str = "yog_signals_heartbeat_failures_total";

pub(crate) struct AlarmMetrics;

impl AlarmMetrics {
    /// Register human-readable descriptions. Call once, before any check.
    pub(crate) fn register_descriptions() {
        describe_gauge!(
            PENDING,
            "How long the oldest raw row not yet materialised has waited, in \
             seconds; 0 when nothing waits (label: aggregate). Keeps its last \
             reading while the backlogs are unreadable — see outcome=unreadable"
        );
        describe_counter!(
            CHECKS,
            "Materialisation checks, by outcome=on_time|late|unreadable|nothing_reported"
        );
        describe_counter!(
            HEARTBEAT_FAILURES,
            "Heartbeat signals that could not be delivered, by kind"
        );
    }

    /// Record how long, in seconds, `aggregate`'s oldest pending row has
    /// waited — zero when none waits, so a gauge that went up comes back down.
    pub(super) fn record_pending(aggregate: &str, seconds: i64) {
        gauge!(PENDING, "aggregate" => aggregate.to_string()).set(seconds as f64);
    }

    /// Record how one check ended.
    pub(super) fn record_check(outcome: &'static str) {
        counter!(CHECKS, "outcome" => outcome).increment(1);
    }
}
