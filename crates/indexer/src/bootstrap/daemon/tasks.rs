//! The daemon's tasks, and how many transactions one may persist at once; what a task's result
//! means is `yog_bootstrap::shutdown`'s.

use crate::{
    application::{
        services::TransactionProcessor,
        source::{IngestedTransaction, TransactionSource},
        workers::{IndexerWorker, IngestionAlarm, NetworkStatusReporter},
    },
    error::{IndexerWorkerError, SourceError},
};
use std::{convert::Infallible, sync::Arc};
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

/// Spawn the ingestion task, whatever shape the source's own graph has.
pub(super) fn spawn_source_task(
    source: Arc<dyn TransactionSource>,
    tx: mpsc::Sender<IngestedTransaction>,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), SourceError>> {
    tokio::spawn(async move { source.run(tx, shutdown).await })
}

/// Connections the pool's other in-process users need while the indexer worker runs: the
/// [`NetworkStatusReporter`]'s tick. `WatchedPoolService` runs once, before the worker.
const CONNECTIONS_RESERVED: u32 = 1;

/// The opened pool minus what is reserved; refuses a pool too small to reserve from.
/// ⚠️ Sound only while an index task issues one statement at a time: a `tokio::join!` of two
/// repository calls in a sub-persistor would double it and starve the pool.
pub(super) fn index_concurrency(max_connections: u32) -> anyhow::Result<usize> {
    anyhow::ensure!(
        max_connections > CONNECTIONS_RESERVED,
        "database pool holds {max_connections} connection(s), and {CONNECTIONS_RESERVED} must \
         stay free for the network status reporter — the indexer would have none left to \
         persist with. Open the pool with more connections."
    );
    Ok((max_connections - CONNECTIONS_RESERVED) as usize)
}

/// Spawn the indexer worker task; only loop-level failures reach the returned handle.
pub(super) fn spawn_indexer_task(
    processor: Arc<TransactionProcessor>,
    rx: mpsc::Receiver<IngestedTransaction>,
    max_concurrent: usize,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), IndexerWorkerError>> {
    let worker = IndexerWorker::new(processor, max_concurrent);
    tokio::spawn(async move { worker.run(rx, shutdown).await })
}

/// Spawn the network status reporter task; it cannot return an error, so only a panic ends the daemon here.
pub(super) fn spawn_network_status_reporter_task(
    reporter: NetworkStatusReporter,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), Infallible>> {
    tokio::spawn(async move { reporter.run(shutdown).await })
}

/// Spawn the ingestion alarm task; a failed check is the check's `/fail`, so only a panic ends the daemon here.
pub(super) fn spawn_ingestion_alarm_task(
    alarm: IngestionAlarm,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), Infallible>> {
    tokio::spawn(async move { alarm.run(shutdown).await })
}

#[cfg(test)]
#[path = "tasks_tests.rs"]
mod tests;
