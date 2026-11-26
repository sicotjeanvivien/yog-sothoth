use thiserror::Error;

use crate::RepositoryError;

/// Failure of a single detector tick.
#[derive(Debug, Error)]
pub enum DetectorError {
    /// A read from one of the detector's repositories failed.
    #[error("detector read failed: {0}")]
    Repository(#[from] RepositoryError),

    /// The data was read but could not be evaluated: a data problem, not a backend one.
    #[error("detector evaluation failed: {0}")]
    Evaluation(String),
}
