//! The error of every repository trait, agnostic of the backend.

use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum RepositoryError {
    /// The storage backend failed: unreachable, connection dropped, query error.
    #[error("repository backend failure: {0}")]
    Backend(String),

    /// Stored data violates an invariant (malformed enum, unparseable address):
    /// a bug or a schema/code mismatch, not a transient failure.
    #[error("repository data integrity issue: {0}")]
    Integrity(String),

    /// An entity expected to exist was not found; a legitimate absence is an `Option`.
    #[error("not found: {0}")]
    NotFound(String),

    /// The write violated a unique constraint or an invariant.
    #[error("conflict: {0}")]
    Conflict(String),

    /// The operation exceeded its time budget.
    #[error("timeout: {0}")]
    Timeout(String),

    #[error("Conversion: {0}")]
    Conversion(String),

    #[error("Conversion: {0}")]
    Decode(String),
}

pub type RepositoryResult<T> = Result<T, RepositoryError>;
