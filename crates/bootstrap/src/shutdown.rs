//! What a requested stop is, for every daemon that has one.
//!
//! Three things live here, and they are here rather than in a binary because
//! each is **one rule applied at several sites**, and a rule written twice is a
//! rule that holds at one site out of two:
//!
//! - which signals mean "stop" ([`shutdown_signal`]);
//! - what a finished task's result says, panic and cancellation told apart
//!   ([`TaskEnd`], [`handle_task_result`]);
//! - what the stop is worth once every stage has answered — or has failed to,
//!   inside [`SHUTDOWN_GRACE`] ([`Stop`]).
//!
//! ⚠️ **These lines come from `yog_bootstrap`, not from the binary**, and that
//! nearly made a stop unreadable. `EnvFilter` has no implicit global level, so
//! a `RUST_LOG` made only of per-crate directives — this repository's own —
//! printed none of `"… stopped"`, `"… panicked"` or the `warn!` naming a stage
//! destroyed mid-write, and a torn stop read exactly like a clean one. Found on
//! 14 September 2026 while measuring, by ten cycles that came back silent.
//!
//! It is a constraint now, not a warning to remember: `runtime::build_filter`
//! keeps this target audible unless the operator has said something that covers
//! it. That function owns the rule; do not restate it.
//!
//! ⚠️ **The divergence this module exists to prevent already happened.** The
//! indexer's daemon and `yog-context`'s each carried their own
//! `handle_task_result`, and the context one's doc-comment said in so many
//! words that it covered "the same three cases the indexer's covers". The
//! duplication was documented, and it diverged anyway the day the indexer
//! learned to tell a cancellation from a panic and `yog-context` did not.

use std::time::Duration;
use tokio::{task::JoinError, task::JoinHandle, time::Instant};

/// What a [`JoinError`] actually says.
///
/// `tokio` bundles two outcomes in one type and only one of them is a failure:
/// the task panicked, or it was cancelled. Reading a `JoinError` as a panic
/// reports a normal stop as a crash — and, worse, counts it as one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskEnd {
    /// The task's future panicked. A real failure, and the only one here.
    Panicked,
    /// The task was destroyed before it could finish — aborted, or caught by
    /// the runtime shutting down. Nothing failed; the work was cut short.
    Cancelled,
}

impl From<&JoinError> for TaskEnd {
    fn from(error: &JoinError) -> Self {
        if error.is_cancelled() {
            Self::Cancelled
        } else {
            Self::Panicked
        }
    }
}

/// Normalise the result of a spawned task into a loggable `anyhow::Result`.
///
/// Distinguishes four cases: clean stop, task error, task panic, and a task
/// destroyed before it could answer — see [`TaskEnd`] for why the last two are
/// one type in `tokio` and must not be one here.
pub fn handle_task_result<E>(
    result: Result<Result<(), E>, JoinError>,
    task_name: &str,
) -> anyhow::Result<()>
where
    E: std::error::Error + Send + Sync + 'static,
{
    match result {
        Ok(Ok(())) => {
            tracing::info!("{task_name} stopped");
            Ok(())
        }
        Ok(Err(e)) => {
            tracing::error!(error = %e, "{task_name} failed");
            Err(anyhow::Error::new(e))
        }
        Err(e) => match TaskEnd::from(&e) {
            TaskEnd::Panicked => {
                tracing::error!(error = %e, "{task_name} panicked");
                Err(anyhow::anyhow!("{task_name} panicked: {e}"))
            }
            TaskEnd::Cancelled => {
                tracing::debug!(error = %e, "{task_name} was cancelled before it could stop");
                Ok(())
            }
        },
    }
}

/// How long a daemon waits for its tasks once the shutdown token has fired.
///
/// ⚠️ **Under Docker's ten seconds, not at them.** `docker-compose.yml` sets no
/// `stop_grace_period` on any service, so the default applies: SIGKILL ten
/// seconds after the SIGTERM. A grace of ten would expire exactly when the
/// process is killed, and the log line saying which stage overran would never
/// be written — the one case where the timeout has something to say is the one
/// where it stays silent.
///
/// ⚠️ **One value for every daemon, and the measurement says why no other
/// value would do.** `yog-context`'s price worker takes 10.7–19.9 s per tick
/// against a rate-limiting Jupiter (measured 14 September 2026, 10 ticks), so
/// *no* grace Docker's ten seconds admits could cover it. Tuning the number per
/// binary would buy nothing and hand the next daemon a knob to set wrong; what
/// an overrun gets instead is a name in the logs, which is what [`Stop`]
/// delivers.
pub const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// What the stop has learned so far: the verdict to return, and who never
/// answered.
///
/// It exists because the verdict is **not** whatever the `select!` produced.
/// Its cancellation arm reports that a stop was asked for, which is not an
/// outcome, and a stage that failed can still be in the middle of stopping when
/// that arm fires — so the error arrives afterwards, through the drain. First
/// error wins; a stage stopping cleanly after one never erases it.
pub struct Stop {
    outcome: anyhow::Result<()>,
    still_running: Vec<&'static str>,
    /// One absolute deadline for every stage, so waiting on them in turn is
    /// still bounded by [`SHUTDOWN_GRACE`] in total.
    deadline: Instant,
    /// The stage the `select!` already collected, if it was one — see
    /// [`Stop::new`]. `None` when the stop came from the cancellation arm and
    /// every handle is still unpolled.
    collected: Option<&'static str>,
}

