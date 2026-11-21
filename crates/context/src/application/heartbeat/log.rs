//! The heartbeat reporter's log lines, one function each.

use std::time::Duration;

use tracing::{info, warn};

pub(super) fn started(window: Duration) {
    info!(window = ?window, "heartbeat reporter started");
}

/// A stop was asked; the reporter leaves between two windows.
pub(super) fn stopping() {
    info!("shutdown requested — heartbeat reporter stopping");
}

/// A window closed with at least one source called without a single success.
pub(super) fn sources_failing(reason: &str) {
    warn!(
        reason,
        "heartbeat: a source refused every call of the window"
    );
}
