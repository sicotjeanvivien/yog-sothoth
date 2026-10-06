//! The ingestion alarm — tells someone when this process stops writing.
//!
//! The ingestion can stop without the process dying: a stream the provider
//! refuses, a WebSocket that never reopens, a database that refuses the
//! writes. `restart: unless-stopped` only brings back a process that died, and
//! the other checks look elsewhere — the materialisation alarm of `yog-signals`
//! deliberately does not read a stalled ingestion as its own fault. This loop
//! reads when the last event was indexed and reports the verdict to a
//! Healthchecks.io check: a stale ingestion as `/fail` with the age of its last
//! event, and a stopped daemon by the silence the check notices on its own.
//!
//! **The verdict is the dashboard's**, [`FreshnessStatus::from_last_event`] —
//! the rule behind the live indicator — so the alarm and the panel cannot
//! disagree. It reads what was *written*, not what this process believes it
//! did: a persist that fails is logged and stepped over, so a count of indexed
//! transactions would keep climbing on a database that refuses every row.
//!
//! Without a heartbeat URL (development) the verdict is logged and counted
//! only.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use yog_bootstrap::Heartbeat;
use yog_core::domain::{EventFreshnessRepository, FreshnessStatus};

use super::ingestion_alarm_metrics::IngestionAlarmMetrics;

/// Time between two checks — and between two pings of the check, whose period
/// must say the same or it reports the daemon down.
///
/// With the dashboard's fifteen minutes before `Stale`, a stopped ingestion
/// fails the check at most twenty minutes after its last event.
const CHECK_INTERVAL: Duration = Duration::from_secs(300);

/// How long Postgres lets one read of the last event run before cancelling it —
/// the `statement_timeout` of the alarm's own pool, opened by the daemon with
/// one connection. A read stuck on a lock is ended **by the server**, so its
/// connection comes back usable: stopping only the client's wait would leave
/// the statement running and the connection held, one more at every check.
pub(crate) const STATEMENT_TIMEOUT: Duration = Duration::from_secs(60);

/// How long the alarm waits for a read before failing the check with
/// `unreadable` on its own — for a server that cannot answer at all, not even
/// to cancel. Longer than [`STATEMENT_TIMEOUT`] by construction, so that the
/// server's cancellation, which frees the connection, comes first; otherwise
/// Healthchecks.io would report a stopped daemon instead of the reason.
const READ_TIMEOUT: Duration = Duration::from_secs(STATEMENT_TIMEOUT.as_secs() + 30);

/// How one check ended — and what the heartbeat is told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// The last event is recent: success.
    Live,
    /// Older than usual, not yet stopped — a lull, which a quiet watched pool
    /// produces several times a day: success.
    Delayed,
    /// Nothing indexed for too long, or ever (`None`): failure.
    Stale {
        last_event_at: Option<DateTime<Utc>>,
    },
    /// The last event could not be read; the text says why: failure.
    Unreadable(String),
}

impl Verdict {
    /// Judge the last event at `now`, by the rule `core` holds.
    fn judge(last_event_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> Self {
        match FreshnessStatus::from_last_event(last_event_at, now) {
            FreshnessStatus::Live => Self::Live,
            FreshnessStatus::Delayed => Self::Delayed,
            FreshnessStatus::Stale => Self::Stale { last_event_at },
        }
    }

    /// The `outcome` label of the checks counter.
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Live => FreshnessStatus::Live.as_str(),
            Self::Delayed => FreshnessStatus::Delayed.as_str(),
            Self::Stale { .. } => FreshnessStatus::Stale.as_str(),
            Self::Unreadable(_) => "unreadable",
        }
    }

    /// What `/fail` and the logs say, or `None` when the check succeeds.
    fn failure_reason(&self, now: DateTime<Utc>) -> Option<String> {
        match self {
            Self::Live | Self::Delayed => None,
            Self::Stale {
                last_event_at: Some(at),
            } => Some(format!(
                "stale: last event {} ago, at {}",
                hours_minutes(now - *at),
                at.format("%Y-%m-%d %H:%M UTC")
            )),
            Self::Stale {
                last_event_at: None,
            } => Some("stale: no event has ever been indexed".to_string()),
            Self::Unreadable(error) => Some(format!("unreadable: {error}")),
        }
    }
}

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

    /// Check on every interval until `shutdown` is cancelled.
    ///
    /// One `select!` per turn: the stop on one side, and on the other the wait
    /// for the tick **then** the whole check — the read, then the ping. So the
    /// check races the stop: a ping cut short is harmless (the check simply
    /// sees the next one, or none), while a stop held by a slow endpoint is the
    /// delay Docker ends with SIGKILL. `biased`: the stop must win a tie.
    ///
    /// `Infallible`, like the network status reporter: nothing a check meets
    /// is a reason to stop indexing, and only a panic ends this task early.
    pub(crate) async fn run(self, shutdown: CancellationToken) -> Result<(), Infallible> {
        let mut ticker = tokio::time::interval(CHECK_INTERVAL);
        // After a slow check, the next one waits a full interval rather than
        // firing the missed ones back to back — each would read and ping again.
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
        info!("shutdown requested — ingestion alarm stopped");
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
                None => heartbeat.success().await,
                Some(reason) => heartbeat.failure(&reason).await,
            }
        }
        verdict
    }

    /// Say what the alarm watches, once, and whether anyone will be told.
    fn log_start(&self) {
        info!(
            interval = ?CHECK_INTERVAL,
            heartbeat = self.heartbeat.is_some(),
            "ingestion alarm started"
        );
        if self.heartbeat.is_none() {
            info!(
                "no heartbeat configured — a stopped ingestion is logged and counted, not signalled"
            );
        }
    }
}

/// Speak when the outcome changes, not on every tick: through a long outage a
/// warning every five minutes would bury everything else.
fn log_change(previous: Option<&'static str>, verdict: &Verdict, now: DateTime<Utc>) {
    if previous == Some(verdict.label()) {
        return;
    }
    let reason = verdict.failure_reason(now);
    match verdict {
        Verdict::Live | Verdict::Delayed => {
            info!(state = verdict.label(), "the ingestion is writing events");
        }
        Verdict::Stale { .. } => warn!(
            reason = reason.as_deref().unwrap_or_default(),
            "the ingestion has stopped writing events"
        ),
        Verdict::Unreadable(_) => warn!(
            reason = reason.as_deref().unwrap_or_default(),
            "the last indexed event could not be read"
        ),
    }
}

/// A duration as `4h05m`, the form the reason uses.
fn hours_minutes(duration: ChronoDuration) -> String {
    let minutes = duration.num_minutes();
    format!("{}h{:02}m", minutes / 60, minutes % 60)
}

#[cfg(test)]
#[path = "ingestion_alarm_tests.rs"]
mod tests;
