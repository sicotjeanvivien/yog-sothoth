//! Signal engine configuration, built once at startup by `Config::load` after `dotenvy`: the
//! database URL and each detector's tunable parameters.

use std::time::Duration;

use chrono::Duration as ChronoDuration;
use rust_decimal::Decimal;
use yog_bootstrap::{ConfigError, SecretUrl, duration_var, required_secret_url};

use crate::application::detectors::{
    FlowImbalanceSettings, PriceOracleDeviationSettings, TvlDrainSettings,
};

mod types;

pub(crate) use types::MaterializationAlarmConfig;

/// How often the flow-imbalance detector ticks, in seconds (`SIGNALS_FLOW_INTERVAL_SECS`).
const DEFAULT_FLOW_INTERVAL_SECS: u64 = 300;

/// Trailing window over which directional volume is aggregated, in hours (`SIGNALS_FLOW_WINDOW_HOURS`).
const DEFAULT_FLOW_WINDOW_HOURS: u64 = 24;

/// Rolling per-pool suppression window, in hours: a persisting imbalance re-alerts at most once
/// per cooldown (`SIGNALS_FLOW_COOLDOWN_HOURS`).
const DEFAULT_FLOW_COOLDOWN_HOURS: u64 = 6;

/// Volume floor (USD) of the flow window (`SIGNALS_FLOW_MIN_VOLUME_USD`).
const DEFAULT_FLOW_MIN_VOLUME_USD: i64 = 10_000;

/// How often the price-oracle-deviation detector ticks, in seconds (`SIGNALS_PRICE_DEVIATION_INTERVAL_SECS`).
const DEFAULT_PRICE_DEVIATION_INTERVAL_SECS: u64 = 300;

/// Rolling per-pool suppression window, in hours (`SIGNALS_PRICE_DEVIATION_COOLDOWN_HOURS`).
const DEFAULT_PRICE_DEVIATION_COOLDOWN_HOURS: u64 = 6;

/// Oldest acceptable oracle price, in minutes (`SIGNALS_PRICE_DEVIATION_MAX_PRICE_AGE_MINS`).
/// ⚠️ A stale oracle reads as a spurious deviation, so the pool is skipped.
const DEFAULT_PRICE_DEVIATION_MAX_PRICE_AGE_MINS: u64 = 15;

/// Oldest acceptable last swap, in hours (`SIGNALS_PRICE_DEVIATION_MAX_SPOT_AGE_HOURS`).
/// ⚠️ A quieter pool's spot price is history, not a live quote, so it is skipped.
const DEFAULT_PRICE_DEVIATION_MAX_SPOT_AGE_HOURS: u64 = 24;

/// Minimum TVL (USD) at the last known prices for a pool to be compared
/// (`SIGNALS_PRICE_DEVIATION_MIN_TVL_USD`): a shallow pool's spot strays from the market by nature.
const DEFAULT_PRICE_DEVIATION_MIN_TVL_USD: i64 = 10_000;

/// How often the TVL-drain detector ticks, in seconds (`SIGNALS_TVL_DRAIN_INTERVAL_SECS`).
const DEFAULT_TVL_DRAIN_INTERVAL_SECS: u64 = 300;

/// Trailing window over which liquidity flow is aggregated, in hours (`SIGNALS_TVL_DRAIN_WINDOW_HOURS`);
/// short on purpose, a 24 h window would dilute a drain.
const DEFAULT_TVL_DRAIN_WINDOW_HOURS: u64 = 6;

/// Rolling per-pool suppression window, in hours (`SIGNALS_TVL_DRAIN_COOLDOWN_HOURS`).
const DEFAULT_TVL_DRAIN_COOLDOWN_HOURS: u64 = 6;

/// Minimum starting TVL (USD) for a pool to be considered (`SIGNALS_TVL_DRAIN_MIN_TVL_USD`).
const DEFAULT_TVL_DRAIN_MIN_TVL_USD: i64 = 10_000;

