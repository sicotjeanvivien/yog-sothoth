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
//! runtime: the listener dispatches on it. The source does not travel that
//! far — but it no longer stops at validation either, and that changed on
//! 9 September 2026: it now also decides **which door reads
//! `INGEST_STREAM`**, since only one of the two consumers sends a metadata
//! header (see [`read_ingest_stream`]). What it still does not do is reach the
//! runtime, because `init_listener` has one arm; it becomes a field on
//! `Config` the day it has two, which is the gRPC ticket's last slice.

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
            ingest_stream: read_ingest_stream(source)?,
            ingest_transaction: required_endpoint("INGEST_TRANSACTION")?,
            worker_max_retries: parse_required_u32("RPC_WORKER_MAX_RETRIES")?,
            scope,
        })
    }
}

/// Read `INGEST_STREAM` through the door that matches what will consume it.
///
/// # ⚠️ Why the door depends on the source
///
/// `required_endpoint_with_header` is a **promise** that the code holding the
/// `Endpoint` calls [`Endpoint::header`] and sends what it returns;
/// `yog-bootstrap` cannot check that, which is why the two doors are separate
/// names rather than a flag. `INGEST_STREAM` is read by *both* sources, and
/// only one of them keeps that promise today: `infra::grpc::interceptor` puts
/// the header on every request, and the WebSocket path passes the URL alone to
/// `PubsubClient` (`application::workers::subscription`).
///
/// Walking through the `_with_header` door unconditionally would therefore
/// re-create, one level up, exactly the defect that door was added to prevent:
/// an operator on `INGEST_SOURCE=rpc` writing `INGEST_STREAM_HEADER_NAME` /
/// `_HEADER_VALUE`, having it accepted, and connecting anonymously — which
/// succeeds against any endpoint that allows it. Under this `match` that
/// configuration is refused, and it deserves to be: nothing would send it.
///
/// # ⚠️ Two things this `match` does **not** say, both asked in review on 10 September 2026
///
/// **It does not say the gRPC path requires a header.** It does not: the pair
/// is optional on both sides of the door, and three of the four authentication
/// shapes measured across providers use no header at all — a self-hosted
/// Yellowstone takes no credential, Triton's load balancers put the token in
/// the URL as basic auth, and an IP allowlist takes nothing. What the door
/// grants is the *ability* to carry one, not an obligation;
/// `CredentialInterceptor::new(None)` is the no-op that path takes.
///
/// **And it does not say the WebSocket client is incapable of one** — an
/// earlier version of this comment claimed exactly that, and it is false.
/// `PubsubClient::new` takes `R: IntoClientRequest`, so a caller handing it a
/// built `http::Request` instead of a `&str` could set any header it likes.
/// The asymmetry is a property of **our** code, not of the client: the worker
/// passes `ws_url.expose()`, a `&str`, which carries none. That matters,
/// because it means this arm is not a law — the day a WebSocket provider
/// authenticates by header, the fix is to build the request in the worker and
/// move this arm, not to work around it in the configuration.
fn read_ingest_stream(source: IngestSource) -> Result<Endpoint, ConfigError> {
    match source {
        IngestSource::Grpc => required_endpoint_with_header("INGEST_STREAM"),
        IngestSource::Rpc => required_endpoint("INGEST_STREAM"),
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
