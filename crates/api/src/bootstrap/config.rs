use std::net::SocketAddr;

use axum::http::HeaderValue;
use yog_bootstrap::{ConfigError, SecretUrl, required};

#[derive(Clone)]
pub(crate) struct Config {
    pub(crate) database_url: SecretUrl,
    pub(crate) bind_addr: SocketAddr,
    /// Browser origins allowed to call the API (CORS). The dashboard
    /// talks to the API directly from the browser, so its origin must
    /// be listed here. Server-side (SSR) calls bypass CORS entirely.
    pub(crate) cors_allowed_origins: Vec<HeaderValue>,
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
            database_url: SecretUrl::new(required("DATABASE_URL_API")?),
            bind_addr,
            cors_allowed_origins,
        })
    }
}

/// Parse the comma-separated `API_CORS_ALLOWED_ORIGINS` value into the
/// list of origins the CORS layer will allow.
///
/// Each entry must be a full origin (`scheme://host[:port]`) — the exact
/// string a browser sends in the `Origin` header. Surrounding whitespace
/// is trimmed and empty entries are skipped, so a trailing comma is
/// harmless. An effectively empty list is rejected: an API reachable
/// from *no* browser origin is almost certainly a misconfiguration, and
/// failing loud at boot beats opaque CORS errors surfacing in the
/// browser later.
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
