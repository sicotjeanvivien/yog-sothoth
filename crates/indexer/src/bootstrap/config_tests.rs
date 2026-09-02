use std::env;

use super::*;

/// The message is the whole point of refusing here rather than letting the
/// listener die on `NoSubscriptionTargets`, so it is what gets asserted —
/// a `matches!` on the variant alone would stay green on an error that
/// tells the operator nothing.
fn refusal_detail(source: IngestSource, scope: IngestScope) -> String {
    match check_supported(source, scope) {
        Err(ConfigError::UnsupportedCombination { detail }) => detail,
        Err(other) => panic!("expected UnsupportedCombination, got {other:?}"),
        Ok(()) => panic!(
            "expected {} / {} to be refused",
            source.as_str(),
            scope.as_str()
        ),
    }
}

#[test]
fn rpc_over_watched_pools_is_the_supported_couple() {
    assert!(check_supported(IngestSource::Rpc, IngestScope::Pools).is_ok());
}

#[test]
fn rpc_over_protocols_is_refused_and_says_why() {
    let detail = refusal_detail(IngestSource::Rpc, IngestScope::Protocols);

    // Both variables named with their values, so the log line stands on
    // its own, plus the actual cause — not just "unsupported".
    assert!(detail.contains("INGEST_SOURCE=rpc"), "{detail}");
    assert!(detail.contains("INGEST_SCOPE=protocols"), "{detail}");
    assert!(detail.contains("_watch"), "{detail}");
    assert!(detail.contains("INGEST_SCOPE=pools"), "{detail}");
}

#[test]
fn grpc_is_refused_under_either_scope() {
    for scope in [IngestScope::Pools, IngestScope::Protocols] {
        let detail = refusal_detail(IngestSource::Grpc, scope);
        assert!(detail.contains("INGEST_SOURCE=grpc"), "{detail}");
        assert!(detail.contains("INGEST_SOURCE=rpc"), "{detail}");
    }
}

#[test]
fn env_names_round_trip_through_as_str() {
    // `as_str` feeds the refusal messages above; if it ever drifts from the
    // names the parser accepts, those messages start advising a value that
    // would be rejected.
    for source in [IngestSource::Rpc, IngestSource::Grpc] {
        assert_eq!(IngestSource::from_env_value(source.as_str()), Some(source));
    }
    for scope in [IngestScope::Protocols, IngestScope::Pools] {
        assert_eq!(IngestScope::from_env_value(scope.as_str()), Some(scope));
    }
}

/// The four tests above exercise `check_supported` as a function. This one
/// exercises the thing that actually protects the process: that `load`
/// **calls** it. Without it, deleting the call from `load` leaves every
/// other test in this file green — verified by mutation, 2 September 2026.
///
/// One test rather than two, walking both couples in sequence: these six
/// keys are process-global and cargo runs this binary's tests in parallel,
/// so splitting them would have the two halves race each other.
#[test]
fn load_refuses_an_unsupported_couple_and_accepts_the_supported_one() {
    // SAFETY: no other test in this binary reads or writes these keys.
    unsafe {
        env::set_var("DATABASE_URL_INDEXER", "postgresql://u:p@localhost:5433/db");
        env::set_var("SOLANA_RPC_WS", "wss://example.invalid");
        env::set_var("SOLANA_RPC_HTTP", "https://example.invalid");
        env::set_var("RPC_WORKER_MAX_RETRIES", "10");
        env::set_var("INGEST_SOURCE", "rpc");
        env::set_var("INGEST_SCOPE", "protocols");
    }

    match Config::load() {
        Err(ConfigError::UnsupportedCombination { detail }) => {
            assert!(detail.contains("INGEST_SCOPE=protocols"), "{detail}");
        }
        Err(other) => panic!("expected UnsupportedCombination, got {other:?}"),
        Ok(_) => panic!("`load` accepted a couple `check_supported` refuses"),
    }

    // SAFETY: same keys, same reasoning.
    unsafe {
        env::set_var("INGEST_SCOPE", "pools");
    }

    let config = Config::load().expect("rpc + pools is the supported couple");
    assert_eq!(config.scope, IngestScope::Pools);
}
