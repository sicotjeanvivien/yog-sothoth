use std::env;

use super::*;

/// The wall the variable split pulls down: the DAS and the account reads used
/// to share `SOLANA_RPC_HTTP`, so a migration that moved one and not the other
/// could not be *expressed* — one variable, two addresses. Here they are given
/// two different hosts, and each field carries its own.
///
/// The refusals of the pair itself — a `{key}` with no key, a key with no
/// `{key}` — are covered where the pair lives, in `yog-bootstrap`; restating
/// them here would be a second definition of one rule.
#[test]
fn the_two_solana_endpoints_are_read_from_two_variables() {
    // SAFETY — the same honest version as `yog-indexer`'s config test: these
    // keys are process-global and cargo runs this binary's tests in parallel.
    // One test rather than several, so the halves cannot race each other; the
    // remaining window is that another test in this binary reads the
    // environment, and none does.
    unsafe {
        env::set_var("DATABASE_URL_CONTEXT", "postgresql://u:p@localhost:5433/db");
        env::set_var("TOKEN_METADATA_URL", "https://das.example.invalid/?k={key}");
        env::set_var("TOKEN_METADATA_KEY", "das-key");
        env::set_var("POOL_ACCOUNT_URL", "https://accounts.example.invalid");
        env::remove_var("POOL_ACCOUNT_KEY");
        env::set_var("JUPITER_URL", "https://api.jup.ag");
        env::set_var("JUPITER_API_KEY", "jup-key");
    }

    let config = Config::load().expect("every required variable is set");

    assert_eq!(
        config.token_metadata.url().expose(),
        "https://das.example.invalid/?k=das-key"
    );
    assert_eq!(
        config.pool_account.url().expose(),
        "https://accounts.example.invalid"
    );
}

/// `Config` derives `Debug`, and `{:?}` on it is one keystroke away in any
/// error path. Neither endpoint's credential may survive that.
#[test]
fn debugging_the_config_prints_no_credential() {
    let config = Config {
        database_url: SecretUrl::for_tests("postgresql://u:hunter2@localhost:5433/db"),
        token_metadata: Endpoint::for_tests(
            "https://das.example.invalid/?k={key}",
            Some("das-key"),
        ),
        pool_account: Endpoint::for_tests("https://accounts.example.invalid/v2/pasted", None),
        jupiter_url: "https://api.jup.ag".to_string(),
        jupiter_api_key: SecretKey::for_tests("jup-key"),
        price_interval: Duration::from_secs(30),
        metadata_poll_interval: Duration::from_secs(10),
    };

    let rendered = format!("{config:?}");
    for secret in ["hunter2", "das-key", "jup-key", "pasted"] {
        assert!(
            !rendered.contains(secret),
            "`{secret}` survived: {rendered}"
        );
    }
    // And the diagnostic survives: a config that says nothing costs more than
    // it saves — a redaction that removed the host once left a crash log with
    // nothing to go on.
    assert!(rendered.contains("das.example.invalid"), "{rendered}");
    assert!(rendered.contains("accounts.example.invalid"), "{rendered}");
}

/// The two cadences the daemon cannot honour. Tested here and not through
/// `Config::load` on purpose: the environment is process-global, and the file
/// keeps a single test that touches it.
///
/// Neither refusal belongs to the redundancy filter — `KeptPrices` decides its
/// floor one tick early so that the cadence alone bounds freshness. What the
/// filter changed is that the coupling is now *named*, and these are the two
/// values that were silently accepted before: zero, which panics the ticker
/// inside the spawned worker long after startup reported success, and anything
/// at or past the staleness bound, which leaves every price stale before the
/// next tick fires.
///
/// 480 s is in the list because it was the wrong answer to this question: an
/// earlier guard capped the cadence at 300 s, on the theory that the floor plus
/// one tick had to fit under the bound. It must be **accepted** — the floor
/// absorbs the cadence now, and a rule that still refused it would be the old
/// one wearing a new name.
#[test]
fn only_a_cadence_that_keeps_prices_current_is_accepted() {
    for accepted in [1_u64, 30, 300, 480, 899] {
        assert!(
            price_interval_that_keeps_prices_current(accepted).is_ok(),
            "{accepted}s is under the staleness bound and must be accepted"
        );
    }

    let zero = price_interval_that_keeps_prices_current(0)
        .expect_err("tokio::time::interval panics on a zero period");
    assert!(zero.to_string().contains("zero"), "{zero}");

    let bound = u64::try_from(PRICE_MAX_AGE_LATEST.num_seconds()).expect("positive");
    let stale = price_interval_that_keeps_prices_current(bound)
        .expect_err("a price would be stale before the next tick fires");
    assert!(
        stale.to_string().contains(&bound.to_string()),
        "an operator reads this in a crash log: {stale}"
    );
}
