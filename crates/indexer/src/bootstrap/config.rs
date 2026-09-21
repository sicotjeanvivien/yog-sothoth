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
//! storey, into the daemon: [`TransactionArrival`] goes into `init_source`, which
//! builds one of the two implementations and hands back the port; the scope
//! decides what is registered with it — the protocols or the pools — and no
//! listener reads either. Nothing downstream learns which model is running.
//!
//! ⚠️ **And it carries `INGEST_TRANSACTION` *inside* that model**, rather than
//! beside it. `getTransaction` exists on the notify-then-ask path alone, so its
//! endpoint is a field of [`TransactionArrival::Fetched`] and does not exist on the other
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

pub(crate) use types::{IngestScope, IngestSource, TransactionArrival};

pub(crate) struct Config {
    pub(crate) database_url: SecretUrl,
    /// Where the notifications the ingestion listens to come from.
    pub(crate) ingest_stream: Endpoint,
    /// How a transaction reaches this process, and what that way of reaching
    /// it needs — the `getTransaction` endpoint on the arm that asks for one.
    /// Read by `init_source` and by `log_ingestion_mode`, and by nothing else.
    pub(crate) transaction_arrival: TransactionArrival,
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
            transaction_arrival: TransactionArrival::from_source(source)?,
            network_status: required_endpoint("NETWORK_STATUS")?,
            worker_max_retries: parse_required_u32("RPC_WORKER_MAX_RETRIES")?,
            scope,
        })
    }

    /// Whether the health probe reads an address ingestion already uses.
    ///
    /// **A fact about the configuration, not a log line**, which is why it is
    /// a method here and not a branch inside a bootstrap function: it can be
    /// read, asserted and mutated in a unit test, where buried in the daemon's
    /// wiring it needed a live `Database` to reach and was therefore never
    /// covered by anything.
    ///
    /// What it can honestly see is **one address written twice**, and that is
    /// the likely mistake, since `.env.example` seeds the probe and the fetch
    /// endpoint with the same public host. Two spellings of one host it cannot
    /// see — `wss://h` and `https://h` compare as different, and recognising
    /// that they are the same host would mean parsing the address, which is
    /// the form-recognition this configuration refuses everywhere else. The
    /// warning built on this says what it compared, rather than implying more.
    ///
    /// Comparison is on [`Endpoint`]'s `Display` — the template the operator
    /// wrote — and not on the assembled URL: the templates compare exactly as
    /// well, and every `.expose()` site of the crate is counted by a guard, so
    /// a new one bought for a comparison would be a widening for nothing.
    pub(crate) fn probe_shares_ingestion_address(&self) -> bool {
        let probe = self.network_status.to_string();
        self.ingest_stream.to_string() == probe
            || self
                .transaction_arrival
                .fetched_from()
                .is_some_and(|fetch| fetch.to_string() == probe)
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
