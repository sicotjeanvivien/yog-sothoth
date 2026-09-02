//! Daemon configuration, loaded from the environment.
//!
//! The ingestion vocabulary this reads — `INGEST_SOURCE`, `INGEST_SCOPE`, and
//! which of their couples can run — lives in [`crate::ingest`]; this module
//! only reads the environment and hands the result to the daemon.
//!
//! **Why `Config` carries a scope but no source.** The source picks *which
//! listener to build*, and there is one: `RpcListener` **is** the rpc source,
//! so asking it which source it serves could only ever answer `Rpc`. Nothing
//! downstream has a use for the value, and a field nobody reads is a field
//! nobody maintains — it would also not survive `-D warnings`, which is the
//! honest form of the same statement. `INGEST_SOURCE` is still **read and
//! validated** here, because refusing an unimplemented source at config load
//! is the whole point; it joins the struct the day a second listener gives it
//! a reader, and the dispatch that reads it belongs in `daemon::init_listener`.

use yog_bootstrap::{ConfigError, SecretUrl, parse_required_enum, parse_required_u32, required};

use crate::ingest::{IngestScope, IngestSource, check_supported};

pub(crate) struct Config {
    pub(crate) database_url: SecretUrl,
    pub(crate) solana_rpc_ws: SecretUrl,
    pub(crate) solana_rpc_http: SecretUrl,
    pub(crate) worker_max_retries: u32,
    pub(crate) scope: IngestScope,
}

impl Config {
    pub(crate) fn load() -> Result<Self, ConfigError> {
        let source = parse_required_enum::<IngestSource>("INGEST_SOURCE")?;
        let scope = parse_required_enum::<IngestScope>("INGEST_SCOPE")?;
        check_supported(source, scope)?;

        Ok(Self {
            database_url: SecretUrl::new(required("DATABASE_URL_INDEXER")?),
            solana_rpc_ws: SecretUrl::new(required("SOLANA_RPC_WS")?),
            solana_rpc_http: SecretUrl::new(required("SOLANA_RPC_HTTP")?),
            worker_max_retries: parse_required_u32("RPC_WORKER_MAX_RETRIES")?,
            scope,
        })
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