/// Runtime configuration for the `yog-signals` binary.
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

    /// `|imbalance|` at or above which the signal escalates to Critical.
    pub(crate) flow_critical: Decimal,

    /// Price-oracle-deviation detector cadence.
    pub(crate) price_deviation_interval: Duration,

    /// Price-oracle-deviation rolling per-pool suppression window.
    pub(crate) price_deviation_cooldown: Duration,

    /// Oldest acceptable oracle price observation.
    pub(crate) price_deviation_max_price_age: ChronoDuration,

    /// Oldest acceptable last swap.
    pub(crate) price_deviation_max_spot_age: ChronoDuration,

    /// Price-oracle-deviation TVL floor, in USD.
    pub(crate) price_deviation_min_tvl_usd: Decimal,

    /// `|deviation|` at or above which a signal is emitted.
    pub(crate) price_deviation_threshold: Decimal,

    /// `|deviation|` at or above which the signal escalates to Critical.
    pub(crate) price_deviation_critical: Decimal,

    /// TVL-drain detector cadence.
    pub(crate) tvl_drain_interval: Duration,

    /// TVL-drain aggregation window.
    pub(crate) tvl_drain_window: ChronoDuration,

    /// TVL-drain rolling per-pool suppression window.
    pub(crate) tvl_drain_cooldown: Duration,

    /// TVL-drain starting-TVL floor, in USD.
    pub(crate) tvl_drain_min_tvl_usd: Decimal,

    /// Drain ratio at or above which a signal is emitted.
    pub(crate) tvl_drain_threshold: Decimal,

    /// Drain ratio at or above which the signal escalates to Critical.
    pub(crate) tvl_drain_critical: Decimal,

    /// The materialisation alarm: its cadence, its limit and its check.
    pub(crate) materialization_alarm: MaterializationAlarmConfig,
}

impl Config {
    pub(crate) fn load() -> Result<Self, ConfigError> {
        let config = Self {
            database_url: required_secret_url("DATABASE_URL_SIGNALS")?,
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
            // 0.9 — near one-sided flow.
            flow_critical: decimal_var("SIGNALS_FLOW_CRITICAL", Decimal::new(9, 1))?,
            price_deviation_interval: Duration::from_secs(duration_var(
                "SIGNALS_PRICE_DEVIATION_INTERVAL_SECS",
                DEFAULT_PRICE_DEVIATION_INTERVAL_SECS,
            )?),
            price_deviation_cooldown: Duration::from_secs(
                duration_var(
                    "SIGNALS_PRICE_DEVIATION_COOLDOWN_HOURS",
                    DEFAULT_PRICE_DEVIATION_COOLDOWN_HOURS,
                )? * 3600,
            ),
            price_deviation_max_price_age: ChronoDuration::minutes(duration_var(
                "SIGNALS_PRICE_DEVIATION_MAX_PRICE_AGE_MINS",
                DEFAULT_PRICE_DEVIATION_MAX_PRICE_AGE_MINS,
            )? as i64),
            price_deviation_max_spot_age: ChronoDuration::hours(duration_var(
                "SIGNALS_PRICE_DEVIATION_MAX_SPOT_AGE_HOURS",
                DEFAULT_PRICE_DEVIATION_MAX_SPOT_AGE_HOURS,
            )? as i64),
            price_deviation_min_tvl_usd: decimal_var(
                "SIGNALS_PRICE_DEVIATION_MIN_TVL_USD",
                Decimal::from(DEFAULT_PRICE_DEVIATION_MIN_TVL_USD),
            )?,
            // 0.05 — a 5% spot/oracle gap, past the fee band and oracle lag.
            price_deviation_threshold: decimal_var(
                "SIGNALS_PRICE_DEVIATION_THRESHOLD",
                Decimal::new(5, 2),
            )?,
            // 0.2 — the pool price is way off the market.
            price_deviation_critical: decimal_var(
                "SIGNALS_PRICE_DEVIATION_CRITICAL",
                Decimal::new(2, 1),
            )?,
            tvl_drain_interval: Duration::from_secs(duration_var(
                "SIGNALS_TVL_DRAIN_INTERVAL_SECS",
                DEFAULT_TVL_DRAIN_INTERVAL_SECS,
            )?),
            tvl_drain_window: ChronoDuration::hours(duration_var(
                "SIGNALS_TVL_DRAIN_WINDOW_HOURS",
                DEFAULT_TVL_DRAIN_WINDOW_HOURS,
            )? as i64),
            tvl_drain_cooldown: Duration::from_secs(
                duration_var(
                    "SIGNALS_TVL_DRAIN_COOLDOWN_HOURS",
                    DEFAULT_TVL_DRAIN_COOLDOWN_HOURS,
                )? * 3600,
            ),
            tvl_drain_min_tvl_usd: decimal_var(
                "SIGNALS_TVL_DRAIN_MIN_TVL_USD",
                Decimal::from(DEFAULT_TVL_DRAIN_MIN_TVL_USD),
            )?,
            // 0.5 — half the pool's liquidity left within the window.
            tvl_drain_threshold: decimal_var("SIGNALS_TVL_DRAIN_THRESHOLD", Decimal::new(5, 1))?,
            // 0.8 — the pool is nearly emptied.
            tvl_drain_critical: decimal_var("SIGNALS_TVL_DRAIN_CRITICAL", Decimal::new(8, 1))?,
            materialization_alarm: MaterializationAlarmConfig::load()?,
        };

        // ⚠️ Warning strictly below Critical: the other way round every signal would be Critical,
        // a silent misconfiguration, so refused at startup.
        validate_ladder(
            "SIGNALS_FLOW_THRESHOLD",
            config.flow_threshold,
            config.flow_critical,
        )?;
        validate_ladder(
            "SIGNALS_PRICE_DEVIATION_THRESHOLD",
            config.price_deviation_threshold,
            config.price_deviation_critical,
        )?;
        validate_ladder(
            "SIGNALS_TVL_DRAIN_THRESHOLD",
            config.tvl_drain_threshold,
            config.tvl_drain_critical,
        )?;

        Ok(config)
    }

