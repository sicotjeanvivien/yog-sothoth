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
//! anywhere — it is read, validated, and dropped. It becomes a field the day
//! `init_listener` has two arms, which is the gRPC ticket's last slice.
//!
//! ⚠️ It briefly decided something else, and that was wrong: for a day it chose
//! **which door read `INGEST_STREAM`**, on the belief that only the gRPC path
//! could send a metadata header. Raised in review of PR #138 on 10 September
//! 2026: whether there is a header is said by
//! `INGEST_STREAM_HEADER_NAME` / `_HEADER_VALUE` and by nothing else, and a
//! transport has no business deciding a credential question — the same
//! inversion `04 - release/une-variable-nomme-un-transport.md` removed from
//! variable *names*. Both listeners now send what the operator declares
//! (`infra::credential`), so there is one door for one variable.

use yog_bootstrap::{
    ConfigError, Endpoint, SecretUrl, parse_required_enum, parse_required_u32, required_endpoint,
    required_endpoint_with_header, required_secret_url,
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
            // ⚠️ `_with_header` is a **promise** that whoever holds this
            // `Endpoint` sends the header it carries, and `yog-bootstrap`
            // cannot check it. Both listeners keep it — `infra::credential` is
            // the one place that turns the pair into something a client sends,
            // and both go through it.
            ingest_stream: required_endpoint_with_header("INGEST_STREAM")?,
            ingest_transaction: required_endpoint("INGEST_TRANSACTION")?,
            worker_max_retries: parse_required_u32("RPC_WORKER_MAX_RETRIES")?,
            scope,
        })
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
