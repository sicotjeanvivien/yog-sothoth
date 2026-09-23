//! Archiver configuration, loaded from the environment.
//!
//! Every variable is named after what it serves (`ARCHIVE_STORE_*`,
//! `ARCHIVE_HEARTBEAT_URL`), never after the provider behind it: the store is
//! Scaleway Object Storage in production and MinIO in a local test, and
//! nothing here needs to know which.

use std::{net::SocketAddr, path::PathBuf, time::Duration};

use yog_bootstrap::{
    ConfigError, SecretKey, SecretUrl, duration_var, required, required_secret_key,
    required_secret_url,
};

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

/// An S3-compatible bucket and the credentials that may write to it.
#[derive(Debug)]
pub(crate) struct StoreConfig {
    /// The endpoint, e.g. `https://s3.fr-par.scw.cloud`. Plain `http://` is
    /// accepted for a local MinIO.
    pub(crate) url: String,
    pub(crate) bucket: String,
    pub(crate) region: String,
    /// Access key id and secret. The key is meant to be **write-only**: a
    /// compromised server must not be able to delete the backups.
    pub(crate) access_key: SecretKey,
    pub(crate) secret_key: SecretKey,
}

impl Config {
    pub(crate) fn load() -> Result<Self, ConfigError> {
        Ok(Self {
            database_url: required_secret_url("DATABASE_URL_ARCHIVE")?,
            interval: interval(duration_var(
                "ARCHIVE_INTERVAL_SECS",
                DEFAULT_INTERVAL_SECS,
            )?)?,
            store: StoreConfig {
                url: required("ARCHIVE_STORE_URL")?,
                bucket: required("ARCHIVE_STORE_BUCKET")?,
                region: required("ARCHIVE_STORE_REGION")?,
                access_key: required_secret_key("ARCHIVE_STORE_ACCESS_KEY")?,
                secret_key: required_secret_key("ARCHIVE_STORE_SECRET_KEY")?,
            },
            heartbeat_url: required_secret_url("ARCHIVE_HEARTBEAT_URL")?,
            metrics_addr: metrics_addr()?,
            pg_dump: optional("ARCHIVE_PG_DUMP")
                .unwrap_or_else(|| "pg_dump".into())
                .into(),
            pg_restore: optional("ARCHIVE_PG_RESTORE")
                .unwrap_or_else(|| "pg_restore".into())
                .into(),
        })
    }
}

/// An optional variable, trimmed, with a blank value read as absent — the
/// rule `yog_bootstrap::required` applies, minus the refusal.
fn optional(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
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
