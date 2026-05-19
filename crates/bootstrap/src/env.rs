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

#[cfg(test)]
mod tests {
    use super::*;

    // SAFETY-NOTE on env tests: tests run in parallel by default, and
    // `env::set_var` is process-global. Tests that mutate the
    // environment must use unique key names to avoid interfering with
    // each other.

    #[test]
    fn required_returns_value_when_present() {
        // SAFETY: unique key, isolated from other tests
        unsafe {
            env::set_var("YOG_TEST_REQUIRED_PRESENT", "value");
        }
        assert_eq!(required("YOG_TEST_REQUIRED_PRESENT").unwrap(), "value");
    }

    #[test]
    fn required_fails_when_absent() {
        let err = required("YOG_TEST_REQUIRED_ABSENT").unwrap_err();
        assert!(matches!(err, ConfigError::MissingVariable(_)));
    }

    #[test]
    fn required_fails_when_empty() {
        // SAFETY: unique key, isolated from other tests
        unsafe {
            env::set_var("YOG_TEST_REQUIRED_EMPTY", "");
        }
        let err = required("YOG_TEST_REQUIRED_EMPTY").unwrap_err();
        assert!(matches!(err, ConfigError::MissingVariable(_)));
    }

    #[test]
    fn parse_required_bool_accepts_true_false_case_insensitive() {
        // SAFETY: unique keys, isolated from other tests
        unsafe {
            env::set_var("YOG_TEST_BOOL_T", "TRUE");
            env::set_var("YOG_TEST_BOOL_F", "False");
        }
        assert!(parse_required_bool("YOG_TEST_BOOL_T").unwrap());
        assert!(!parse_required_bool("YOG_TEST_BOOL_F").unwrap());
    }

    #[test]
    fn parse_required_bool_rejects_garbage() {
        // SAFETY: unique key, isolated from other tests
        unsafe {
            env::set_var("YOG_TEST_BOOL_BAD", "yes");
        }
        let err = parse_required_bool("YOG_TEST_BOOL_BAD").unwrap_err();
        assert!(matches!(err, ConfigError::InvalidValue { .. }));
    }
}
