use std::env;

use super::*;

/// The DAS and the account reads come from two variables, given two hosts
/// here. The refusals of each pair are tested in `yog-bootstrap`, where the
/// pair lives.
#[test]
fn the_two_solana_endpoints_are_read_from_two_variables() {
    // SAFETY: process-global keys, and the tests run in parallel; this is the
    // binary's only test that touches the environment.
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

/// `{:?}` on `Config` prints no credential, and still names the hosts.
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
        jupiter_rate_limit: NonZeroU32::new(60).expect("non-zero"),
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
    assert!(rendered.contains("das.example.invalid"), "{rendered}");
    assert!(rendered.contains("accounts.example.invalid"), "{rendered}");
}

/// The two cadences the daemon cannot honour: zero, and anything at or past the
/// staleness bound.
///
/// ⚠️ 480 s must be accepted: the floor of `KeptPrices` absorbs the cadence,
/// and a guard capping it at 300 s would refuse a cadence that works.
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
