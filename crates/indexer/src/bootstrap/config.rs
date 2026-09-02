//! Daemon configuration, loaded from the environment.
//!
//! Ingestion is described by **two independent axes**, one variable each:
//!
//! - `INGEST_SOURCE` — *where transactions come from*, i.e. the acquisition
//!   model (notify-then-ask over JSON-RPC, or a delivered gRPC stream);
//! - `INGEST_SCOPE` — *what is subscribed to*, a program id per watched
//!   protocol or one entry per row of `watched_pools`.
//!
//! Both are values read once at start-up and immutable afterwards, which is
//! why their types live under `config/types/` and not beside whoever reads
//! them: a consumer reads *a setting*, it does not own the type. `SecretUrl`
//! sits in `yog-bootstrap` for the same reason, and is likewise consumed by
//! the infrastructure layer.
//!
//! The two axes are orthogonal on purpose: all four couples mean something,
//! and the **three** that cannot run today — for two distinct causes, hence
//! two `Err` arms — are refused by `check_supported`, which `load` calls
//! before anything else is read, rather than letting the mistake surface
//! further down as an unexplained `NoSubscriptionTargets`. Each refusal is a
//! state of this repository, not a law.
//!
//! **Why `Config` carries a scope but no source.** The scope travels into the
//! runtime: the listener dispatches on it. The source does not travel
//! anywhere yet — it picks *which listener to build*, and there is one, so
//! the dispatch that would read it has nowhere to branch. It is still **read
//! and validated** here, because refusing an unimplemented source at config
//! load is the whole point; it becomes a field the day `init_listener` has
//! two arms, which is the gRPC ticket's job, not this module's.

use yog_bootstrap::{ConfigError, SecretUrl, parse_required_enum, parse_required_u32, required};

mod types;

// `IngestScope` travels on — `bootstrap.rs` re-exports it for the listener.
// `IngestSource` stops here, for the reason given in the module doc.
pub(crate) use types::IngestScope;
use types::IngestSource;

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

/// Refuse the `(source, scope)` couples this repository cannot honour yet.
///
/// Lives here rather than with either type because it validates neither of
/// them: it validates their *couple*, which is a property of the
/// configuration as a whole — the same place `validate_ladder` occupies in
/// `yog-signals`.
///
/// Neither refusal is a law about the two axes — all four couples are
/// meaningful. What they buy is a failure that *names its cause*:
/// `INGEST_SCOPE=protocols` on the RPC path used to reach the listener and
/// die there on `NoSubscriptionTargets`, which reads like a network problem
/// and is a configuration one.
///
/// **The two arms have different preconditions, and do not lift together.**
/// The `grpc` arm goes when a `GrpcListener` exists. The `(rpc, protocols)`
/// arm goes when `RpcListener::_watch` has a caller — expected of the gRPC
/// migration, but a separate fact: lifting it merely because gRPC landed
/// would restore the failure this whole change was written to remove, an
/// indexer that cannot start on a fresh clone.
///
/// The match is exhaustive on the couple deliberately. Adding a variant to
/// either enum — the "gRPC to detect, `getTransaction` to fetch" stepping
/// stone, say — fails to compile here, and this is the site where a
/// *decision* is owed rather than a name. Verified by adding a third
/// `IngestSource` variant on 2 September 2026: two `E0004`s, here and in
/// `as_str`. What the compiler does **not** flag is `from_env_value`, which
/// matches on `&str` and keeps its `_` arm — a new variant stays unparseable
/// until someone adds its name there by hand.
fn check_supported(source: IngestSource, scope: IngestScope) -> Result<(), ConfigError> {
    match (source, scope) {
        (IngestSource::Rpc, IngestScope::Pools) => Ok(()),

        (IngestSource::Rpc, IngestScope::Protocols) => Err(ConfigError::UnsupportedCombination {
            detail: format!(
                "INGEST_SOURCE={} with INGEST_SCOPE={}: nothing feeds the listener's watched \
                 protocols yet (`RpcListener::_watch` has no caller), so it would start with \
                 zero subscription targets. It gets wired with the Yellowstone gRPC listener; \
                 until then use INGEST_SCOPE=pools.",
                source.as_str(),
                scope.as_str(),
            ),
        }),

        (IngestSource::Grpc, _) => Err(ConfigError::UnsupportedCombination {
            detail: format!(
                "INGEST_SOURCE={} (with INGEST_SCOPE={}): the gRPC listener does not exist yet, \
                 the RPC path is the only implemented source. Use INGEST_SOURCE=rpc.",
                source.as_str(),
                scope.as_str(),
            ),
        }),
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
