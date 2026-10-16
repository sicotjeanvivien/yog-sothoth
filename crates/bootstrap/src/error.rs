use thiserror::Error;

/// Errors of every binary's `Config::load`: a variable missing, one malformed, or
/// several valid alone that cannot be used together.
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

    /// Values each accepted, whose *combination* cannot be honoured; `detail` names
    /// the variables and what to do instead, never a secret's value.
    #[error("unsupported configuration: {detail}")]
    UnsupportedCombination { detail: String },
}
