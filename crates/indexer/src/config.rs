pub mod secret_url;

use dotenvy::dotenv;
use std::env;

use crate::config::secret_url::SecretUrl;
use crate::error::ConfigError;

/// Application configuration loaded from environment variables.
pub(crate) struct Config {
    /// TimescaleDB connection URL.
    pub(crate) database_url: SecretUrl,
    /// Solana RPC WebSocket URL.
    pub(crate) solana_rpc_ws: SecretUrl,
    /// Solana RPC HTTP URL.
    pub(crate) solana_rpc_http: SecretUrl,
    /// Maximum number of consecutive retries a worker performs before giving up.
    pub(crate) worker_max_retries: u32,
    /// Indexing mode: `true` for protocol-centric (subscribe to programs),
    /// `false` for pool-centric (subscribe to individual pool accounts).
    /// See `RpcListener` for the rationale and the migration plan.
    pub(crate) mode_protocol_centric: bool,
}

impl Config {
    pub(crate) fn load() -> Result<Self, ConfigError> {
        dotenv().ok();

        Ok(Self {
            database_url: SecretUrl::new(required("DATABASE_URL")?),
            solana_rpc_ws: SecretUrl::new(required("SOLANA_RPC_WS")?),
            solana_rpc_http: SecretUrl::new(required("SOLANA_RPC_HTTP")?),
            worker_max_retries: parse_required_u32("RPC_WORKER_MAX_RETRIES")?,
            mode_protocol_centric: parse_requires_bool("MODE_PROTOCOL_CENTRIC")?,
        })
    }
}

fn required(key: &str) -> Result<String, ConfigError> {
    env::var(key).map_err(|_| ConfigError::MissingVariable(key.to_string()))
}

fn parse_required_u32(key: &str) -> Result<u32, ConfigError> {
    parse_u32(key, required(key)?)
}

fn parse_requires_bool(key: &str) -> Result<bool, ConfigError> {
    parse_bool(key, required(key)?)
}

/// Parse an environment variable value into a `u32`. Returns an explicit
/// error if the value is present but cannot be parsed — silent fallback to
/// a default would mask typos in the `.env`.
fn parse_u32(key: &str, raw: String) -> Result<u32, ConfigError> {
    raw.parse::<u32>().map_err(|_| ConfigError::InvalidValue {
        key: key.to_string(),
        value: raw,
        expected: "a non-negative integer (u32)",
    })
}

/// Parse an environment variable value into a `bool`. Accepts the literals
/// "true" and "false" (case-insensitive). Anything else is rejected — we
/// prefer a loud failure over a silently coerced `false`.
fn parse_bool(key: &str, raw: String) -> Result<bool, ConfigError> {
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