impl Stop {
    /// Open a stop with the verdict the `select!` produced, and start the clock.
    ///
    /// `collected` names the stage that verdict came from, so the drain can be
    /// written as a plain list of every stage — see [`Stop::settle`].
    ///
    /// ⚠️ **The deadline is computed here, not handed in.** It used to be the
    /// caller's job, next to a `finish` that printed [`SHUTDOWN_GRACE`] in its
    /// warning — one binary, so the two could not disagree. At two they could:
    /// the grace named in the logs and the grace actually waited would have
    /// been two statements of one rule. Now there is one.
    pub fn new(first: anyhow::Result<()>, collected: Option<&'static str>) -> Self {
        Self {
            outcome: first,
            still_running: Vec::new(),
            deadline: Instant::now() + SHUTDOWN_GRACE,
            collected,
        }
    }

    /// Wait for `handle`, but no longer than what is left of the grace.
    ///
    /// A task that ends in time has its result logged, and adopted as the
    /// verdict if nothing has failed yet. A task that does not is recorded by
    /// name: it is still running, and it will be destroyed mid-flight when
    /// `main` drops the runtime. Keeping the name — rather than only logging it
    /// — is what makes "the overrun names its task" something a test can
    /// falsify.
    ///
    /// ⚠️ **Stages are served in call order, and that order is a decision.**
    /// One deadline spent in turn means the first stage waited on can eat all
    /// of it; a stage reached afterwards still gets one poll — `timeout_at`
    /// polls the task before the clock, so an answer already given is collected
    /// — but no wait for one that has not come. Call first the stage whose
    /// work is *lost* rather than merely abandoned.
    ///
    /// ⚠️ **The stage the `select!` already collected is skipped here, and that
    /// is why it is skipped nowhere else.** Its handle has been polled to
    /// completion and `tokio` panics on a `JoinHandle` polled again, so every
    /// drain has to step over it. Written at the call sites, that guard was six
    /// `if`s across two daemons — one rule stated six times, which is one rule
    /// per site waiting to be forgotten by the seventh. Callers now list every
    /// stage, in the order they want them served, and say nothing about which
    /// one is spent.
    pub async fn settle<E>(&mut self, name: &'static str, handle: &mut JoinHandle<Result<(), E>>)
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        if self.collected == Some(name) {
            return;
        }
        match tokio::time::timeout_at(self.deadline, handle).await {
            Ok(result) => {
                let reported = handle_task_result(result, name);
                if self.outcome.is_ok() {
                    self.outcome = reported;
                }
            }
            Err(_elapsed) => self.still_running.push(name),
        }
    }

    /// Say who outlived the grace, then hand back the verdict.
    pub fn finish(self) -> anyhow::Result<()> {
        if !self.still_running.is_empty() {
            tracing::warn!(
                tasks = ?self.still_running,
                grace_secs = SHUTDOWN_GRACE.as_secs(),
                "shutdown grace expired — these tasks are destroyed mid-flight with the runtime"
            );
        }
        self.outcome
    }
}

/// Resolve when the process is asked to stop.
///
/// ⚠️ **Both signals, and the second one is the one production sends.**
/// Ctrl-C (SIGINT) is what a developer types; `docker compose stop`, a
/// Kubernetes eviction and `systemctl stop` all send **SIGTERM**. That was
/// `yog-context` and `yog-signals` until 14 September 2026, which is why this
/// lives here instead of in one binary's `main`.
///
/// ⚠️ **And in a container it is worse than dying.** Every compose service
/// `exec`s its binary, so the daemon is **PID 1** — and the kernel does not
/// deliver a signal's default action to PID 1. A process with no SIGTERM
/// handler therefore *ignores* it: nothing stops, nothing is logged, and Docker
/// waits its ten seconds before SIGKILL. The stop did not tear, it never
/// started.
pub async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let sigterm = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    // On non-Unix targets (e.g. Windows CI), only Ctrl-C is available.
    #[cfg(not(unix))]
    let sigterm = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c  => tracing::info!("received Ctrl-C — shutting down"),
        _ = sigterm => tracing::info!("received SIGTERM — shutting down"),
    }
}

#[cfg(test)]
#[path = "shutdown_tests.rs"]
mod tests;
