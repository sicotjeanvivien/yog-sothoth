use std::env;

use super::*;

/// `validator` exercises `check_supported` as a function. This one
/// exercises the thing that actually protects the process: that `load`
/// **calls** it. Without it, deleting the call from `load` leaves every
/// other test in this file green — verified by mutation, 2 September 2026.
///
/// One test rather than two, walking both couples in sequence: these eight
/// keys are process-global and cargo runs this binary's tests in parallel,
/// so splitting them would have the two halves race each other.
///
/// ⚠️ A test **owns both halves of every pair it reads** — sets them, or
/// removes them on purpose as `yog-context`'s sibling does for its public
/// endpoint. Leaving one half to the ambient environment is not a theoretical
/// risk: the documented native workflow is `set -a; . ./.env`, this repo's own
/// `.env` carries both `_KEY`s, and a key inherited from there against a
/// placeholder-free URL is exactly the combination `required_endpoint`
/// refuses. Measured: `INGEST_STREAM_KEY=abc cargo test -p yog-indexer` turned
/// this test red before the halves were set here.
#[test]
fn load_refuses_an_unsupported_couple_and_accepts_the_supported_one() {
    // SAFETY — and the honest version of it: `set_var` is unsound while any
    // other thread touches the environment *at all*, not merely the same
    // keys, and cargo runs this binary's tests multi-threaded. There is one
    // such reader: `env::var_os("UPDATE_GOLDEN")` in the extraction oracle
    // tests. Nothing here makes that race impossible — a mutex among writers
    // would not, the reader not holding it — so what is claimed is only that
    // the window is a handful of instructions at process start-up and has
    // never been observed to fire. The way to actually close it is to stop
    // `load` reading the process environment, which is a bigger change than
    // this test is worth.
    unsafe {
        env::set_var("DATABASE_URL_INDEXER", "postgresql://u:p@localhost:5433/db");
        env::set_var("INGEST_STREAM_URL", "wss://example.invalid/?k={key}");
        env::set_var("INGEST_STREAM_KEY", "stream-key");
        env::set_var("INGEST_TRANSACTION_URL", "https://example.invalid/?k={key}");
        env::set_var("INGEST_TRANSACTION_KEY", "transaction-key");
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

    // The two endpoints are read from two variables, and each keeps its own
    // value: this is what `SOLANA_RPC_HTTP` could not express, since one
    // variable cannot hold two addresses.
    assert_eq!(
        config.ingest_stream.url().expose(),
        "wss://example.invalid/?k=stream-key"
    );
    assert_eq!(
        config.ingest_transaction.url().expose(),
        "https://example.invalid/?k=transaction-key"
    );
}
