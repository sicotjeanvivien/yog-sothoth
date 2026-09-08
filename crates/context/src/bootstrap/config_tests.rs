use std::env;

use super::*;

/// The wall this ticket exists to pull down: the DAS and the account reads used
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
    // it saves — that lesson is `04 - release/cle-api-en-clair-dans-les-logs.md`.
    assert!(rendered.contains("das.example.invalid"), "{rendered}");
    assert!(rendered.contains("accounts.example.invalid"), "{rendered}");
}
