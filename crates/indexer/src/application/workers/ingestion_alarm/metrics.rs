//! The ingestion alarm's metrics. Nothing scrapes them in production — the
//! Healthchecks.io check is the alarm — but `/metrics` says how the checks
//! went without reading the logs.

use metrics::{counter, describe_counter};

const CHECKS: &str = "yog_indexer_ingestion_checks_total";
/// Counted by `yog_bootstrap`'s heartbeat, which is handed this name.
pub(crate) const HEARTBEAT_FAILURES: &str = "yog_indexer_heartbeat_failures_total";

pub(crate) struct IngestionAlarmMetrics;

impl IngestionAlarmMetrics {
    /// Register human-readable descriptions. Call once, before any check.
    pub(crate) fn register_descriptions() {
        describe_counter!(
            CHECKS,
            "Ingestion checks, by outcome=live|delayed|stale|unreadable"
        );
        describe_counter!(
            HEARTBEAT_FAILURES,
            "Heartbeat signals that could not be delivered, by kind"
        );
    }

    /// Record how one check ended.
    pub(super) fn record_check(outcome: &'static str) {
        counter!(CHECKS, "outcome" => outcome).increment(1);
    }
}
