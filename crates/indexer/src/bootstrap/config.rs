//! Daemon configuration, loaded from the environment.
//!
//! # The `INGEST_*` family
//!
//! Ingestion is described by **two independent axes**, one variable each:
//!
//! - `INGEST_SOURCE` — *where transactions come from*, i.e. the acquisition
//!   model (notify-then-ask over JSON-RPC, or a delivered gRPC stream);
//! - `INGEST_SCOPE` — *what is subscribed to*, a program id per watched
//!   protocol or one entry per row of `watched_pools`.
//!
//! and the two endpoints join the same family — `INGEST_STREAM_URL` for what
//! is listened to, `INGEST_TRANSACTION_URL` for what is fetched back. Each is
//! named after the **function it serves**, never after the protocol it speaks:
//! `SOLANA_RPC_HTTP`, which they replace, excluded nothing, so three roles for
//! two dependencies had accumulated under it across two crates.
//!
//! Both axes are values read once at start-up and immutable afterwards, which
//! is why their types live under `config/types/` and not beside whoever reads
//! them: a consumer reads *a setting*, it does not own the type. `SecretUrl`
//! and `Endpoint` sit in `yog-bootstrap` for the same reason, and are likewise
//! consumed by the infrastructure layer.
//!
//! The two axes are orthogonal on purpose: all four couples mean something,
//! and the three that cannot run today are refused by `validator`, which
//! `load` calls before anything else is read — see that module for which,
//! and why.
//!
//! **Why `Config` carries a scope but no source.** The scope travels into the
//! runtime: the listener dispatches on it. The source does not travel
//! anywhere yet — it picks *which listener to build*, and there is one, so
//! the dispatch that would read it has nowhere to branch. It is still **read
//! and validated** here, because refusing an unimplemented source at config
//! load is the whole point; it becomes a field the day `init_listener` has
//! two arms, which is the gRPC ticket's job, not this module's.

use yog_bootstrap::{
    ConfigError, Endpoint, SecretUrl, parse_required_enum, parse_required_u32, required_endpoint,
    required_secret_url,
};

mod types;
mod validator;

pub(crate) use types::{IngestScope, IngestSource};
use validator::check_supported;

pub(crate) struct Config {
    pub(crate) database_url: SecretUrl,
    /// Where the notifications the ingestion listens to come from.
    pub(crate) ingest_stream: Endpoint,
    /// Where a transaction is fetched back from, once a notification names it.
    pub(crate) ingest_transaction: Endpoint,
    pub(crate) worker_max_retries: u32,
    pub(crate) scope: IngestScope,
}

impl Config {
    pub(crate) fn load() -> Result<Self, ConfigError> {
        let source = parse_required_enum::<IngestSource>("INGEST_SOURCE")?;
        let scope = parse_required_enum::<IngestScope>("INGEST_SCOPE")?;
        check_supported(source, scope)?;

        Ok(Self {
            database_url: required_secret_url("DATABASE_URL_INDEXER")?,
            ingest_stream: required_endpoint("INGEST_STREAM")?,
            ingest_transaction: required_endpoint("INGEST_TRANSACTION")?,
            worker_max_retries: parse_required_u32("RPC_WORKER_MAX_RETRIES")?,
            scope,
        })
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
