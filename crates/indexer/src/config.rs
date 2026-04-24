pub mod secret_url;

use dotenvy::dotenv;
use std::env;

use crate::config::secret_url::SecretUrl;
use crate::error::ConfigError;

/// Application configuration loaded from environment variables.
pub(crate) struct Config {
    /// TimescaleDB connection URL — contient le mot de passe DB.
    pub(crate) database_url: SecretUrl,
    /// Solana RPC WebSocket URL — contient potentiellement l'api-key.
    pub(crate) solana_rpc_ws: SecretUrl,
    /// Solana RPC HTTP URL — contient potentiellement l'api-key.
    pub(crate) solana_rpc_http: SecretUrl,
    /// Worker max retries before stop
    pub(crate) worker_max_retries: u32,
}

impl Config {
    pub(crate) fn load() -> anyhow::Result<Self> {
        dotenv().ok();

        Ok(Self {
            database_url: SecretUrl::new(required("DATABASE_URL")?),
            solana_rpc_ws: SecretUrl::new(required("SOLANA_RPC_WS")?),
            solana_rpc_http: SecretUrl::new(required("SOLANA_RPC_HTTP")?),
            worker_max_retries: parse_value_to_u32(required("RPC_WORKER_MAX_RETRIES")?),
        })
    }
}

fn required(key: &str) -> Result<String, ConfigError> {
    env::var(key).map_err(|_| ConfigError::MissingVariable(key.to_string()))
}

fn parse_value_to_u32(raw: String) -> u32 {
    raw.parse::<u32>().ok().unwrap_or(10)
}
