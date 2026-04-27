use thiserror::Error;

/// All errors that can occur within yog-core.
#[derive(Debug, Error)]
pub(crate) enum ConfigError {
    #[error("missing required environment variable: {0}")]
    MissingVariable(String),

    #[error("invalid value for {key}: got {value:?}, expected {expected}")]
    InvalidValue {
        key: String,
        value: String,
        expected: &'static str,
    },
}
