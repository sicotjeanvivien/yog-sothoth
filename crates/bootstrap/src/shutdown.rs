//! What a requested stop is, for every daemon: which signals mean "stop", what a
//! finished task says, and what the stop is worth within [`SHUTDOWN_GRACE`].
//! ⚠️ These lines log under `yog_bootstrap`, not the binary's target:
//! `runtime::build_filter` keeps that target audible.

use std::time::Duration;
use tokio::{task::JoinError, task::JoinHandle, time::Instant};

/// What a [`JoinError`] actually says.
/// ⚠️ It bundles a panic and a cancellation: read as a panic, a normal stop counts as a crash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskEnd {
    /// The task's future panicked: the only failure here.
    Panicked,
    /// The task was destroyed before it finished, aborted or by the runtime stopping.
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

/// A spawned task's result as a logged [`anyhow::Result`]: clean stop, error,
/// panic, or cancelled (`Ok`, see [`TaskEnd`]).
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
/// ⚠️ Under Docker's default ten seconds before SIGKILL, or the line naming the
/// overrunning stage is never written. Longer work must hear the stop itself.
pub const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// What the stop has learned: the verdict to return, and who never answered.
/// ⚠️ Not the `select!`'s result: a stage's error can arrive during the drain.
/// The first error wins; a later clean stop never erases it.
pub struct Stop {
    outcome: anyhow::Result<()>,
    still_running: Vec<&'static str>,
    /// One absolute deadline for every stage, so waiting on them in turn is
    /// still bounded by [`SHUTDOWN_GRACE`] in total.
    deadline: Instant,
    /// The stage the `select!` already collected; `None` when the stop came from
    /// the cancellation arm.
    collected: Option<&'static str>,
}

impl Stop {
    /// Open a stop with the `select!`'s verdict and the stage it came from; start the clock.
    /// ⚠️ The deadline is computed here, so the grace waited is the grace logged.
    pub fn new(first: anyhow::Result<()>, collected: Option<&'static str>) -> Self {
        Self {
            outcome: first,
            still_running: Vec::new(),
            deadline: Instant::now() + SHUTDOWN_GRACE,
            collected,
        }
    }

    /// Wait for `handle` within what is left of the grace; a stage that overruns is named.
    /// ⚠️ Call order is a decision: the first stage can eat the grace, so call first
    /// the one whose work is *lost*. The collected stage is skipped: a re-poll panics.
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

/// Resolve when the process is asked to stop, on Ctrl-C or **SIGTERM**.
/// ⚠️ As PID 1 in a container the daemon ignores an unhandled SIGTERM: without
/// this handler nothing stops until Docker's SIGKILL.
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
