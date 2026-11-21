//! The dead man's switch of the sources: every window, a success, or a failure naming each source
//! called without a single success.

use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use yog_bootstrap::Heartbeat;

use super::source_health::{SourceHealth, WindowVerdict};
use crate::error::WorkerError;

mod log;

/// ⚠️ The Healthchecks.io check's period is set on it: changed alone, it raises false alarms.
pub(crate) const HEARTBEAT_WINDOW: Duration = Duration::from_secs(300);

pub(crate) struct HeartbeatReporter {
    health: Arc<SourceHealth>,
    heartbeat: Arc<dyn Heartbeat>,
}

impl HeartbeatReporter {
    pub(crate) fn new(health: Arc<SourceHealth>, heartbeat: Arc<dyn Heartbeat>) -> Self {
        Self { health, heartbeat }
    }

    /// Pings at once, on an empty window, then at the end of every window, until the stop.
    pub(crate) async fn run(self, shutdown: CancellationToken) -> Result<(), WorkerError> {
        log::started(HEARTBEAT_WINDOW);
        let mut ticker = tokio::time::interval(HEARTBEAT_WINDOW);
        loop {
            tokio::select! {
                biased;

                _ = shutdown.cancelled() => {
                    log::stopping();
                    return Ok(());
                }
                _ = ticker.tick() => self.report().await,
            }
        }
    }

    async fn report(&self) {
        match self.health.take_window() {
            WindowVerdict::Healthy => self.heartbeat.report_success().await,
            WindowVerdict::Failing(reason) => {
                log::sources_failing(&reason);
                self.heartbeat.report_failure(&reason).await;
            }
        }
    }
}

#[cfg(test)]
#[path = "heartbeat_tests.rs"]
mod tests;
