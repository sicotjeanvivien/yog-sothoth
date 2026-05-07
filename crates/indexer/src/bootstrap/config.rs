use yog_bootstrap::{ConfigError, SecretUrl, parse_required_bool, parse_required_u32, required};

pub(crate) struct Config {
    pub(crate) database_url: SecretUrl,
    pub(crate) solana_rpc_ws: SecretUrl,
    pub(crate) solana_rpc_http: SecretUrl,
    pub(crate) worker_max_retries: u32,
    pub(crate) mode_protocol_centric: bool,
}

impl Config {
    pub(crate) fn load() -> Result<Self, ConfigError> {
        Ok(Self {
            database_url: SecretUrl::new(required("DATABASE_URL_INDEXER")?),
            solana_rpc_ws: SecretUrl::new(required("SOLANA_RPC_WS")?),
            solana_rpc_http: SecretUrl::new(required("SOLANA_RPC_HTTP")?),
            worker_max_retries: parse_required_u32("RPC_WORKER_MAX_RETRIES")?,
            mode_protocol_centric: parse_required_bool("MODE_PROTOCOL_CENTRIC")?,
        })
    }
}
