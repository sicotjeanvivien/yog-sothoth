//! Signal engine configuration, loaded from the environment.
//!
//! Mirrors the config pattern of the other crates: a plain struct built
//! once at startup by `Config::load`, after `dotenvy` has populated the
//! environment. Holds the DB URL (yog_signals role) and the flow-imbalance
//! detector's tunable parameters.

use std::time::Duration;

use chrono::Duration as ChronoDuration;
use rust_decimal::Decimal;
use yog_bootstrap::{ConfigError, SecretUrl, duration_var, required};

/// How often the flow-imbalance detector ticks, in seconds.
/// Overridable via `SIGNALS_FLOW_INTERVAL_SECS`.
const DEFAULT_FLOW_INTERVAL_SECS: u64 = 300;

/// Trailing window over which directional volume is aggregated, in hours.
/// Overridable via `SIGNALS_FLOW_WINDOW_HOURS`.
const DEFAULT_FLOW_WINDOW_HOURS: u64 = 24;

/// Rolling per-pool suppression window, in hours — a persisting imbalance
/// re-alerts at most once per cooldown. Overridable via
/// `SIGNALS_FLOW_COOLDOWN_HOURS`.
const DEFAULT_FLOW_COOLDOWN_HOURS: u64 = 6;

/// Minimum total window volume (USD) for a pool to be considered.
/// Overridable via `SIGNALS_FLOW_MIN_VOLUME_USD`.
const DEFAULT_FLOW_MIN_VOLUME_USD: i64 = 10_000;

/// Runtime configuration for the `signal-engine` binary.
#[derive(Debug, Clone)]
pub(crate) struct Config {
    /// Postgres connection string for the yog_signals role.
    pub(crate) database_url: SecretUrl,

    /// Flow-imbalance detector cadence.
    pub(crate) flow_interval: Duration,

    /// Flow-imbalance aggregation window.
    pub(crate) flow_window: ChronoDuration,

    /// Flow-imbalance rolling per-pool suppression window.
    pub(crate) flow_cooldown: Duration,

    /// Flow-imbalance volume floor, in USD.
    pub(crate) flow_min_volume_usd: Decimal,

    /// `|imbalance|` at or above which a signal is emitted.
    pub(crate) flow_threshold: Decimal,
}

impl Config {
    pub(crate) fn load() -> Result<Self, ConfigError> {
        Ok(Self {
            database_url: SecretUrl::new(required("DATABASE_URL_SIGNALS")?),
            flow_interval: Duration::from_secs(duration_var(
                "SIGNALS_FLOW_INTERVAL_SECS",
                DEFAULT_FLOW_INTERVAL_SECS,
            )?),
            flow_window: ChronoDuration::hours(duration_var(
                "SIGNALS_FLOW_WINDOW_HOURS",
                DEFAULT_FLOW_WINDOW_HOURS,
            )? as i64),
            flow_cooldown: Duration::from_secs(
                duration_var("SIGNALS_FLOW_COOLDOWN_HOURS", DEFAULT_FLOW_COOLDOWN_HOURS)? * 3600,
            ),
            flow_min_volume_usd: decimal_var(
                "SIGNALS_FLOW_MIN_VOLUME_USD",
                Decimal::from(DEFAULT_FLOW_MIN_VOLUME_USD),
            )?,
            // 0.6 — a clearly lopsided flow, without drowning in noise.
            flow_threshold: decimal_var("SIGNALS_FLOW_THRESHOLD", Decimal::new(6, 1))?,
        })
    }
}

/// Read an optional `Decimal` env var, falling back to `default` when unset.
/// A present-but-unparseable value is an error (mirrors `duration_var`).
fn decimal_var(key: &'static str, default: Decimal) -> Result<Decimal, ConfigError> {
    match std::env::var(key) {
        Err(_) => Ok(default),
        Ok(raw) => raw
            .parse::<Decimal>()
            .map_err(|_| ConfigError::InvalidValue {
                key: key.to_string(),
                value: raw,
                expected: "a decimal number",
            }),
    }
}
