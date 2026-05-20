//! Error types for the `yog-context` daemon.
//!
//! One enum per concern. `WorkerError` is the type the supervised
//! worker tasks return — it is a `thiserror` enum, so it satisfies
//! the `Error + Send + Sync + 'static` bound the task supervisor
//! expects.

use thiserror::Error;

/// Failure inside a supervised worker loop.
///
/// STUB-level for commit 1: the variants below are the ones the real
/// workers (commits 2 and 3) will produce. Kept here now so the
/// worker `run` signatures are stable across the three commits.
#[derive(Debug, Error)]
pub enum WorkerError {
    /// A persistence operation failed.
    #[error("worker persistence failure")]
    Persistence(#[from] yog_core::RepositoryError),
}
