use std::net::SocketAddr;
use std::time::Duration;

use axum::http::HeaderValue;
use yog_bootstrap::{ConfigError, SecretUrl, duration_var, required, required_secret_url};

/// How often the stream poller checks the feed, in seconds (`API_SIGNAL_STREAM_POLL_SECS`).
const DEFAULT_SIGNAL_STREAM_POLL_SECS: u64 = 3;

#[derive(Clone)]
pub(crate) struct Config {
    pub(crate) database_url: SecretUrl,
    pub(crate) bind_addr: SocketAddr,
    /// Browser origins allowed to call the API: the dashboard's must be listed.
    pub(crate) cors_allowed_origins: Vec<HeaderValue>,
    /// Cadence of the signal-stream poller feeding `/api/signals/stream`.
    pub(crate) signal_stream_poll: Duration,
}

impl Config {
    pub(crate) fn load() -> Result<Self, ConfigError> {
        let bind_addr_raw = required("API_BIND_ADDR")?;
        let bind_addr =
            bind_addr_raw
                .parse::<SocketAddr>()
                .map_err(|_| ConfigError::InvalidValue {
                    key: "API_BIND_ADDR".to_string(),
                    value: bind_addr_raw,
                    expected: "a host:port socket address (e.g. 127.0.0.1:3000)",
                })?;

        let cors_allowed_origins = parse_cors_origins(&required("API_CORS_ALLOWED_ORIGINS")?)?;

        Ok(Self {
            database_url: required_secret_url("DATABASE_URL_API")?,
            bind_addr,
            cors_allowed_origins,
            signal_stream_poll: Duration::from_secs(duration_var(
                "API_SIGNAL_STREAM_POLL_SECS",
                DEFAULT_SIGNAL_STREAM_POLL_SECS,
            )?),
        })
    }
}

/// Parses `API_CORS_ALLOWED_ORIGINS`: full origins, comma-separated, blanks skipped. An empty
/// list fails at boot rather than as opaque CORS errors in the browser.
fn parse_cors_origins(raw: &str) -> Result<Vec<HeaderValue>, ConfigError> {
    let origins = raw
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            HeaderValue::from_str(entry).map_err(|_| ConfigError::InvalidValue {
                key: "API_CORS_ALLOWED_ORIGINS".to_string(),
                value: entry.to_string(),
                expected: "a comma-separated list of origins (e.g. https://yog-scope.xyz)",
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    if origins.is_empty() {
        return Err(ConfigError::InvalidValue {
            key: "API_CORS_ALLOWED_ORIGINS".to_string(),
            value: raw.to_string(),
            expected: "at least one origin (e.g. https://yog-scope.xyz)",
        });
    }

    Ok(origins)
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
