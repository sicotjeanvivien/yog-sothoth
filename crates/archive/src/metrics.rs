//! The archiver's Prometheus metrics.
//!
//! Nobody scrapes them in production yet — Healthchecks.io is what raises the
//! alarm. They exist so the archiver reads like the other daemons the day a
//! Prometheus is added, and so a run can be inspected on `/metrics` without
//! reading logs.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};

use crate::archiver::RunOutcome;

const RUNS: &str = "yog_archive_runs_total";
const LAST_SUCCESS: &str = "yog_archive_last_success_timestamp_seconds";
const DUMP_BYTES: &str = "yog_archive_dump_bytes";
const DURATION: &str = "yog_archive_duration_seconds";
const HEARTBEAT_FAILURES: &str = "yog_archive_heartbeat_failures_total";

pub(crate) fn register_descriptions() {
    describe_counter!(RUNS, "Archiving runs, by outcome");
    describe_gauge!(
        LAST_SUCCESS,
        "Unix time of the last run that put a dump in the bucket"
    );
    describe_gauge!(DUMP_BYTES, "Size of the last archived dump, in bytes");
    describe_histogram!(DURATION, "Duration of an archiving run, in seconds");
    describe_counter!(
        HEARTBEAT_FAILURES,
        "Heartbeat signals that could not be delivered, by kind"
    );
}

/// Record how a run ended.
pub(crate) fn record(outcome: &RunOutcome, elapsed: Duration) {
    counter!(RUNS, "outcome" => outcome.label()).increment(1);
    histogram!(DURATION, "outcome" => outcome.label()).record(elapsed.as_secs_f64());
    if let RunOutcome::Archived { bytes, .. } = outcome {
        gauge!(DUMP_BYTES).set(*bytes as f64);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        gauge!(LAST_SUCCESS).set(now.as_secs_f64());
    }
}

pub(crate) fn heartbeat_failed(kind: &'static str) {
    counter!(HEARTBEAT_FAILURES, "kind" => kind).increment(1);
}
