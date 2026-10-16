//! Error types for the `yog-context` daemon; `WorkerError` is what the supervised worker tasks return.

use thiserror::Error;

/// Failure inside a supervised worker loop.
#[derive(Debug, Error)]
pub enum WorkerError {
    /// A persistence operation failed.
    #[error("worker persistence failure")]
    Persistence(#[from] yog_core::RepositoryError),
}