    /// The flow-imbalance detector's settings.
    pub(crate) fn flow_imbalance_settings(&self) -> FlowImbalanceSettings {
        FlowImbalanceSettings {
            window: self.flow_window,
            interval: self.flow_interval,
            cooldown: self.flow_cooldown,
            min_volume_usd: self.flow_min_volume_usd,
            threshold: self.flow_threshold,
            critical: self.flow_critical,
        }
    }

    /// The price-oracle-deviation detector's settings.
    pub(crate) fn price_oracle_deviation_settings(&self) -> PriceOracleDeviationSettings {
        PriceOracleDeviationSettings {
            interval: self.price_deviation_interval,
            cooldown: self.price_deviation_cooldown,
            max_price_age: self.price_deviation_max_price_age,
            max_spot_age: self.price_deviation_max_spot_age,
            min_tvl_usd: self.price_deviation_min_tvl_usd,
            threshold: self.price_deviation_threshold,
            critical: self.price_deviation_critical,
        }
    }

    /// The TVL-drain detector's settings.
    pub(crate) fn tvl_drain_settings(&self) -> TvlDrainSettings {
        TvlDrainSettings {
            window: self.tvl_drain_window,
            interval: self.tvl_drain_interval,
            cooldown: self.tvl_drain_cooldown,
            min_tvl_usd: self.tvl_drain_min_tvl_usd,
            threshold: self.tvl_drain_threshold,
            critical: self.tvl_drain_critical,
        }
    }
}

/// Reject a Warning threshold that reaches its detector's Critical cutoff.
fn validate_ladder(
    threshold_key: &'static str,
    threshold: Decimal,
    critical: Decimal,
) -> Result<(), ConfigError> {
    if threshold >= critical {
        return Err(ConfigError::InvalidValue {
            key: threshold_key.to_string(),
            value: threshold.to_string(),
            expected: "a Warning threshold strictly below the detector's Critical cutoff",
        });
    }
    Ok(())
}

/// Read an optional `Decimal` env var, `default` when unset; a present but unparseable value is
/// an error. ⚠️ Trimmed, like [`yog_bootstrap::required`]: a `\r` from the CRLF `.env` would be refused.
fn decimal_var(key: &'static str, default: Decimal) -> Result<Decimal, ConfigError> {
    match std::env::var(key).map(|v| v.trim().to_string()) {
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

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
