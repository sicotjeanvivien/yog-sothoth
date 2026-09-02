use thiserror::Error;

/// Errors raised while loading configuration from the environment.
///
/// Every binary's `Config::load` returns this type. The variants cover
/// the three failure modes that exist at this stage: a required
/// variable is missing, a present variable is malformed, or several
/// variables are each valid on their own and cannot be used together.
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

    /// Distinct from `InvalidValue`, which names a single key: here every
    /// value is individually accepted and it is their *combination* that
    /// the process cannot honour. `detail` is expected to name the
    /// variables, their values, and what to do instead — the operator
    /// reads it in a crash log, with nothing else to go on.
    #[error("unsupported configuration: {detail}")]
    UnsupportedCombination { detail: String },
}
