use dotenvy::dotenv;
use std::env;

use crate::error::ConfigError;

/// Application configuration loaded from environment variables.
pub(crate) struct Config {
    /// TimescaleDB connection URL.
    pub(crate) database_url: String,
    /// Solana RPC WebSocket URL.
    pub(crate) solana_rpc_ws: String,
    /// Solana RPC http url
    pub(crate) solana_rpc_http: String,
}

impl Config {
    /// Load configuration from environment variables.
    /// Panics at startup if a required variable is missing.
    pub(crate) fn load() -> anyhow::Result<Self> {
        dotenv().ok();

        Ok(Self {
            database_url: required("DATABASE_URL")?,
            solana_rpc_ws: required("SOLANA_RPC_WS")?,
            solana_rpc_http: required("SOLANA_RPC_HTTP")?,
        })
    }
}

/// Retrieve a required environment variable.
/// Panics with a clear message if the variable is missing.
fn required(key: &str) -> Result<String, ConfigError> {
    env::var(key).map_err(|_| ConfigError::MissingVariable(key.to_string()))
}
