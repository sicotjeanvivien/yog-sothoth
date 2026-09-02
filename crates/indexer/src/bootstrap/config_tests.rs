use std::env;

use super::*;

/// `crate::ingest` tests `check_supported` as a function. This one
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
