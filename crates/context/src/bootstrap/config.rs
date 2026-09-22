//! Daemon configuration, loaded from the environment.
//!
//! Mirrors the config pattern of the other crates: a plain struct
//! built once at startup by `Config::load`, after `dotenvy` has
//! populated the environment.
//!
//! Every external address is a `<FUNCTION>_URL` / `<FUNCTION>_KEY` pair named
//! after **what it serves**, never after the protocol it speaks — see
//! [`Endpoint`]. Jupiter is the exception that proves the rule below: it
//! authenticates by header, so its key never enters its URL and the two stay
//! `String` + `SecretKey`.

use std::time::Duration;

use yog_bootstrap::{
    ConfigError, Endpoint, SecretKey, SecretUrl, duration_var, required, required_endpoint,
    required_secret_key, required_secret_url,
};
use yog_core::domain::max_price_interval;

/// Default interval between Jupiter price fetches, in seconds.
///
/// Overridable via `CONTEXT_PRICE_INTERVAL_SECS`. 30s is a sensible
/// default — frequent enough for a dashboard, light on Jupiter.
const DEFAULT_PRICE_INTERVAL_SECS: u64 = 30;

/// Default interval between `pools` polls for new mints, in seconds.
///
/// Overridable via `CONTEXT_METADATA_POLL_SECS`.
const DEFAULT_METADATA_POLL_SECS: u64 = 10;

/// Runtime configuration for the `yog-context` daemon.
#[derive(Debug, Clone)]
pub(crate) struct Config {
    /// Postgres connection string.
    pub(crate) database_url: SecretUrl,

    /// Where token metadata is read from — the DAS API, which is Helius'
    /// own and not a Solana RPC method.
    ///
    /// Its own variable, and the point of this pair: the DAS and the account
    /// reads below shared one `SOLANA_RPC_HTTP` because that name — a
    /// transport — excluded neither. One variable cannot hold two addresses,
    /// so the day either moves to another provider, the configuration could
    /// not have said so.
    pub(crate) token_metadata: Endpoint,

    /// Where pool accounts are read from — `getMultipleAccounts`, standard
    /// Solana JSON-RPC, which any provider serves.
    pub(crate) pool_account: Endpoint,

    /// Jupiter API base URL (e.g. `https://api.jup.ag`); the client
    /// appends `/price/v3` itself.
    ///
    /// A plain `String`, and deliberately so: Jupiter authenticates by header,
    /// so this URL carries no secret. Wrapping it would blur what `SecretUrl`
    /// asserts — that the value has something to hide.
    pub(crate) jupiter_url: String,

    /// Jupiter API key — sent on every request via `x-api-key`.
    ///
    /// A [`SecretKey`] and not a `SecretUrl`: a bare key has no carrier worth
    /// showing, and `SecretUrl` would have returned it unredacted for want of
    /// a `?`.
    pub(crate) jupiter_api_key: SecretKey,

    /// How often the price worker fetches from Jupiter.
    pub(crate) price_interval: Duration,

    /// How often the metadata worker polls `pools` for new mints.
    pub(crate) metadata_poll_interval: Duration,
}

impl Config {
    pub(crate) fn load() -> Result<Self, ConfigError> {
        Ok(Self {
            database_url: required_secret_url("DATABASE_URL_CONTEXT")?,
            token_metadata: required_endpoint("TOKEN_METADATA")?,
            pool_account: required_endpoint("POOL_ACCOUNT")?,
            jupiter_url: required("JUPITER_URL")?,
            jupiter_api_key: required_secret_key("JUPITER_API_KEY")?,
            price_interval: price_interval()?,
            metadata_poll_interval: Duration::from_secs(duration_var(
                "CONTEXT_METADATA_POLL_SECS",
                DEFAULT_METADATA_POLL_SECS,
            )?),
        })
    }
}

/// Read `CONTEXT_PRICE_INTERVAL_SECS`, and refuse a cadence the price series
/// cannot absorb.
///
/// The worker no longer writes a row per tick: a motionless price is rewritten
/// only when the last kept one reaches the floor, and the forced row lands at
/// the *next tick* after that. Too slow a cadence therefore pushes the newest
/// observation past `yog_price_max_age_latest()` and stops valuing every
/// motionless token — silently, and only for the tokens that never move.
///
/// Refused at startup rather than logged, because the symptom is USD figures
/// that go absent hours later, on a subset of pools, with nothing in the logs
/// pointing back at a cadence someone raised to be kind to Jupiter.
fn price_interval() -> Result<Duration, ConfigError> {
    let seconds = duration_var("CONTEXT_PRICE_INTERVAL_SECS", DEFAULT_PRICE_INTERVAL_SECS)?;
    price_interval_the_series_can_absorb(seconds)
}

/// The rule of [`price_interval`], separated from the variable it reads.
///
/// Environment variables are process-global and this binary's tests run in
/// parallel, so the module keeps to a single test that touches them (see
/// `config_tests.rs`). Taking the seconds as an argument is what lets the bound
/// itself be tested without joining that queue.
fn price_interval_the_series_can_absorb(seconds: u64) -> Result<Duration, ConfigError> {
    let ceiling = max_price_interval().num_seconds();

    if i64::try_from(seconds).is_ok_and(|s| s <= ceiling) {
        return Ok(Duration::from_secs(seconds));
    }

    Err(ConfigError::UnsupportedCombination {
        detail: format!(
            "CONTEXT_PRICE_INTERVAL_SECS={seconds} is longer than the {ceiling}s a price \
             series can absorb: a token whose price never moves would carry an observation \
             older than the staleness bound of yog_price_max_age_latest(), and its USD \
             figures would read as absent. Use {ceiling} or less"
        ),
    })
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
