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
//! The two axes are orthogonal on purpose, and **all four couples now run**.
//! Three of them were refused until 10 September 2026 by a `validator` module
//! that no longer exists: its two arms shared one precondition — a subscription
//! set nothing populated — and both were lifted together when the daemon
//! started registering what it watches. What replaced the refusal is not a
//! looser check but a filled precondition.
//!
//! **Why `Config` carries the acquisition model.** It travels exactly one
//! storey, into the daemon: [`Acquisition`] goes into `init_source`, which
//! builds one of the two implementations and hands back the port; the scope
//! decides what is registered with it — the protocols or the pools — and no
//! listener reads either. Nothing downstream learns which model is running.
//!
//! ⚠️ **And it carries `INGEST_TRANSACTION` *inside* that model**, rather than
//! beside it. `getTransaction` exists on the notify-then-ask path alone, so its
//! endpoint is a field of [`Acquisition::Rpc`] and does not exist on the other
//! arm — which is what makes the variable stop being required under
//! `INGEST_SOURCE=grpc`. Not a rule written somewhere and remembered: no code
//! reads it there. It was required on both paths until the health probe stopped
//! reading it, and the probe was the only reason it ever was.
//!
//! ⚠️ It briefly decided something else, and that was wrong: for a day it chose
//! **which door read `INGEST_STREAM`**, on the belief that only the gRPC path
//! could send a metadata header. Raised in review of PR #138 on 10 September
//! 2026: whether there is a header is said by `INGEST_STREAM_HEADER_NAME` /
//! `_HEADER_VALUE` and by nothing else, and a transport has no business
//! deciding a credential question — the same inversion that was removed from
//! variable *names*. Both listeners now send what the operator declares
//! ([`Credential`]), so there is one door for one variable.
//!
//! # `NETWORK_STATUS_*`, which is deliberately not in that family
//!
//! The health probe's endpoint is the one this process reads that has **no
//! relation to ingestion**, and its name says so. It answers "is the chain
//! advancing, and how far away is it" — an external reference — while the other
//! half of the same dashboard panel, `freshness`, answers "is our ingestion
//! keeping up" from the last event written to the database, with no network
//! call at all. Pointing the probe at the ingestion's own endpoint, which is
//! what `INGEST_TRANSACTION` did until 21 September 2026, collapses two
//! independent questions onto one link: the day the link drops both halves go
//! red together and neither says which failed.
//!
//! It is therefore required on **both** sources — the probe runs whichever
//! model does — and it is free to point at a different provider entirely, which
//! is the only form independence can take.
//!
//! [`Credential`]: crate::infra::Credential

use yog_bootstrap::{
    ConfigError, Endpoint, SecretUrl, parse_required_enum, parse_required_u32, required_endpoint,
    required_endpoint_allowing_header, required_secret_url,
};

mod types;

pub(crate) use types::{Acquisition, IngestScope, IngestSource};

pub(crate) struct Config {
    pub(crate) database_url: SecretUrl,
    /// Where the notifications the ingestion listens to come from.
    pub(crate) ingest_stream: Endpoint,
    /// Which acquisition model to build, and what that model needs — the
    /// `getTransaction` endpoint on the arm that has one. Read by
    /// `init_source` and by `log_ingestion_mode`, and by nothing else.
    pub(crate) acquisition: Acquisition,
    /// The external chain reference the health probe reads. **Not** an
    /// ingestion endpoint: see this module's second section.
    pub(crate) network_status: Endpoint,
    pub(crate) worker_max_retries: u32,
    pub(crate) scope: IngestScope,
}

impl Config {
    pub(crate) fn load() -> Result<Self, ConfigError> {
        let source = parse_required_enum::<IngestSource>("INGEST_SOURCE")?;
        let scope = parse_required_enum::<IngestScope>("INGEST_SCOPE")?;

        Ok(Self {
            database_url: required_secret_url("DATABASE_URL_INDEXER")?,
            // ⚠️ The wide door is a **promise** that whoever holds this
            // `Endpoint` sends the header it carries, and `yog-bootstrap`
            // cannot check it. Both listeners keep it — `infra::endpoint::credential` is
            // the one place that turns the pair into something a client sends,
            // and both go through it.
            ingest_stream: required_endpoint_allowing_header("INGEST_STREAM")?,
            acquisition: match source {
                IngestSource::Rpc => Acquisition::Rpc {
                    transaction: required_endpoint("INGEST_TRANSACTION")?,
                },
                // Nothing to read: the stream delivers the transaction whole.
                IngestSource::Grpc => Acquisition::Grpc,
            },
            network_status: required_endpoint("NETWORK_STATUS")?,
            worker_max_retries: parse_required_u32("RPC_WORKER_MAX_RETRIES")?,
            scope,
        })
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
