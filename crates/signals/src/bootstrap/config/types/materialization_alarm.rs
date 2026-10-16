use std::time::Duration;

use chrono::Duration as ChronoDuration;
use yog_bootstrap::{ConfigError, SecretUrl, duration_var, optional_secret_url};

/// How often the alarm checks, in seconds (`SIGNALS_MATERIALIZATION_INTERVAL_SECS`).
const DEFAULT_INTERVAL_SECS: u64 = 600;

/// How long a raw row may wait before its aggregate is late, in minutes
/// (`SIGNALS_MATERIALIZATION_MAX_WAIT_MINS`); a healthy aggregate peaks at three hours.
const DEFAULT_MAX_WAIT_MINS: u64 = 240;

/// The materialisation alarm's settings, read and checked together.
#[derive(Debug, Clone)]
pub(crate) struct MaterializationAlarmConfig {
    /// Time between two checks, and between two pings of the check below.
    pub(crate) interval: Duration,
    /// How long a raw row may wait before its aggregate is late.
    pub(crate) max_wait: ChronoDuration,
    /// The Healthchecks.io check the alarm reports to; optional, production's compose requires it.
    pub(crate) heartbeat_url: Option<SecretUrl>,
}

impl MaterializationAlarmConfig {
    /// Read every `SIGNALS_MATERIALIZATION_*` variable, refusing at startup the values the alarm
    /// cannot run on.
    pub(crate) fn load() -> Result<Self, ConfigError> {
        Ok(Self {
            interval: interval_secs(duration_var(
                "SIGNALS_MATERIALIZATION_INTERVAL_SECS",
                DEFAULT_INTERVAL_SECS,
            )?)?,
            max_wait: max_wait_minutes(duration_var(
                "SIGNALS_MATERIALIZATION_MAX_WAIT_MINS",
                DEFAULT_MAX_WAIT_MINS,
            )?)?,
            heartbeat_url: optional_secret_url("SIGNALS_MATERIALIZATION_HEARTBEAT_URL"),
        })
    }
}

/// The longest cadence accepted, one day: far beyond a useful check, far below what tokio can
/// add to an `Instant`.
const MAX_INTERVAL_SECS: u64 = 86_400;

/// The cadence, refused at both ends: zero panics `tokio::time::interval`, and past what an
/// `Instant` holds the `Delay` ticker panics on its next tick.
/// ⚠️ The alarm runs beside the engine, not in its own task: either panic would take the daemon down.
fn interval_secs(seconds: u64) -> Result<Duration, ConfigError> {
    if !(1..=MAX_INTERVAL_SECS).contains(&seconds) {
        return Err(ConfigError::InvalidValue {
            key: "SIGNALS_MATERIALIZATION_INTERVAL_SECS".to_string(),
            value: seconds.to_string(),
            expected: "a number of seconds from 1 to 86400 (one day)",
        });
    }
    Ok(Duration::from_secs(seconds))
}

/// The limit, refused at startup: zero fails every check (the filling bucket always has a pending
/// row), and past chrono's range `Duration::minutes` would panic; `as i64` would wrap a huge value.
fn max_wait_minutes(minutes: u64) -> Result<ChronoDuration, ConfigError> {
    i64::try_from(minutes)
        .ok()
        .filter(|m| *m > 0)
        .and_then(ChronoDuration::try_minutes)
        .ok_or_else(|| ConfigError::InvalidValue {
            key: "SIGNALS_MATERIALIZATION_MAX_WAIT_MINS".to_string(),
            value: minutes.to_string(),
            expected: "a number of minutes greater than zero that fits a duration",
        })
}

#[cfg(test)]
#[path = "materialization_alarm_tests.rs"]
mod tests;
