//! How ingestion is chosen: the two axes, their environment names, and the
//! rule that says which couples this repository can honour.
//!
//! Ingestion is described by **two independent axes**, one variable each:
//!
//! - `INGEST_SOURCE` — *where transactions come from*, i.e. the acquisition
//!   model (notify-then-ask over JSON-RPC, or a delivered gRPC stream);
//! - `INGEST_SCOPE` — *what is subscribed to*, a program id per watched
//!   protocol or one entry per row of `watched_pools`.
//!
//! They are orthogonal on purpose: all four couples mean something, and the
//! **three** that cannot run today — for two distinct causes, hence two `Err`
//! arms — are refused by [`check_supported`], which `bootstrap::config` calls
//! at load time rather than letting the mistake surface further down as an
//! unexplained `NoSubscriptionTargets`. Each refusal is a state of this
//! repository, not a law.
//!
//! This vocabulary sits at the crate root, beside `error`, rather than under
//! `bootstrap`: it is read by the startup wiring **and** by `infra::rpc`, and
//! a shared vocabulary is what both should depend on — not the infra layer on
//! the wiring layer.

use yog_bootstrap::{ConfigError, EnvEnum};

/// Where the indexer's transactions come from.
///
/// Names the acquisition model rather than the wire protocol, because that
/// is what differs: `Rpc` **notifies then asks** — a `logsSubscribe` socket
/// carrying signatures, then one `getTransaction` per signature, which is
/// what caps throughput and drops `transaction_index`. `Grpc` **delivers** —
/// a single Yellowstone stream carrying whole transactions, no second call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IngestSource {
    Rpc,
    Grpc,
}

impl IngestSource {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Rpc => "rpc",
            Self::Grpc => "grpc",
        }
    }
}

impl EnvEnum for IngestSource {
    const EXPECTED: &'static str = "rpc or grpc";

    fn from_env_value(value: &str) -> Option<Self> {
        match value {
            "rpc" => Some(Self::Rpc),
            "grpc" => Some(Self::Grpc),
            _ => None,
        }
    }
}

/// What the listener subscribes to.
///
/// `Protocols` is one subscription per watched protocol, keyed on its
/// program id — full coverage, and the throughput the free tier cannot
/// sustain. `Pools` is one per row of `watched_pools`: that is where the
/// allowlist is enforced — **at the subscription, not by a filter** —
/// nothing downstream being aware of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IngestScope {
    Protocols,
    Pools,
}

impl IngestScope {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Protocols => "protocols",
            Self::Pools => "pools",
        }
    }
}

impl EnvEnum for IngestScope {
    const EXPECTED: &'static str = "protocols or pools";

    fn from_env_value(value: &str) -> Option<Self> {
        match value {
            "protocols" => Some(Self::Protocols),
            "pools" => Some(Self::Pools),
            _ => None,
        }
    }
}

/// Refuse the `(source, scope)` couples this repository cannot honour yet.
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
pub(crate) fn check_supported(source: IngestSource, scope: IngestScope) -> Result<(), ConfigError> {
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
#[path = "ingest_tests.rs"]
mod tests;
