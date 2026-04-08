use dotenvy::dotenv;
use std::env;

/// Application configuration loaded from environment variables.
pub(crate) struct Config {
    /// TimescaleDB connection URL.
    pub(crate) database_url: String,
    /// Solana RPC WebSocket URL.
    pub(crate) solana_rpc_ws: String,
}

impl Config {
    /// Load configuration from environment variables.
    /// Panics at startup if a required variable is missing.
    pub(crate) fn load() -> Self {
        dotenv().ok();

        Self {
            database_url: required("DATABASE_URL"),
            solana_rpc_ws: required("SOLANA_RPC_WS"),
        }
    }
}

/// Retrieve a required environment variable.
/// Panics with a clear message if the variable is missing.
fn required(key: &str) -> String {
    env::var(key).unwrap_or_else(|_| panic!("missing required environment variable: {key}"))
}