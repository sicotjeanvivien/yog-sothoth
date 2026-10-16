use std::env;
use std::sync::Mutex;

use super::*;
use yog_bootstrap::EnvEnum;

/// Held by every test in this file that writes the environment; poison is stepped over, the
/// panicking test having already failed.
/// ⚠️ It closes the race between writers only, not with a reader such as the oracle's `UPDATE_GOLDEN`.
static ENV: Mutex<()> = Mutex::new(());

/// The variable a refusal is expected to name: the name is what bites, `MissingVariable` alone
/// matching any. Takes the `Result`, `Config` having no `Debug` on purpose.
fn assert_refused_naming(loaded: Result<Config, ConfigError>, expected: &str, why: &str) {
    match loaded {
        Ok(_) => panic!("{why}: expected a refusal naming `{expected}`, it loaded"),
        Err(ConfigError::MissingVariable(key)) => assert_eq!(key, expected, "{why}"),
        Err(other) => panic!("{why}: expected a missing `{expected}`, got {other:?}"),
    }
}

/// All four `(source, scope)` couples load, and each endpoint variable is required where it is read.
/// ⚠️ A test owns both halves of every pair it reads: `.env` carries both `_KEY`s, and one inherited
/// against a placeholder-free URL is what `required_endpoint` refuses.
#[test]
fn every_couple_of_the_two_axes_loads() {
    let _env = ENV.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    // SAFETY: `set_var` is unsound while another thread reads the environment; the one reader,
    // the oracle's `UPDATE_GOLDEN`, races a handful of start-up instructions, never observed.
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
    assert_eq!(config.transaction_arrival.source(), IngestSource::Rpc);
    assert_eq!(config.scope, IngestScope::Protocols);

    // SAFETY: same keys, same reasoning.
    unsafe {
        env::set_var("INGEST_SOURCE", "grpc");
    }
    let config = Config::load().expect("grpc + protocols loads");
    assert_eq!(config.transaction_arrival.source(), IngestSource::Grpc);

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
    assert_eq!(config.transaction_arrival.source(), IngestSource::Rpc);
    assert_eq!(config.scope, IngestScope::Pools);

    // The three endpoints are read from three variables, each keeping its own value.
    assert_eq!(
        config.ingest_stream.url().expose(),
        "wss://stream.invalid/?k=stream-key"
    );
    assert_eq!(
        config.network_status.url().expose(),
        "https://reference.invalid/?k=reference-key"
    );
    let TransactionArrival::Fetched { from } = &config.transaction_arrival else {
        panic!("`rpc` must carry the endpoint `getTransaction` goes to");
    };
    assert_eq!(
        from.url().expose(),
        "https://fetch.invalid/?k=transaction-key"
    );
    // The probe and the ingestion are free to be two different hosts.
    assert_ne!(config.network_status.url().expose(), from.url().expose());

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
    assert!(matches!(
        config.transaction_arrival,
        TransactionArrival::Delivered
    ));

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

    // ⚠️ Put back what was removed: a process left without `NETWORK_STATUS_URL` hands the next
    // test a refusal it did not ask for.
    //
    // SAFETY: same keys, same reasoning.
    unsafe {
        env::set_var("NETWORK_STATUS_URL", "https://reference.invalid/?k={key}");
        env::set_var("NETWORK_STATUS_KEY", "reference-key");
    }
    Config::load().expect("every variable this test removed is back");
}

/// The probe's independence, asserted on the configuration.
/// ⚠️ Three cases, one per way the predicate can be wrong: a collision with the stream, with the
/// fetch endpoint (on one arrival only), or none.
#[test]
fn the_probe_is_told_apart_from_every_address_ingestion_uses() {
    let _env = ENV.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    // SAFETY: as on the test above, a handful of start-up writes and one known reader.
    unsafe {
        env::set_var("DATABASE_URL_INDEXER", "postgresql://u:p@localhost:5433/db");
        env::set_var("RPC_WORKER_MAX_RETRIES", "10");
        env::set_var("INGEST_SCOPE", "pools");
        env::set_var("INGEST_SOURCE", "rpc");
        env::remove_var("INGEST_STREAM_KEY");
        env::remove_var("INGEST_TRANSACTION_KEY");
        env::remove_var("NETWORK_STATUS_KEY");

        // 1. three distinct addresses — nothing to object to.
        env::set_var("INGEST_STREAM_URL", "wss://stream.invalid");
        env::set_var("INGEST_TRANSACTION_URL", "https://fetch.invalid");
        env::set_var("NETWORK_STATUS_URL", "https://reference.invalid");
    }
    assert!(
        !Config::load()
            .expect("three distinct addresses load")
            .probe_shares_ingestion_address(),
        "three different hosts are three different providers"
    );

    // SAFETY: same keys, same reasoning.
    unsafe {
        // 2. the probe is the fetch endpoint.
        env::set_var("NETWORK_STATUS_URL", "https://fetch.invalid");
    }
    assert!(
        Config::load()
            .expect("loads")
            .probe_shares_ingestion_address(),
        "the probe on the getTransaction endpoint is the original defect"
    );

    // SAFETY: same keys, same reasoning.
    unsafe {
        // 3. the probe is the stream, the only endpoint ingestion has on the gRPC path.
        env::set_var("INGEST_SOURCE", "grpc");
        env::remove_var("INGEST_TRANSACTION_URL");
        env::set_var("INGEST_STREAM_URL", "https://stream.invalid");
        env::set_var("NETWORK_STATUS_URL", "https://stream.invalid");
    }
    assert!(
        Config::load()
            .expect("grpc needs no fetch endpoint")
            .probe_shares_ingestion_address(),
        "with no fetch endpoint the stream is the whole of ingestion"
    );

    // SAFETY: same keys, same reasoning, and the restore the sibling test relies on.
    unsafe {
        env::set_var("INGEST_TRANSACTION_URL", "https://fetch.invalid");
    }
}

/// ⚠️ `as_str` names the running model on the start-up `ingestion mode` line: a drift from the
/// parser would misname it, and a wrong log line is believed.
#[test]
fn env_names_round_trip_through_as_str() {
    for source in [IngestSource::Rpc, IngestSource::Grpc] {
        assert_eq!(IngestSource::from_env_value(source.as_str()), Some(source));
    }
    for scope in [IngestScope::Protocols, IngestScope::Pools] {
        assert_eq!(IngestScope::from_env_value(scope.as_str()), Some(scope));
    }
}

/// The ingestion alarm's check is optional and read from its own variable: a loader reading
/// another name would leave the production check silent.
#[test]
fn the_heartbeat_url_is_optional_and_read_when_set() {
    let _env = ENV.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    // SAFETY: as in `every_couple_of_the_two_axes_loads`; the key this test adds is removed after.
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
        env::set_var("INGEST_SCOPE", "pools");
        env::remove_var("INDEXER_HEARTBEAT_URL");
    }
    let config = Config::load().expect("a check is optional");
    assert!(config.heartbeat_url.is_none());

    // SAFETY: same keys, same reasoning.
    unsafe {
        env::set_var(
            "INDEXER_HEARTBEAT_URL",
            "https://hc-ping.invalid/check-uuid",
        );
    }
    let config = Config::load().expect("loads with a check");
    assert_eq!(
        config.heartbeat_url.as_ref().map(SecretUrl::expose),
        Some("https://hc-ping.invalid/check-uuid")
    );

    // SAFETY: same keys, same reasoning.
    unsafe {
        env::remove_var("INDEXER_HEARTBEAT_URL");
    }
}
