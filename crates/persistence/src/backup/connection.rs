//! Where `pg_dump` connects.

use yog_bootstrap::{SecretKey, SecretUrl};

use crate::error::BackupError;

/// The URL with no password, passed to `pg_dump` as an argument, and the
/// password alone, passed as `PGPASSWORD`.
///
/// An argument is readable by any user of the host in `/proc/<pid>/cmdline`
/// for as long as `pg_dump` runs; a process's environment is readable only by
/// its owner. The split itself is [`SecretUrl::split_password`]'s; the
/// password stays a [`SecretKey`] until the line that hands it over.
///
/// Built apart from [`PgTools::start_dump`](super::PgTools::start_dump) so
/// that a URL `pg_dump` cannot use is refused before anything else starts.
pub struct DumpConnection {
    pub(super) url: String,
    pub(super) password: Option<SecretKey>,
}

impl DumpConnection {
    /// Fails only with `InvalidUrl`, which does not quote the value.
    pub fn from_secret(url: &SecretUrl) -> Result<Self, BackupError> {
        let (url, password) = url.split_password().ok_or(BackupError::InvalidUrl)?;
        Ok(Self { url, password })
    }
}
