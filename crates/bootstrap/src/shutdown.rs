//! What a requested stop is, for every daemon that has one — each rule here is
//! applied at several sites, and written once:
//!
//! - which signals mean "stop" ([`shutdown_signal`]);
//! - what a finished task's result says, panic and cancellation told apart
//!   ([`TaskEnd`], [`handle_task_result`]);
//! - what the stop is worth once every stage has answered — or has failed to,
//!   inside [`SHUTDOWN_GRACE`] ([`Stop`]).
//!
//! ⚠️ These lines log under `yog_bootstrap`, not the binary's target: a
//! `RUST_LOG` of per-crate directives alone would silence them, and a torn stop
//! would read like a clean one. `runtime::build_filter` keeps the target
//! audible, and owns that rule.

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

/// Normalise the result of a spawned task into a loggable [`anyhow::Result`].
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
/// ⚠️ **One value for every daemon.** Work longer than Docker's ten seconds
/// admits — `yog-context`'s price tick, whose Jupiter requests leave 1.1 s
/// apart — cannot be covered by any grace: it has to hear the stop and end
/// early, as that tick does, dropping the request in flight. Tuning the number
/// per binary would buy nothing and hand the next daemon a knob to set wrong;
/// what an overrun gets instead is a name in the logs, which is what [`Stop`]
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
    /// ⚠️ The deadline is computed here, not handed in: the grace waited and
    /// the grace named in the logs are one value.
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
    /// name: it is still running, and `main` dropping the runtime destroys it
    /// mid-flight.
    ///
    /// ⚠️ **Stages are served in call order, and that order is a decision.**
    /// One deadline spent in turn means the first stage waited on can eat all
    /// of it; a stage reached afterwards still gets one poll — `timeout_at`
    /// polls the task before the clock, so an answer already given is collected
    /// — but no wait for one that has not come. Call first the stage whose
    /// work is *lost* rather than merely abandoned.
    ///
    /// ⚠️ The stage the `select!` already collected is skipped here, and
    /// nowhere else: its handle is polled to completion, and `tokio` panics on
    /// a `JoinHandle` polled again. Callers list every stage, in the order they
    /// want them served.
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
/// ⚠️ **Both signals: the second is the one production sends.** Ctrl-C
/// (SIGINT) is what a developer types; `docker compose stop`, a Kubernetes
/// eviction and `systemctl stop` send **SIGTERM**.
///
/// ⚠️ **In a container, an unheard SIGTERM is ignored.** Every compose service
/// `exec`s its binary, so the daemon is PID 1, and the kernel does not apply a
/// signal's default action to PID 1: without a handler nothing stops, nothing
/// is logged, and Docker sends SIGKILL ten seconds later.
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
