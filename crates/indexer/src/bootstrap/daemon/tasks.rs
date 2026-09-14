//! What a task of the daemon is, how many transactions one may persist at
//! once, and what its result means once it has ended.

use crate::{
    application::{
        reporter::NetworkStatusReporter,
        services::TransactionProcessor,
        source::{IngestedTransaction, TransactionSource},
        workers::IndexerWorker,
    },
    error::{IndexerWorkerError, SourceError, TaskEnd},
};
use std::{convert::Infallible, sync::Arc};
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

/// Spawn the ingestion task — whatever shape the source's own graph has.
pub(super) fn spawn_source_task(
    source: Arc<dyn TransactionSource>,
    tx: mpsc::Sender<IngestedTransaction>,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), SourceError>> {
    tokio::spawn(async move { source.run(tx, shutdown).await })
}

/// How many connections the pool's *other* in-process users may need while the
/// indexer worker is running.
///
/// One today: [`NetworkStatusReporter`] upserts a snapshot on every tick.
/// `WatchedPoolService` is not counted — it restores subscriptions once, before
/// the worker is spawned, and never touches the pool again.
const CONNECTIONS_RESERVED: u32 = 1;

/// How many transactions may be persisted concurrently.
///
/// # ⚠️ The mechanism, stated correctly
///
/// A task does **not** hold a connection for its lifetime. Every repository
/// call executes against `&PgPool`, so sqlx takes a connection per *statement*
/// and returns it — nothing in `crates/persistence` opens a transaction or
/// calls `acquire`. What makes a task-count bound a connection bound is
/// something else: **an index task issues one statement at a time**. Its
/// events are persisted in a sequential loop, so *n* tasks put at most *n*
/// statements in flight, and capping tasks at `pool − 1` leaves the pool one
/// slot for the reporter.
///
/// ⚠️ **And that invariant is not enforced anywhere.** The first sub-persistor
/// that runs two repository calls under `tokio::join!` doubles a task's
/// concurrent statements and silently reinstates the starvation this bound
/// exists to prevent. What that starvation costs has changed: until 11
/// September 2026 the reporter propagated its tick errors, so one
/// `acquire_timeout` stopped the process by way of its health probe. A failed
/// tick is now counted and skipped, so the reporter only goes quiet — its
/// `network_status.observed_at` freezes and
/// `yog_indexer_network_status_tick_failures_total{reason="persistence"}`
/// climbs — while the index tasks queue on the same pool, and *their*
/// `acquire_timeout`s are what lose rows. Whoever parallelises a persist owes
/// this line a second look.
///
/// Read from the pool that was opened, not from
/// [`yog_persistence::Database::DEFAULT_MAX_CONNECTIONS`], so that sizing the
/// pool differently resizes this too instead of silently parting ways with it.
/// It takes the number rather than the `Database` so the boundary is a plain
/// unit test.
///
/// # Errors
///
/// ⚠️ **Refuses a pool too small to reserve from, rather than clamping to one.**
/// An earlier version returned `.max(1)`, which defeated the reservation in the
/// exact case this function exists to prevent: a pool of one hands its only
/// connection to an index task. Clamping cannot be right here —
/// `Semaphore::new(0)` would deadlock instead — so the only honest answers are
/// "refuse" or "open a bigger pool", and a configuration that cannot work
/// should say so at startup rather than five seconds into a busy minute.
pub(super) fn index_concurrency(max_connections: u32) -> anyhow::Result<usize> {
    anyhow::ensure!(
        max_connections > CONNECTIONS_RESERVED,
        "database pool holds {max_connections} connection(s), and {CONNECTIONS_RESERVED} must \
         stay free for the network status reporter — the indexer would have none left to \
         persist with. Open the pool with more connections."
    );
    Ok((max_connections - CONNECTIONS_RESERVED) as usize)
}

/// Spawn the indexer worker task.
///
/// Per-transaction failures stay inside the worker (logged, counted, not
/// propagated). Only loop-level failures reach the returned `JoinHandle`
/// and bubble up to `Daemon::run`.
pub(super) fn spawn_indexer_task(
    processor: Arc<TransactionProcessor>,
    rx: mpsc::Receiver<IngestedTransaction>,
    max_concurrent: usize,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), IndexerWorkerError>> {
    let worker = IndexerWorker::new(processor, max_concurrent);
    tokio::spawn(async move { worker.run(rx, shutdown).await })
}

/// Spawn the network status reporter task.
///
/// It cannot return an error — a failed tick is counted and skipped inside —
/// so the only way this handle ends the daemon from `run`'s `select!` is a
/// panic, which is a bug and should stop things.
pub(super) fn spawn_network_status_reporter_task(
    reporter: NetworkStatusReporter,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), Infallible>> {
    tokio::spawn(async move { reporter.run(shutdown).await })
}

// ── Task result handling ─────────────────────────────────────────────────────

/// Normalise the result of a spawned task into a loggable anyhow::Result.
///
/// Distinguishes four cases: clean stop, task error, task panic, and a task
/// destroyed before it could answer — see [`TaskEnd`] for why the last two are
/// one type in `tokio` and must not be one here.
pub(super) fn handle_task_result<E>(
    result: Result<Result<(), E>, tokio::task::JoinError>,
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

#[cfg(test)]
#[path = "tasks_tests.rs"]
mod tests;
