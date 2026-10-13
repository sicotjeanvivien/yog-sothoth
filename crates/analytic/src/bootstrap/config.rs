//! The writer's configuration, loaded from the environment.

use std::time::Duration;

use yog_bootstrap::{
    ConfigError, SecretUrl, duration_var, optional_secret_url, required_secret_url,
};

const DEFAULT_CYCLE_SECS: u64 = 300;

/// One day: far beyond a useful cadence, far below what an `Instant` holds.
const MAX_CYCLE_SECS: u64 = 86_400;

#[derive(Debug)]
pub(crate) struct Config {
    /// The `yog_analytic` role.
    pub(crate) database_url: SecretUrl,
    pub(crate) cycle: Duration,
    /// Optional in development; `docker-compose.prod.yml` requires it.
    pub(crate) heartbeat_url: Option<SecretUrl>,
}

impl Config {
    pub(crate) fn load() -> Result<Self, ConfigError> {
        Ok(Self {
            database_url: required_secret_url("DATABASE_URL_ANALYTIC")?,
            cycle: cycle(duration_var("ANALYTIC_CYCLE_SECS", DEFAULT_CYCLE_SECS)?)?,
            heartbeat_url: optional_secret_url("ANALYTIC_HEARTBEAT_URL"),
        })
    }
}

/// ⚠️ At zero `tokio::time::interval` panics, and past an `Instant`'s range
/// the ticker does: both would stop the daemon after startup instead of here.
fn cycle(seconds: u64) -> Result<Duration, ConfigError> {
    if !(1..=MAX_CYCLE_SECS).contains(&seconds) {
        return Err(ConfigError::InvalidValue {
            key: "ANALYTIC_CYCLE_SECS".to_string(),
            value: seconds.to_string(),
            expected: "a number of seconds from 1 to 86400 (one day)",
        });
    }
    Ok(Duration::from_secs(seconds))
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
