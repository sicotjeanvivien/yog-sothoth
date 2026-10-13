//! The loop: a cycle at startup, then one every interval, until the stop.

use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use yog_bootstrap::Heartbeat;
use yog_core::RepositoryResult;
use yog_core::domain::{AnalyticRepository, CycleOutcome, CycleWrite};

use super::{log, metrics};

pub(crate) struct AnalyticWorker {
    repository: Arc<dyn AnalyticRepository>,
    heartbeat: Option<Arc<dyn Heartbeat>>,
    interval: Duration,
}

impl AnalyticWorker {
    pub(crate) fn new(
        repository: Arc<dyn AnalyticRepository>,
        heartbeat: Option<Arc<dyn Heartbeat>>,
        interval: Duration,
    ) -> Self {
        Self {
            repository,
            heartbeat,
            interval,
        }
    }

    /// A stop drops the cycle in flight, and its transaction with it: the next
    /// start writes it again.
    pub(crate) async fn run(self, shutdown: CancellationToken) {
        let mut ticker = tokio::time::interval(self.interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        log::started(self.interval, self.heartbeat.is_some());
        loop {
            tokio::select! {
                // ⚠️ A stop that arrives with a tick due must not start a cycle.
                biased;
                () = shutdown.cancelled() => break,
                _ = async {
                    ticker.tick().await;
                    self.cycle(Utc::now()).await
                } => {}
            }
        }
        log::stopped();
    }

    pub(crate) async fn cycle(&self, now: DateTime<Utc>) -> RepositoryResult<CycleOutcome> {
        let started = Instant::now();
        let written = self.write(now).await;
        let elapsed = started.elapsed();
        match &written {
            Ok((cycle, outcome)) => {
                metrics::record_cycle("written", elapsed);
                metrics::record_frozen(outcome.hours_frozen);
                log::written(cycle, outcome, elapsed);
                if let Some(heartbeat) = &self.heartbeat {
                    heartbeat.success().await;
                }
            }
            Err(error) => {
                metrics::record_cycle("failed", elapsed);
                log::failed(error, elapsed);
                if let Some(heartbeat) = &self.heartbeat {
                    heartbeat.failure(&format!("cycle failed: {error}")).await;
                }
            }
        }
        written.map(|(_, outcome)| outcome)
    }

    async fn write(&self, now: DateTime<Utc>) -> RepositoryResult<(CycleWrite, CycleOutcome)> {
        let progress = self.repository.progress().await?;
        let cycle = CycleWrite::plan(now, progress);
        let outcome = self.repository.write_cycle(&cycle).await?;
        Ok((cycle, outcome))
    }
}

#[cfg(test)]
#[path = "worker_tests.rs"]
mod tests;
