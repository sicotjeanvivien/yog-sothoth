use std::time::Duration;

use chrono::Duration as ChronoDuration;
use yog_bootstrap::{ConfigError, SecretUrl, duration_var, optional_secret_url};

/// How often the alarm checks, in seconds. Overridable via
/// `SIGNALS_MATERIALIZATION_INTERVAL_SECS`.
const DEFAULT_INTERVAL_SECS: u64 = 600;

/// How long a raw row may wait to be materialised before its aggregate is
/// late, in minutes. A healthy aggregate peaks at three hours — `end_offset`
/// one hour, up to one hour until the next hourly refresh, one hour of bucket
/// (migration 008) — so four leaves an hour of margin. Overridable via
/// `SIGNALS_MATERIALIZATION_MAX_WAIT_MINS`.
const DEFAULT_MAX_WAIT_MINS: u64 = 240;

/// The materialisation alarm's settings, read and checked together.
#[derive(Debug, Clone)]
pub(crate) struct MaterializationAlarmConfig {
    /// Time between two checks — and between two pings of the check below.
    pub(crate) interval: Duration,
    /// How long a raw row may wait before its aggregate is late.
    pub(crate) max_wait: ChronoDuration,
    /// The Healthchecks.io check the alarm reports to. Optional: development
    /// runs the scheduler off and has no check. Production requires it —
    /// `docker-compose.prod.yml` refuses to start without it.
    pub(crate) heartbeat_url: Option<SecretUrl>,
}

impl MaterializationAlarmConfig {
    /// Read every `SIGNALS_MATERIALIZATION_*` variable, and refuse the values
    /// the alarm cannot run on rather than fail later.
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

/// The cadence, refused at zero: `tokio::time::interval` panics on a zero
/// period, and the alarm runs beside the engine, not in a task of its own —
/// the panic would take the whole daemon down after startup rather than
/// refuse it here.
fn interval_secs(seconds: u64) -> Result<Duration, ConfigError> {
    if seconds == 0 {
        return Err(ConfigError::InvalidValue {
            key: "SIGNALS_MATERIALIZATION_INTERVAL_SECS".to_string(),
            value: "0".to_string(),
            expected: "a number of seconds greater than zero",
        });
    }
    Ok(Duration::from_secs(seconds))
}

/// The limit, refused at startup rather than trusted. Zero would make every
/// check fail — a row is always pending in the bucket that is still filling —
/// and a value past chrono's range would panic in `Duration::minutes` instead
/// of naming the variable. `as i64` would also wrap a huge value into a
/// negative one.
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
