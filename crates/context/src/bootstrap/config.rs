//! Daemon configuration, loaded from the environment.
//!
//! Mirrors the config pattern of the other crates: a plain struct
//! built once at startup by `Config::load`, after `dotenvy` has
//! populated the environment.

use std::time::Duration;

use yog_bootstrap::{ConfigError, SecretUrl, duration_var, required};

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
    pub(crate) database_url: SecretUrl,
    pub(crate) helius_url: SecretUrl,
    pub(crate) jupiter_url: SecretUrl,
    pub(crate) price_interval: Duration,
    pub(crate) metadata_poll_interval: Duration,
}

impl Config {
    pub(crate) fn load() -> Result<Self, ConfigError> {
        Ok(Self {
            database_url: SecretUrl::new(required("DATABASE_URL_CONTEXT")?),
            helius_url: SecretUrl::new(required("HELIUS_URL")?),
            jupiter_url: SecretUrl::new(required("JUPITER_URL")?),
            price_interval: Duration::from_secs(duration_var(
                "CONTEXT_PRICE_INTERVAL_SECS",
                DEFAULT_PRICE_INTERVAL_SECS,
            )?),
            metadata_poll_interval: Duration::from_secs(duration_var(
                "CONTEXT_METADATA_POLL_SECS",
                DEFAULT_METADATA_POLL_SECS,
            )?),
        })
    }
}
