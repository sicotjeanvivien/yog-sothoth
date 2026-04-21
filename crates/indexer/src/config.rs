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
}

impl Config {
    pub(crate) fn load() -> anyhow::Result<Self> {
        dotenv().ok();

        Ok(Self {
            database_url: SecretUrl::new(required("DATABASE_URL")?),
            solana_rpc_ws: SecretUrl::new(required("SOLANA_RPC_WS")?),
            solana_rpc_http: SecretUrl::new(required("SOLANA_RPC_HTTP")?),
        })
    }
}

fn required(key: &str) -> Result<String, ConfigError> {
    env::var(key).map_err(|_| ConfigError::MissingVariable(key.to_string()))
}
