//! The materialisation alarm: when TimescaleDB's scheduler stops, the aggregates freeze and
//! nothing fails. This loop reads how long raw rows have waited to be materialised and reports to
//! a Healthchecks.io check: a late aggregate as `/fail` with its name, a stopped daemon as silence.
//! Without a heartbeat URL (development, scheduler off) the verdict is logged and measured only.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use yog_bootstrap::Heartbeat;
use yog_core::domain::{MaterializationBacklog, MaterializationBacklogRepository};

mod failure;
mod metrics;
mod verdict;

use failure::{Failure, hours_minutes};
pub(crate) use metrics::{AlarmMetrics, HEARTBEAT_FAILURES};
use verdict::Verdict;

/// The `statement_timeout` of the alarm's one-connection pool.
/// ⚠️ Ended by the server, so the connection comes back usable; a client-side wait alone would
/// leave the statement running and one more connection held at every check.
pub(crate) const STATEMENT_TIMEOUT: Duration = Duration::from_secs(60);

/// How long the alarm waits for a read before failing the check `unreadable` itself.
/// ⚠️ Longer than [`STATEMENT_TIMEOUT`]: the server's cancellation, which frees the connection, comes first.
const READ_TIMEOUT: Duration = Duration::from_secs(STATEMENT_TIMEOUT.as_secs() + 30);

/// The two numbers an operator tunes.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MaterializationAlarmSettings {
    /// Time between two checks.
    pub(crate) interval: Duration,
    /// How long a raw row may wait before its aggregate is late.
    pub(crate) max_wait: ChronoDuration,
}

pub(crate) struct MaterializationAlarm {
    repository: Arc<dyn MaterializationBacklogRepository>,
    heartbeat: Option<Arc<dyn Heartbeat>>,
    settings: MaterializationAlarmSettings,
}

impl MaterializationAlarm {
    pub(crate) fn new(
        repository: Arc<dyn MaterializationBacklogRepository>,
        heartbeat: Option<Arc<dyn Heartbeat>>,
        settings: MaterializationAlarmSettings,
    ) -> Self {
        Self {
            repository,
            heartbeat,
            settings,
        }
    }

    /// Check on every interval until `shutdown` is cancelled.
    /// ⚠️ The whole check races the stop, `biased`: a stop held by a slow endpoint ends in SIGKILL.
    /// Not `run_until_cancelled`, which breaks a tie the other way.
    pub(crate) async fn run(self, shutdown: CancellationToken) {
        let mut ticker = tokio::time::interval(self.settings.interval);
        // Missed ticks are not fired back to back.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        self.log_start();

        let mut previous: Option<Verdict> = None;
        loop {
            tokio::select! {
                biased;
                _ = shutdown.cancelled() => break,
                verdict = async {
                    ticker.tick().await;
                    self.check(Utc::now()).await
                } => {
                    log_change(previous.as_ref(), &verdict);
                    previous = Some(verdict);
                }
            }
        }
        info!("shutdown requested — materialisation alarm stopped");
    }

    /// One check: read, measure, judge, signal. Returns the verdict.
    pub(crate) async fn check(&self, now: DateTime<Utc>) -> Verdict {
        let read = tokio::time::timeout(READ_TIMEOUT, self.repository.backlogs()).await;
        let verdict = match read {
            Ok(Ok(backlogs)) => {
                record_pending(&backlogs, now);
                Verdict::judge(&backlogs, now, self.settings.max_wait)
            }
            Ok(Err(e)) => Verdict::Failed(Failure::Unreadable(e.to_string())),
            Err(_) => Verdict::Failed(Failure::Unreadable(format!(
                "no answer within {} s",
                READ_TIMEOUT.as_secs()
            ))),
        };
        AlarmMetrics::record_check(verdict.label());

        if let Some(heartbeat) = &self.heartbeat {
            match &verdict {
                Verdict::OnTime => heartbeat.success().await,
                Verdict::Failed(failure) => heartbeat.failure(&failure.reason()).await,
            }
        }
        verdict
    }

    /// Say what the alarm watches, once, and whether anyone will be told.
    fn log_start(&self) {
        info!(
            interval = ?self.settings.interval,
            max_wait = %hours_minutes(self.settings.max_wait),
            heartbeat = self.heartbeat.is_some(),
            "materialisation alarm started"
        );
        if self.heartbeat.is_none() {
            info!(
                "no heartbeat configured — a late aggregate is logged and measured, not signalled"
            );
        }
    }
}

/// Set every aggregate's pending gauge, to zero when no row waits, so a raised gauge comes down.
fn record_pending(backlogs: &[MaterializationBacklog], now: DateTime<Utc>) {
    for backlog in backlogs {
        let seconds = backlog
            .pending_for(now)
            .map_or(0, |wait| wait.num_seconds());
        AlarmMetrics::record_pending(&backlog.aggregate, seconds);
    }
}

/// Speak when the state changes, not on every tick: in development every aggregate is late for good.
fn log_change(previous: Option<&Verdict>, verdict: &Verdict) {
    if previous.map(Verdict::state) == Some(verdict.state()) {
        return;
    }
    match verdict {
        Verdict::OnTime => info!("every continuous aggregate is materialised on time"),
        Verdict::Failed(failure @ Failure::Late { .. }) => warn!(
            reason = %failure.reason(),
            "a continuous aggregate is not being materialised"
        ),
        Verdict::Failed(failure @ Failure::Unreadable(_)) => warn!(
            reason = %failure.reason(),
            "the materialisation backlogs could not be read"
        ),
        Verdict::Failed(failure @ Failure::NothingReported) => warn!(
            reason = %failure.reason(),
            "the materialisation backlogs name no aggregate — nothing is being watched"
        ),
    }
}

#[cfg(test)]
#[path = "materialization_alarm/materialization_alarm_tests.rs"]
mod tests;
