use thiserror::Error;

/// All errors that can occur within yog-core.
#[derive(Debug, Error)]
pub(crate) enum IndexerError {
    /// .env missing required environment variable.
    #[error("missing required environment variable: {0}")]
    ConfigError(String),
}
