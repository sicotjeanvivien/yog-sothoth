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
use yog_core::domain::PRICE_MAX_AGE_LATEST;

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

/// Read `CONTEXT_PRICE_INTERVAL_SECS`, and refuse the two values the daemon
/// cannot honour.
///
/// Neither is new with the redundancy filter — `KeptPrices` decides its floor
/// one tick early precisely so that the cadence, and nothing else, bounds
/// freshness. Both were simply never checked:
///
///   * **zero** panics `tokio::time::interval` inside the spawned worker,
///     *after* startup succeeded — a typo that takes the daemon down where no
///     configuration error is expected any more;
///   * **at or past [`PRICE_MAX_AGE_LATEST`]** means the newest observation is
///     older than the staleness bound before the next tick even fires, so every
///     `pool_current_tvl` reads NULL. Not the filter's doing either: a worker
///     writing on every tick at that cadence was already past the bound.
///
/// Refused at startup rather than logged, because the symptom arrives hours
/// later, on the subset of pools nobody is watching, with nothing in the logs
/// pointing back at a cadence someone raised to be kind to Jupiter.
fn price_interval() -> Result<Duration, ConfigError> {
    let seconds = duration_var("CONTEXT_PRICE_INTERVAL_SECS", DEFAULT_PRICE_INTERVAL_SECS)?;
    price_interval_that_keeps_prices_current(seconds)
}

/// The rule of [`price_interval`], separated from the variable it reads.
///
/// Environment variables are process-global and this binary's tests run in
/// parallel, so the module keeps to a single test that touches them (see
/// `config_tests.rs`). Taking the seconds as an argument is what lets the rule
/// itself be tested without joining that queue.
fn price_interval_that_keeps_prices_current(seconds: u64) -> Result<Duration, ConfigError> {
    const KEY: &str = "CONTEXT_PRICE_INTERVAL_SECS";
    let bound = PRICE_MAX_AGE_LATEST.num_seconds();

    if seconds == 0 {
        return Err(ConfigError::InvalidValue {
            key: KEY.to_string(),
            value: seconds.to_string(),
            expected: "a cadence of at least one second — zero panics the ticker",
        });
    }

    if i64::try_from(seconds).is_ok_and(|s| s < bound) {
        return Ok(Duration::from_secs(seconds));
    }

    Err(ConfigError::InvalidValue {
        key: KEY.to_string(),
        value: seconds.to_string(),
        expected: "a cadence under the 900s price staleness bound of \
                   yog_price_max_age_latest(), or every USD figure reads as absent",
    })
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
