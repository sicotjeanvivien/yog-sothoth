use std::env;

use super::*;
use yog_bootstrap::EnvEnum;

/// The variable a refusal is expected to name, or the test fails saying what it
/// got instead.
///
/// Written once because it is asserted three times, and because the assertion
/// that matters is **the name**, not the variant: `MissingVariable` alone is
/// satisfied by any missing variable, including the one the operator did set.
/// What an operator reads in a crash log is the name, and that is the only
/// level at which this test bites.
///
/// It takes the whole `Result` rather than an error, because `Config` has no
/// `Debug` — it holds the database URL and every endpoint — and `expect_err`
/// would demand one. Keeping the type unprintable is worth a helper.
fn assert_refused_naming(loaded: Result<Config, ConfigError>, expected: &str, why: &str) {
    match loaded {
        Ok(_) => panic!("{why}: expected a refusal naming `{expected}`, it loaded"),
        Err(ConfigError::MissingVariable(key)) => assert_eq!(key, expected, "{why}"),
        Err(other) => panic!("{why}: expected a missing `{expected}`, got {other:?}"),
    }
}

/// ⚠️ **This test used to assert a refusal, and now asserts its opposite.**
/// Until 10 September 2026 three of the four `(source, scope)` couples were
/// rejected at load time by a `validator` module, for one shared reason:
/// nothing populated a subscription set. The gRPC work filled that
/// precondition — the daemon registers, as the scope says, the protocols whose
/// extraction is written or the pools restored from the database — so the
/// module and its refusals are gone. What replaced them is not a looser check
/// but a met precondition, which is why the assertion flips rather than
/// disappearing.
///
/// One test rather than four, walking the couples in sequence: these keys are
/// process-global and cargo runs this binary's tests in parallel, so splitting
/// them would have the halves race each other. The same reason keeps the
/// endpoint refusals below in here rather than in tests of their own — a test
/// that *removes* a variable is exactly the one that must not run beside a test
/// that expects it.
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
fn every_couple_of_the_two_axes_loads() {
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
        env::set_var("INGEST_STREAM_URL", "wss://stream.invalid/?k={key}");
        env::set_var("INGEST_STREAM_KEY", "stream-key");
        env::set_var("INGEST_TRANSACTION_URL", "https://fetch.invalid/?k={key}");
        env::set_var("INGEST_TRANSACTION_KEY", "transaction-key");
        env::set_var("NETWORK_STATUS_URL", "https://reference.invalid/?k={key}");
        env::set_var("NETWORK_STATUS_KEY", "reference-key");
        env::set_var("RPC_WORKER_MAX_RETRIES", "10");
        env::set_var("INGEST_SOURCE", "rpc");
        env::set_var("INGEST_SCOPE", "protocols");
    }

    let config = Config::load().expect("rpc + protocols no longer needs a refusal");
    assert_eq!(config.acquisition.source(), IngestSource::Rpc);
    assert_eq!(config.scope, IngestScope::Protocols);

    // SAFETY: same keys, same reasoning.
    unsafe {
        env::set_var("INGEST_SOURCE", "grpc");
    }
    let config = Config::load().expect("grpc + protocols loads");
    assert_eq!(config.acquisition.source(), IngestSource::Grpc);

    // SAFETY: same keys, same reasoning.
    unsafe {
        env::set_var("INGEST_SCOPE", "pools");
    }
    let config = Config::load().expect("grpc + pools loads");
    assert_eq!(config.scope, IngestScope::Pools);

    // SAFETY: same keys, same reasoning.
    unsafe {
        env::set_var("INGEST_SOURCE", "rpc");
    }

    let config = Config::load().expect("rpc + pools is the couple that runs today");
    assert_eq!(config.acquisition.source(), IngestSource::Rpc);
    assert_eq!(config.scope, IngestScope::Pools);

    // The three endpoints are read from three variables, and each keeps its own
    // value: this is what `SOLANA_RPC_HTTP` could not express, since one
    // variable cannot hold two addresses — and what the probe could not express
    // either while it borrowed the ingestion's.
    assert_eq!(
        config.ingest_stream.url().expose(),
        "wss://stream.invalid/?k=stream-key"
    );
    assert_eq!(
        config.network_status.url().expose(),
        "https://reference.invalid/?k=reference-key"
    );
    let Acquisition::Rpc { transaction } = &config.acquisition else {
        panic!("`rpc` must carry the endpoint `getTransaction` goes to");
    };
    assert_eq!(
        transaction.url().expose(),
        "https://fetch.invalid/?k=transaction-key"
    );
    // The point of the split, asserted rather than assumed: the probe and the
    // ingestion are free to be two different hosts.
    assert_ne!(
        config.network_status.url().expose(),
        transaction.url().expose()
    );

    // ── `INGEST_TRANSACTION` belongs to the path that fetches ──────────────
    //
    // SAFETY: same keys, same reasoning.
    unsafe {
        env::remove_var("INGEST_TRANSACTION_URL");
        env::remove_var("INGEST_TRANSACTION_KEY");
        env::set_var("INGEST_SOURCE", "grpc");
    }
    let config = Config::load()
        .expect("a delivered stream fetches nothing back, so it configures nothing for it");
    assert!(matches!(config.acquisition, Acquisition::Grpc));

    // SAFETY: same keys, same reasoning.
    unsafe {
        env::set_var("INGEST_SOURCE", "rpc");
    }
    assert_refused_naming(
        Config::load(),
        "INGEST_TRANSACTION_URL",
        "notify-then-ask cannot fetch without an endpoint",
    );

    // ── The probe's own variable, required on both models ──────────────────
    //
    // SAFETY: same keys, same reasoning.
    unsafe {
        env::set_var("INGEST_TRANSACTION_URL", "https://fetch.invalid/?k={key}");
        env::set_var("INGEST_TRANSACTION_KEY", "transaction-key");
        env::remove_var("NETWORK_STATUS_URL");
        env::remove_var("NETWORK_STATUS_KEY");
    }
    assert_refused_naming(
        Config::load(),
        "NETWORK_STATUS_URL",
        "the probe runs on the rpc model too",
    );

    // SAFETY: same keys, same reasoning.
    unsafe {
        env::set_var("INGEST_SOURCE", "grpc");
    }
    assert_refused_naming(
        Config::load(),
        "NETWORK_STATUS_URL",
        "and on the grpc model, where nothing else is an HTTP client",
    );

    // ⚠️ **Put back what was removed.** These keys are process-global, and this
    // test is the only one in the binary that unsets any of them: leaving the
    // process without `NETWORK_STATUS_URL` hands the next test added here a
    // refusal it did not ask for, and a failure whose cause is in another
    // function. The rule this file states for the *ambient* environment — a
    // test owns both halves of every pair it reads — is the same one, read from
    // the other end.
    //
    // SAFETY: same keys, same reasoning.
    unsafe {
        env::set_var("NETWORK_STATUS_URL", "https://reference.invalid/?k={key}");
        env::set_var("NETWORK_STATUS_KEY", "reference-key");
    }
    Config::load().expect("every variable this test removed is back");
}

/// ⚠️ **Rescued from `validator_tests.rs` when that module was deleted**, and
/// it matters more now than it did there. `as_str` fed the validator's refusal
/// messages; its reader today is the `ingestion mode` line the daemon writes at
/// start-up, which is what an operator reads to answer "which model is
/// running". A drift between it and the parser used to advise a value that
/// would be rejected; it would now *misname the running mode*, which is worse:
/// a wrong refusal is noticed, a wrong log line is believed.
#[test]
fn env_names_round_trip_through_as_str() {
    for source in [IngestSource::Rpc, IngestSource::Grpc] {
        assert_eq!(IngestSource::from_env_value(source.as_str()), Some(source));
    }
    for scope in [IngestScope::Protocols, IngestScope::Pools] {
        assert_eq!(IngestScope::from_env_value(scope.as_str()), Some(scope));
    }
}
