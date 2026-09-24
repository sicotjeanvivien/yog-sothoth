//! Archiver configuration, loaded from the environment.
//!
//! Every variable is named after what it serves (`ARCHIVE_STORE_*`,
//! `ARCHIVE_HEARTBEAT_URL`), never after the provider behind it: the store is
//! Scaleway Object Storage in production and MinIO in a local test, and
//! nothing here needs to know which.

use std::{net::SocketAddr, path::PathBuf, time::Duration};

use yog_bootstrap::{ConfigError, SecretUrl, duration_var, optional, required_secret_url};

mod types;

pub(crate) use types::StoreConfig;

/// Six hours between dumps: the largest hole the history can take when the
/// database is lost, since the indexer cannot re-ingest the past.
/// Overridable via `ARCHIVE_INTERVAL_SECS`.
const DEFAULT_INTERVAL_SECS: u64 = 6 * 60 * 60;

/// The shortest interval accepted. Dumps are named to the second, so two
/// runs within the same second would share a key and the second would
/// overwrite the first; and a zero interval dumps back to back without end.
const MIN_INTERVAL_SECS: u64 = 60;

/// Where `/metrics` listens unless `ARCHIVE_METRICS_ADDR` says otherwise —
/// the port every daemon uses inside its container.
const DEFAULT_METRICS_ADDR: &str = "0.0.0.0:9000";

/// Runtime configuration for the `yog-archive` binary.
#[derive(Debug)]
pub(crate) struct Config {
    /// Postgres connection string for the `yog_archive` role, which reads
    /// everything and writes nothing.
    pub(crate) database_url: SecretUrl,

    /// Time between two dumps. The first one runs at startup.
    pub(crate) interval: Duration,

    /// The bucket the dumps go to.
    pub(crate) store: StoreConfig,

    /// The dead man's switch. Required: an archiver that fails in silence
    /// looks exactly like one that works. A `SecretUrl` because the check's
    /// UUID in the path is what lets anyone ping it.
    pub(crate) heartbeat_url: SecretUrl,

    /// Where `/metrics` listens.
    pub(crate) metrics_addr: SocketAddr,

    /// The `pg_dump` and `pg_restore` to run. Plain names resolved on `PATH`
    /// by default; configurable so a host with several Postgres clients can
    /// point at the right major, and so the tests can substitute fakes.
    pub(crate) pg_dump: PathBuf,
    pub(crate) pg_restore: PathBuf,
}

impl Config {
    pub(crate) fn load() -> Result<Self, ConfigError> {
        Ok(Self {
            database_url: required_secret_url("DATABASE_URL_ARCHIVE")?,
            interval: interval(duration_var(
                "ARCHIVE_INTERVAL_SECS",
                DEFAULT_INTERVAL_SECS,
            )?)?,
            store: StoreConfig::load()?,
            heartbeat_url: required_secret_url("ARCHIVE_HEARTBEAT_URL")?,
            metrics_addr: metrics_addr()?,
            pg_dump: program("ARCHIVE_PG_DUMP", "pg_dump"),
            pg_restore: program("ARCHIVE_PG_RESTORE", "pg_restore"),
        })
    }
}

/// A client program: the variable when set, the plain name resolved on
/// `PATH` otherwise.
fn program(key: &str, default: &str) -> PathBuf {
    optional(key).unwrap_or_else(|| default.to_string()).into()
}

fn interval(secs: u64) -> Result<Duration, ConfigError> {
    if secs < MIN_INTERVAL_SECS {
        return Err(ConfigError::InvalidValue {
            key: "ARCHIVE_INTERVAL_SECS".to_string(),
            value: secs.to_string(),
            expected: "at least 60 seconds",
        });
    }
    Ok(Duration::from_secs(secs))
}

fn metrics_addr() -> Result<SocketAddr, ConfigError> {
    let raw = optional("ARCHIVE_METRICS_ADDR").unwrap_or_else(|| DEFAULT_METRICS_ADDR.into());
    raw.parse().map_err(|_| ConfigError::InvalidValue {
        key: "ARCHIVE_METRICS_ADDR".to_string(),
        value: raw,
        expected: "a socket address such as 0.0.0.0:9000",
    })
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
