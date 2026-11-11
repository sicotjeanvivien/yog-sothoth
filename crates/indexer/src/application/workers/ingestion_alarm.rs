//! The ingestion alarm: tells a Healthchecks.io check when this process stops writing, which
//! `restart: unless-stopped` cannot see. An ingestion the dashboard no longer shows live pings `/fail`, a
//! stopped daemon is silence.
//! ⚠️ It judges what was *written*, by the dashboard's [`from_last_event`](yog_core::domain::FreshnessStatus::from_last_event),
//! not what the process believes: a failing persist is stepped over while counts keep climbing.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use yog_bootstrap::Heartbeat;
use yog_core::domain::EventFreshnessRepository;

mod log;
mod metrics;
mod verdict;

pub(crate) use metrics::{HEARTBEAT_FAILURES, IngestionAlarmMetrics};
use verdict::Verdict;

/// Time between two checks, and between two pings: the check's period must say the same, or it
/// reports the daemon down.
const CHECK_INTERVAL: Duration = Duration::from_secs(300);

/// The `statement_timeout` of the alarm's one-connection pool.
/// ⚠️ Ended by the server, so the connection comes back usable; a client-side wait alone would
/// leave the statement running and one more connection held at every check.
pub(crate) const STATEMENT_TIMEOUT: Duration = Duration::from_secs(60);

/// How long the alarm waits for a read before failing the check `unreadable` itself.
/// ⚠️ Longer than [`STATEMENT_TIMEOUT`]: the server's cancellation, which frees the connection, comes first.
const READ_TIMEOUT: Duration = Duration::from_secs(STATEMENT_TIMEOUT.as_secs() + 30);

pub(crate) struct IngestionAlarm {
    repository: Arc<dyn EventFreshnessRepository>,
    heartbeat: Option<Arc<dyn Heartbeat>>,
}

impl IngestionAlarm {
    pub(crate) fn new(
        repository: Arc<dyn EventFreshnessRepository>,
        heartbeat: Option<Arc<dyn Heartbeat>>,
    ) -> Self {
        Self {
            repository,
            heartbeat,
        }
    }

    /// Check on every interval until `shutdown` is cancelled; `Infallible`, nothing a check meets
    /// is a reason to stop indexing.
    /// ⚠️ The whole check races the stop, `biased`: a stop held by a slow endpoint ends in SIGKILL.
    pub(crate) async fn run(self, shutdown: CancellationToken) -> Result<(), Infallible> {
        let mut ticker = tokio::time::interval(CHECK_INTERVAL);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        self.log_start();

        let mut previous: Option<&'static str> = None;
        loop {
            tokio::select! {
                biased;
                _ = shutdown.cancelled() => break,
                (verdict, now) = async {
                    ticker.tick().await;
                    let now = Utc::now();
                    (self.check(now).await, now)
                } => {
                    log_change(previous, &verdict, now);
                    previous = Some(verdict.label());
                }
            }
        }
        log::alarm_stopped();
        Ok(())
    }

    /// One check: read, judge, count, signal. Returns the verdict.
    pub(crate) async fn check(&self, now: DateTime<Utc>) -> Verdict {
        let read = tokio::time::timeout(READ_TIMEOUT, self.repository.last_event_at()).await;
        let verdict = match read {
            Ok(Ok(last_event_at)) => Verdict::judge(last_event_at, now),
            Ok(Err(e)) => Verdict::Unreadable(e.to_string()),
            Err(_) => Verdict::Unreadable(format!("no answer within {} s", READ_TIMEOUT.as_secs())),
        };
        IngestionAlarmMetrics::record_check(verdict.label());

        if let Some(heartbeat) = &self.heartbeat {
            match verdict.failure_reason(now) {
                None => heartbeat.report_success().await,
                Some(reason) => heartbeat.report_failure(&reason).await,
            }
        }
        verdict
    }

    /// Say what the alarm watches, once, and whether anyone will be told.
    fn log_start(&self) {
        log::alarm_started(CHECK_INTERVAL, self.heartbeat.is_some());
        if self.heartbeat.is_none() {
            log::no_heartbeat();
        }
    }
}

/// Speak when the outcome changes, not on every tick, or a long outage buries everything else.
fn log_change(previous: Option<&'static str>, verdict: &Verdict, now: DateTime<Utc>) {
    if previous == Some(verdict.label()) {
        return;
    }
    let reason = verdict.failure_reason(now);
    let reason = reason.as_deref().unwrap_or_default();
    match verdict {
        Verdict::Live => log::writing(),
        Verdict::Delayed { .. } => log::falling_behind(reason),
        Verdict::Stale { .. } => log::stopped_writing(reason),
        Verdict::Unreadable(_) => log::unreadable(reason),
    }
}

#[cfg(test)]
#[path = "ingestion_alarm/ingestion_alarm_tests.rs"]
mod tests;
