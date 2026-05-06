pub mod database_error;

use thiserror::Error;
use yog_core::error::RepositoryError;

/// Errors raised by the persistence layer.
///
/// Internally the layer talks sqlx; externally it must surface
/// `RepositoryError` (defined in `yog-core`) so callers (indexer, api) can
/// stay infrastructure-agnostic. The `From<PersistenceError> for RepositoryError`
/// conversion is what bridges the two.
#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("database connection failed: {0}")]
    Connection(#[from] sqlx::Error),

    #[error("numeric conversion failed: {0}")]
    NumericConversion(String),

    #[error("row decode failed: {0}")]
    Decode(String),
}

impl From<PersistenceError> for RepositoryError {
    fn from(err: PersistenceError) -> Self {
        match err {
            PersistenceError::Connection(e) => RepositoryError::Backend(e.to_string()),
            PersistenceError::NumericConversion(msg) => RepositoryError::Conversion(msg),
            PersistenceError::Decode(msg) => RepositoryError::Decode(msg),
        }
    }
}
