//! The writer's configuration, loaded from the environment.

use std::time::Duration;

use yog_bootstrap::{
    ConfigError, SecretUrl, duration_var, optional_secret_url, required_secret_url,
};

const DEFAULT_CYCLE_SECS: u64 = 300;

/// An hour: a cycle advances `MAX_HOURS_PER_CYCLE` hours at most, so a much
/// longer one would fall behind for good.
const MAX_CYCLE_SECS: u64 = 3_600;

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

/// ⚠️ At zero `tokio::time::interval` panics: refused here, not after startup.
fn cycle(seconds: u64) -> Result<Duration, ConfigError> {
    if !(1..=MAX_CYCLE_SECS).contains(&seconds) {
        return Err(ConfigError::InvalidValue {
            key: "ANALYTIC_CYCLE_SECS".to_string(),
            value: seconds.to_string(),
            expected: "a number of seconds from 1 to 3600 (one hour)",
        });
    }
    Ok(Duration::from_secs(seconds))
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
