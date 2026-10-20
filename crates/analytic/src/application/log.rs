//! The lines the writer writes, one function each.

use std::time::Duration;

use tracing::{error, info};
use yog_core::RepositoryError;
use yog_core::domain::CycleOutcome;

pub(super) fn started(interval: Duration, heartbeat: bool) {
    info!(interval = ?interval, heartbeat, "analytic writer started");
    if !heartbeat {
        info!("no heartbeat configured — a failed cycle is logged and measured, not signalled");
    }
}

pub(super) fn written(outcome: &CycleOutcome, elapsed: Duration) {
    let cycle = &outcome.written;
    let tranches = (outcome.tranches_written > 0)
        .then(|| format!("{}..={}", cycle.tranches_from, cycle.tranches_through));
    info!(
        tranches = ?tranches,
        tranches_written = outcome.tranches_written,
        from = %cycle.from,
        through = %cycle.through,
        freeze_through = ?cycle.freeze_through,
        pool_hours = outcome.pool_hours,
        hours_frozen = outcome.hours_frozen,
        secs = elapsed.as_secs_f64(),
        "analytic cycle written"
    );
}

pub(super) fn failed(error: &RepositoryError, elapsed: Duration) {
    error!(error = %error, secs = elapsed.as_secs_f64(), "analytic cycle failed");
}

pub(super) fn stopped() {
    info!("shutdown requested — analytic writer stopped");
}
