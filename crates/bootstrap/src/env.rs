use std::env;

use crate::error::ConfigError;

/// Read a required environment variable. Returns `MissingVariable` if
/// the key is absent or empty.
///
/// Empty strings are treated as missing on purpose — a `.env` line like
/// `DATABASE_URL=` is almost certainly an oversight, and silently
/// returning an empty value would propagate the bug deeper into the
/// system before failing.
pub fn required(key: &str) -> Result<String, ConfigError> {
    match env::var(key) {
        Ok(v) if !v.is_empty() => Ok(v),
        _ => Err(ConfigError::MissingVariable(key.to_string())),
    }
}

/// Read a required environment variable and parse it as a `u32`.
///
/// Fails with `MissingVariable` if absent, `InvalidValue` if present
/// but unparseable. Silent fallback to a default would mask typos in
/// the `.env`.
pub fn parse_required_u32(key: &str) -> Result<u32, ConfigError> {
    let raw = required(key)?;
    raw.parse::<u32>().map_err(|_| ConfigError::InvalidValue {
        key: key.to_string(),
        value: raw,
        expected: "a non-negative integer (u32)",
    })
}

/// Read a required environment variable and parse it as a `bool`.
///
/// Accepts the literals `true` and `false` (case-insensitive). Anything
/// else is rejected — a loud failure is preferable to a silent coercion
/// to `false`.
pub fn parse_required_bool(key: &str) -> Result<bool, ConfigError> {
    let raw = required(key)?;
    match raw.to_ascii_lowercase().as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(ConfigError::InvalidValue {
            key: key.to_string(),
            value: raw,
            expected: "true or false",
        }),
    }
}

/// Read an optional `u64` environment variable, falling back to
/// `default` when unset. A present-but-unparseable value is an error.
pub fn duration_var(key: &'static str, default: u64) -> Result<u64, ConfigError> {
    match std::env::var(key) {
        Err(_) => Ok(default),
        Ok(raw) => raw.parse::<u64>().map_err(|_| ConfigError::InvalidValue {
            key: key.to_string(),
            value: raw,
            expected: "a integer (u64)",
        }),
    }
}

/// A configuration value read from the environment as one of a closed
/// set of names — the alternative to a `bool` whenever the axis has, or
/// may one day have, more than two states.
///
/// Implementors describe *what* their names are. They do not deal with
/// case, nor with what an unknown value costs: `parse_required_enum`
/// owns both, so the rule is written once instead of being restated —
/// and forgotten — in every enum.
pub trait EnvEnum: Sized {
    /// The accepted values, phrased as they appear in the error message
    /// (e.g. `"rpc or grpc"`).
    const EXPECTED: &'static str;

    /// Map one accepted name to its variant.
    ///
    /// `value` arrives **already trimmed and lowercased** — match on bare
    /// lowercase literals only, or the variant becomes unreachable.
    fn from_env_value(value: &str) -> Option<Self>;
}

/// Read a required environment variable and parse it as an [`EnvEnum`].
///
/// Fails with `MissingVariable` if absent or empty, `InvalidValue` if
/// present but not one of the accepted names — which are listed back to
/// the operator, since a config that dies at startup should say what it
/// wanted instead of what it got.
///
/// Surrounding whitespace is trimmed for the same reason the value is
/// lowercased, and it is not cosmetic: this repository's `.env` has CRLF
/// line endings, and a `\r` that ever reached here would produce
/// ``invalid value for `X`: got `pools`, expected … or pools`` — a
/// refusal whose cause is an invisible byte.
pub fn parse_required_enum<T: EnvEnum>(key: &str) -> Result<T, ConfigError> {
    let raw = required(key)?;
    T::from_env_value(&raw.trim().to_ascii_lowercase()).ok_or_else(|| ConfigError::InvalidValue {
        key: key.to_string(),
        value: raw,
        expected: T::EXPECTED,
    })
}

#[cfg(test)]
#[path = "env_tests.rs"]
mod tests;
