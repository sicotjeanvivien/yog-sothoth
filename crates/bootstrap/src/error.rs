use thiserror::Error;

/// Errors raised while loading configuration from the environment.
///
/// Every binary's `Config::load` returns this type. The variants cover
/// the only two failure modes that exist at this stage: a required
/// variable is missing, or a present variable is malformed.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("required environment variable `{0}` is not set")]
    MissingVariable(String),

    #[error("invalid value for `{key}`: got `{value}`, expected {expected}")]
    InvalidValue {
        key: String,
        value: String,
        expected: &'static str,
    },
}
