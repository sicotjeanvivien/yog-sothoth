// ── To ADD to crates/indexer/src/error.rs ────────────────────────────
//
// Alongside RpcListenerError, DispatcherError, IndexerWorkerError.
// Same shape as the other stage errors: a thiserror enum, so it
// satisfies `handle_task_result`'s `E: Error + Send + Sync + 'static`.

use thiserror::Error;

/// Failure modes of the network status reporter.
///
/// The reporter is supervised like a pipeline stage: a failure of
/// either variant terminates the task and bubbles up to `Daemon::run`
/// via `handle_task_result`.
#[derive(Debug, Error)]
pub enum NetworkStatusReporterError {
    /// The `getSlot` RPC call failed (RPC unreachable, transport
    /// error, malformed response).
    #[error("network status reporter: getSlot RPC call failed: {0}")]
    Rpc(String),

    /// Persisting the snapshot failed. Wraps the repository error.
    #[error("network status reporter: failed to persist snapshot")]
    Persistence(#[from] yog_core::RepositoryError),
}
