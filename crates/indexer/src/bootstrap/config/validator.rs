//! Cross-field validation of the ingestion configuration.
//!
//! Neither axis is wrong on its own — `INGEST_SOURCE` and `INGEST_SCOPE` each
//! parse to a value this repository understands. What can be wrong is their
//! *couple*, which is why this validation belongs to neither type's file.

use yog_bootstrap::ConfigError;

use super::types::{IngestScope, IngestSource};

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
pub(super) fn check_supported(source: IngestSource, scope: IngestScope) -> Result<(), ConfigError> {
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
#[path = "validator_tests.rs"]
mod tests;
