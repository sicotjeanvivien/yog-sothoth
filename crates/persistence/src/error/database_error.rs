use thiserror::Error;
use yog_core::RepositoryError;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("backend failure: {0}")]
    Backend(String),

    #[error("data integrity issue: {0}")]
    Integrity(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("timeout: {0}")]
    Timeout(String),

    #[error("Conversion: {0}")]
    Conversion(String),

    #[error("Conversion: {0}")]
    Decode(String),
}

impl From<RepositoryError> for DatabaseError {
    fn from(value: RepositoryError) -> Self {
        match value {
            RepositoryError::Backend(msg) => Self::Backend(msg),
            RepositoryError::Integrity(msg) => Self::Integrity(msg),
            RepositoryError::NotFound(msg) => Self::NotFound(msg),
            RepositoryError::Conflict(msg) => Self::Conflict(msg),
            RepositoryError::Timeout(msg) => Self::Timeout(msg),
            RepositoryError::Conversion(msg) => Self::Conversion(msg),
            RepositoryError::Decode(msg) => Self::Decode(msg),
        }
    }
}
