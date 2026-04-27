//! Repository-layer error type, returned by all repository traits.
//!
//! This error is intentionally agnostic of the persistence backend
//! (Postgres, in-memory, mock). Concrete implementations map their
//! native errors (e.g. `sqlx::Error`) onto these variants. Callers in
//! the application layer convert `RepositoryError` into their own
//! domain-specific error type via `From`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepositoryError {
    /// The underlying storage backend failed (DB unreachable, connection
    /// dropped, query execution error). Caller should generally retry or
    /// surface as an infrastructure failure.
    #[error("repository backend failure: {0}")]
    Backend(String),

    /// Data already in the store violates an expected invariant
    /// (corrupted row, malformed enum value, unparseable address).
    /// Indicates a bug, a manual DB edit, or a schema/code mismatch —
    /// not a transient failure.
    #[error("repository data integrity issue: {0}")]
    Integrity(String),

    /// The requested entity was not found. Use this only when absence
    /// is exceptional (e.g. an explicit `find_by_id` that was expected
    /// to succeed). Methods that legitimately return optional results
    /// should return `Option<T>`, not this variant.
    #[error("not found: {0}")]
    NotFound(String),

    /// A unique constraint or business invariant was violated by the
    /// write (duplicate key, foreign key violation when relevant).
    #[error("conflict: {0}")]
    Conflict(String),

    /// The operation exceeded its time budget. Distinct from `Backend`
    /// because timeouts are often retryable with different semantics.
    #[error("timeout: {0}")]
    Timeout(String),
}

pub type RepositoryResult<T> = Result<T, RepositoryError>;
