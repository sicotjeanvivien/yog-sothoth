//! Daemon configuration, read once from the environment by `Config::load`.
//!
//! Every external address is a `<FUNCTION>_URL` / `<FUNCTION>_KEY` pair named
//! after what it serves, never after its protocol (see [`Endpoint`]). Jupiter
//! is the exception: it authenticates by header, so its key never enters its
//! URL.

use std::num::NonZeroU32;
use std::time::Duration;

use yog_bootstrap::{
    ConfigError, Endpoint, SecretKey, SecretUrl, duration_var, parse_optional, required,
    required_endpoint, required_secret_key, required_secret_url,
};
use yog_core::domain::PRICE_MAX_AGE_LATEST;

/// Default for `CONTEXT_PRICE_INTERVAL_SECS`.
const DEFAULT_PRICE_INTERVAL_SECS: u64 = 30;

/// Default for `JUPITER_RATE_LIMIT_PER_MINUTE`: Jupiter's free tier.
const DEFAULT_JUPITER_RATE_LIMIT_PER_MINUTE: NonZeroU32 = NonZeroU32::new(60).unwrap();

/// Default for `CONTEXT_METADATA_POLL_SECS`.
const DEFAULT_METADATA_POLL_SECS: u64 = 10;

/// Runtime configuration for the `yog-context` daemon.
#[derive(Debug, Clone)]
pub(crate) struct Config {
    /// Postgres connection string.
    pub(crate) database_url: SecretUrl,

    /// Where token metadata is read from: the DAS API, Helius' own, not
    /// Solana RPC.
    ///
    /// ⚠️ Its own variable, apart from `pool_account`: the two can live at two
    /// providers.
    pub(crate) token_metadata: Endpoint,

    /// Where pool accounts are read from — `getMultipleAccounts`, standard
    /// Solana JSON-RPC, which any provider serves.
    pub(crate) pool_account: Endpoint,

    /// Jupiter API base URL (e.g. `https://api.jup.ag`); the client appends
    /// `/price/v3`. A plain `String`: Jupiter authenticates by header, so the
    /// URL hides nothing.
    pub(crate) jupiter_url: String,

    /// Jupiter API key, sent as `x-api-key`. A [`SecretKey`], masked whole:
    /// a bare key has no carrier worth showing.
    pub(crate) jupiter_api_key: SecretKey,

    /// The requests per minute the Jupiter key's tier allows, as Jupiter
    /// documents it. The client stays under it (`request_spacing`).
    pub(crate) jupiter_rate_limit: NonZeroU32,

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
            jupiter_rate_limit: parse_optional(
                "JUPITER_RATE_LIMIT_PER_MINUTE",
                DEFAULT_JUPITER_RATE_LIMIT_PER_MINUTE,
                "the requests per minute the Jupiter tier allows, at least 1",
            )?,
            price_interval: price_interval()?,
            metadata_poll_interval: Duration::from_secs(duration_var(
                "CONTEXT_METADATA_POLL_SECS",
                DEFAULT_METADATA_POLL_SECS,
            )?),
        })
    }
}

/// Read `CONTEXT_PRICE_INTERVAL_SECS`, refusing at startup the two cadences
/// the daemon cannot honour: zero, which panics `tokio::time::interval` in the
/// spawned worker after startup succeeded, and anything at or past
/// [`PRICE_MAX_AGE_LATEST`], which leaves every price stale before the next
/// tick fires and every `pool_current_tvl` NULL.
///
/// ⚠️ It bounds the cadence, not the age of the newest price: a tick's own
/// length — about `request_spacing` per chunk — adds to it.
fn price_interval() -> Result<Duration, ConfigError> {
    let seconds = duration_var("CONTEXT_PRICE_INTERVAL_SECS", DEFAULT_PRICE_INTERVAL_SECS)?;
    price_interval_that_keeps_prices_current(seconds)
}

/// The rule of [`price_interval`], apart from the variable it reads, so that
/// it is tested without touching the process-global environment.
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
