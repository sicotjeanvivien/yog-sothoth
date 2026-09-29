//! The materialisation watch — the one alarm on the continuous aggregates.
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
use yog_core::domain::{AggregateMaterialization, MaterializationRepository};

use crate::metrics::MaterializationMetrics;

/// How long one read of the progress may take before the check fails with
/// `unreadable`. The pool sets no `statement_timeout`: a read stuck on a lock
/// would otherwise hold the check forever, and Healthchecks.io would report a
/// stopped daemon instead of the reason. Measured at 3.5 ms warm on the dev
/// database (29 September 2026), so a minute is margin, not a budget.
const READ_TIMEOUT: Duration = Duration::from_secs(60);

/// The two numbers an operator tunes.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MaterializationWatchSettings {
    /// Time between two checks.
    pub(crate) interval: Duration,
    /// How long a raw row may wait before its aggregate is late.
    pub(crate) max_wait: ChronoDuration,
}

/// How one check ended — and what the heartbeat is told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// No raw row has waited longer than the limit.
    OnTime,
    /// These aggregates have a row waiting beyond the limit, with how long.
    Late(Vec<(String, ChronoDuration)>),
    /// The database could not be read; the error says why.
    Unreadable(String),
}

impl Verdict {
    /// Judge what the database reported, at `now`.
    pub(crate) fn judge(
        progress: &[AggregateMaterialization],
        now: DateTime<Utc>,
        max_wait: ChronoDuration,
    ) -> Self {
        // ⚠️ **Nothing reported is not "on time".** The function finds the
        // aggregates in TimescaleDB's catalog; if an upgrade changed what it
        // joins on, it would return no row, and every check would ping success
        // while watching nothing — the silent green this alarm exists to end.
        // The schema always holds aggregates, so an empty answer is a fault.
        if progress.is_empty() {
            return Self::Unreadable("no continuous aggregate reported".to_string());
        }
        let late: Vec<_> = progress
            .iter()
            .filter_map(|m| Some((m.aggregate.clone(), m.late_by(now, max_wait)?)))
            .collect();
        if late.is_empty() {
            Self::OnTime
        } else {
            Self::Late(late)
        }
    }

    /// The `outcome` label of the checks counter.
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::OnTime => "on_time",
            Self::Late(_) => "late",
            Self::Unreadable(_) => "unreadable",
        }
    }

    /// What a failed check says, in the check's log and in ours. `None` when
    /// there is nothing to say.
    fn reason(&self, max_wait: ChronoDuration) -> Option<String> {
        match self {
            Self::OnTime => None,
            Self::Late(late) => {
                let names = late
                    .iter()
                    .map(|(name, wait)| format!("{name} pending for {}", hours_minutes(*wait)))
                    .collect::<Vec<_>>()
                    .join("; ");
                Some(format!("late (limit {}): {names}", hours_minutes(max_wait)))
            }
            Self::Unreadable(error) => Some(format!("unreadable: {error}")),
        }
    }

    /// What must change for the logs to speak again: the outcome, or which
    /// aggregates are late — not how long they have been.
    fn state(&self) -> (&'static str, Vec<&str>) {
        let names = match self {
            Self::Late(late) => late.iter().map(|(name, _)| name.as_str()).collect(),
            _ => Vec::new(),
        };
        (self.label(), names)
    }
}

fn hours_minutes(duration: ChronoDuration) -> String {
    let minutes = duration.num_minutes();
    format!("{}h{:02}m", minutes / 60, minutes % 60)
}

pub(crate) struct MaterializationWatch {
    repository: Arc<dyn MaterializationRepository>,
    heartbeat: Option<Arc<dyn Heartbeat>>,
    settings: MaterializationWatchSettings,
}

impl MaterializationWatch {
    pub(crate) fn new(
        repository: Arc<dyn MaterializationRepository>,
        heartbeat: Option<Arc<dyn Heartbeat>>,
        settings: MaterializationWatchSettings,
    ) -> Self {
        Self {
            repository,
            heartbeat,
            settings,
        }
    }

    /// Check on every interval until `shutdown` is cancelled.
    ///
    /// The whole check — the read, then the ping — races the stop: a ping cut
    /// short is harmless (the check simply sees the next one, or none), while a
    /// stop held by a slow endpoint is the delay Docker ends with SIGKILL.
    pub(crate) async fn run(self, shutdown: CancellationToken) {
        let mut ticker = tokio::time::interval(self.settings.interval);
        // After a slow check, the next one waits a full interval rather than
        // firing the missed ones back to back — each would read and ping again.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        info!(
            interval = ?self.settings.interval,
            max_wait = %hours_minutes(self.settings.max_wait),
            heartbeat = self.heartbeat.is_some(),
            "materialisation watch started"
        );
        if self.heartbeat.is_none() {
            info!(
                "no heartbeat configured — a late aggregate is logged and measured, not signalled"
            );
        }

        let mut previous: Option<Verdict> = None;
        loop {
            // `biased` for the reason the detector loops give: the stop must
            // win a tie with a tick that is already due.
            tokio::select! {
                biased;
                _ = shutdown.cancelled() => break,
                _ = ticker.tick() => {}
            }
            tokio::select! {
                biased;
                _ = shutdown.cancelled() => break,
                verdict = self.check(Utc::now()) => {
                    self.log_change(previous.as_ref(), &verdict);
                    previous = Some(verdict);
                }
            }
        }
        info!("shutdown requested — materialisation watch stopped");
    }

    /// One check: read, measure, judge, signal. Returns the verdict.
    pub(crate) async fn check(&self, now: DateTime<Utc>) -> Verdict {
        let read = tokio::time::timeout(READ_TIMEOUT, self.repository.progress()).await;
        let verdict = match read {
            Ok(Ok(progress)) => {
                for materialization in &progress {
                    let wait = materialization
                        .pending_for(now)
                        .unwrap_or_else(ChronoDuration::zero);
                    MaterializationMetrics::record_pending(&materialization.aggregate, wait);
                }
                Verdict::judge(&progress, now, self.settings.max_wait)
            }
            Ok(Err(e)) => Verdict::Unreadable(e.to_string()),
            Err(_) => Verdict::Unreadable(format!("no answer within {} s", READ_TIMEOUT.as_secs())),
        };
        MaterializationMetrics::record_check(verdict.label());

        if let Some(heartbeat) = &self.heartbeat {
            match verdict.reason(self.settings.max_wait) {
                None => heartbeat.success().await,
                Some(reason) => heartbeat.failure(&reason).await,
            }
        }
        verdict
    }

    /// Speak when the state changes, not on every tick: in development the
    /// scheduler is off, every aggregate is late for good, and a warning every
    /// ten minutes would bury everything else.
    fn log_change(&self, previous: Option<&Verdict>, verdict: &Verdict) {
        if previous.map(Verdict::state) == Some(verdict.state()) {
            return;
        }
        let reason = verdict.reason(self.settings.max_wait).unwrap_or_default();
        match verdict {
            Verdict::OnTime => info!("every continuous aggregate is materialised on time"),
            Verdict::Late(_) => warn!(%reason, "a continuous aggregate is not being materialised"),
            Verdict::Unreadable(_) => {
                warn!(%reason, "the materialisation progress could not be read")
            }
        }
    }
}

#[cfg(test)]
#[path = "materialization_watch_tests.rs"]
mod tests;
