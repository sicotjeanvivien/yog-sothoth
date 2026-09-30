//! The materialisation alarm — the one alarm on the continuous aggregates.
//!
//! The detectors read hourly aggregates that TimescaleDB's scheduler keeps
//! materialised. When it stops, nothing fails: the aggregates freeze and the
//! detectors keep evaluating hours that no longer change. From 16 June to
//! 10 August 2026 all four sat that way unnoticed. This loop asks the database
//! how long raw rows have been waiting to be materialised, and reports the
//! verdict to a Healthchecks.io check — a late aggregate as `/fail` with its
//! name, and a stopped daemon by the silence the check notices on its own.
//!
//! Nothing scrapes Prometheus in production: the gauge is for the diagnosis,
//! the heartbeat is the alarm. Without a heartbeat URL (development, where the
//! scheduler is off by design) the verdict is logged and measured only.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use yog_bootstrap::Heartbeat;
use yog_core::domain::MaterializationBacklogRepository;

use super::materialization_alarm_metrics::AlarmMetrics;
use super::materialization_verdict::{Failure, Verdict, hours_minutes};

/// How long one read of the backlogs may take before the check fails with
/// `unreadable`. The pool sets no `statement_timeout`: a read stuck on a lock
/// would otherwise hold the check forever, and Healthchecks.io would report a
/// stopped daemon instead of the reason. Measured at 3.5 ms warm on the dev
/// database (29 September 2026), so a minute is margin, not a budget.
const READ_TIMEOUT: Duration = Duration::from_secs(60);

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
    ///
    /// One `select!` per turn: the stop on one side, and on the other the wait
    /// for the tick **then** the whole check — the read, then the ping. So the
    /// check races the stop too: a ping cut short is harmless (the check simply
    /// sees the next one, or none), while a stop held by a slow endpoint is the
    /// delay Docker ends with SIGKILL. `biased`, for the reason the detector
    /// loops give: the stop must win a tie. `CancellationToken::run_until_cancelled`
    /// would read shorter, and its documentation says it breaks a tie the other
    /// way.
    pub(crate) async fn run(self, shutdown: CancellationToken) {
        let mut ticker = tokio::time::interval(self.settings.interval);
        // After a slow check, the next one waits a full interval rather than
        // firing the missed ones back to back — each would read and ping again.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
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
                for backlog in &backlogs {
                    let seconds = backlog
                        .pending_for(now)
                        .map_or(0, |wait| wait.num_seconds());
                    AlarmMetrics::record_pending(&backlog.aggregate, seconds);
                }
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
}

/// Speak when the state changes, not on every tick: in development the
/// scheduler is off, every aggregate is late for good, and a warning every
/// ten minutes would bury everything else.
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
#[path = "materialization_alarm_tests.rs"]
mod tests;
