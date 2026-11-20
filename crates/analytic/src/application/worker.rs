//! The loop: a cycle at startup, then one every interval, until the stop.

use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use yog_bootstrap::Heartbeat;
use yog_core::RepositoryResult;
use yog_core::domain::{
    AnalyticRepository, CycleOutcome, CycleWrite, TvlMeasure, last_closed_hour,
};

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
                    self.run_cycle(Utc::now()).await
                } => {}
            }
        }
        log::stopped();
    }

    /// The cycle, then the TVL measure whatever the cycle did: it reads the pools' current
    /// state, not what the cycle writes.
    pub(crate) async fn run_cycle(&self, now: DateTime<Utc>) -> RepositoryResult<CycleOutcome> {
        let started = Instant::now();
        let written = self.write(now).await;
        let elapsed = started.elapsed();
        match &written {
            Ok(outcome) => {
                metrics::record_cycle("written", elapsed);
                metrics::record_frozen(outcome.hours_frozen);
                log::written(outcome, elapsed);
            }
            Err(error) => {
                metrics::record_cycle("failed", elapsed);
                log::failed(error, elapsed);
            }
        }
        // ⚠️ Its own clock, not the cycle's: a long cycle would date the pools' state minutes early.
        let measured = self.measure_tvl(Utc::now()).await;
        if let Some(heartbeat) = &self.heartbeat {
            match (&written, &measured) {
                (Ok(_), Ok(_)) => heartbeat.report_success().await,
                (Err(error), _) => {
                    heartbeat
                        .report_failure(&format!("cycle failed: {error}"))
                        .await;
                }
                (Ok(_), Err(error)) => {
                    heartbeat
                        .report_failure(&format!("TVL measure failed: {error}"))
                        .await;
                }
            }
        }
        written
    }

    async fn measure_tvl(&self, now: DateTime<Utc>) -> RepositoryResult<TvlMeasure> {
        let hour = last_closed_hour(now);
        let started = Instant::now();
        let measured = self.repository.measure_tvl(hour, now).await;
        let elapsed = started.elapsed();
        match &measured {
            Ok(TvlMeasure::Measured) => {
                metrics::record_tvl("measured", elapsed);
                log::tvl_measured(hour, elapsed);
            }
            Ok(TvlMeasure::AlreadyMeasured) => {}
            Err(error) => {
                metrics::record_tvl("failed", elapsed);
                log::tvl_failed(hour, error, elapsed);
            }
        }
        measured
    }

    async fn write(&self, now: DateTime<Utc>) -> RepositoryResult<CycleOutcome> {
        let progress = self.repository.progress().await?;
        self.repository
            .write_cycle(&CycleWrite::plan(now, progress))
            .await
    }
}

#[cfg(test)]
#[path = "worker_tests.rs"]
mod tests;
