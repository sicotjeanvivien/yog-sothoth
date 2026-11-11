//! The lines the ingestion alarm writes, one function each.

use std::time::Duration;

use tracing::{info, warn};

/// The alarm started: what it watches, and whether anyone will be told.
pub(super) fn alarm_started(interval: Duration, heartbeat: bool) {
    info!(?interval, heartbeat, "ingestion alarm started");
}

/// No heartbeat is configured: verdicts are logged and counted only.
pub(super) fn no_heartbeat() {
    info!("no heartbeat configured — a stopped ingestion is logged and counted, not signalled");
}

/// A stop was requested and the alarm has stopped.
pub(super) fn alarm_stopped() {
    info!("shutdown requested — ingestion alarm stopped");
}

/// The ingestion is live.
pub(super) fn writing() {
    info!(state = "live", "the ingestion is writing events");
}

/// The dashboard no longer shows the ingestion live.
pub(super) fn falling_behind(reason: &str) {
    warn!(reason, "the ingestion is falling behind");
}

/// Nothing indexed for too long, or ever.
pub(super) fn stopped_writing(reason: &str) {
    warn!(reason, "the ingestion has stopped writing events");
}

/// The last indexed event could not be read.
pub(super) fn unreadable(reason: &str) {
    warn!(reason, "the last indexed event could not be read");
}
