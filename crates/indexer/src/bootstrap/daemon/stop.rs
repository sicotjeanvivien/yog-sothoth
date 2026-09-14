//! What a requested stop is worth once every stage has answered — or has
//! failed to, inside [`SHUTDOWN_GRACE`].

use std::time::Duration;
use tokio::{task::JoinHandle, time::Instant};

use super::tasks::handle_task_result;

/// The names the three tasks answer to — in the logs, and in the list of what
/// outlived the grace. Named once because each is written at two sites.
pub(super) const SOURCE: &str = "transaction source";
pub(super) const INDEXER: &str = "indexer worker";
pub(super) const REPORTER: &str = "network status reporter";

/// How long `Daemon::run` waits for its tasks once the token has fired.
///
/// ⚠️ **Under Docker's ten seconds, not at them.** `docker-compose.yml` sets no
/// `stop_grace_period`, so the default applies: SIGKILL ten seconds after the
/// SIGTERM. A grace of ten would expire exactly when the process is killed, and
/// the log line saying which stage overran would never be written — the one
/// case where the timeout has something to say is the one where it stays
/// silent.
pub(super) const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// What the stop has learned so far: the verdict to return, and who never
/// answered.
///
/// It exists because the verdict is **not** whatever the `select!` produced.
/// Its cancellation arm reports that a stop was asked for, which is not an
/// outcome, and a stage that failed can still be in the middle of stopping when
/// that arm fires — so the error arrives afterwards, through the drain. First
/// error wins; a stage stopping cleanly after one never erases it.
pub(super) struct Stop {
    outcome: anyhow::Result<()>,
    still_running: Vec<&'static str>,
}

impl Stop {
    pub(super) fn new(first: anyhow::Result<()>) -> Self {
        Self {
            outcome: first,
            still_running: Vec::new(),
        }
    }

    /// Wait for `handle`, but no longer than `deadline`.
    ///
    /// A task that ends in time has its result logged, and adopted as the
    /// verdict if nothing has failed yet. A task that does not is recorded by
    /// name: it is still running, and it will be destroyed mid-flight when
    /// `main` drops the runtime. Keeping the name — rather than only logging it
    /// — is what makes "the overrun names its task" something a test can
    /// falsify.
    pub(super) async fn settle<E>(
        &mut self,
        name: &'static str,
        handle: &mut JoinHandle<Result<(), E>>,
        deadline: Instant,
    ) where
        E: std::error::Error + Send + Sync + 'static,
    {
        match tokio::time::timeout_at(deadline, handle).await {
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
    pub(super) fn finish(self) -> anyhow::Result<()> {
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

#[cfg(test)]
#[path = "stop_tests.rs"]
mod tests;
