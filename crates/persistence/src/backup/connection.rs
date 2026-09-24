//! The connection string, split the way libpq wants it without leaking it.

use percent_encoding::percent_decode_str;
use yog_bootstrap::SecretUrl;

use crate::error::BackupError;

/// Where `pg_dump` connects: the URL with no password, passed as an
/// argument, and the password alone, passed as `PGPASSWORD`.
///
/// An argument is readable by any user of the host in `/proc/<pid>/cmdline`
/// for as long as `pg_dump` runs; a process's environment is readable only by
/// its owner. Deliberately not `Debug`, and no field is public: it holds the
/// password in clear.
///
/// Built apart from [`PgTools::start_dump`](super::PgTools::start_dump) so
/// that a URL `pg_dump` cannot use is refused before anything else starts.
pub struct DumpConnection {
    pub(super) url: String,
    pub(super) password: Option<String>,
}

impl DumpConnection {
    pub fn from_secret(url: &SecretUrl) -> Result<Self, BackupError> {
        let mut parsed = url::Url::parse(url.expose()).map_err(|_| BackupError::InvalidUrl)?;
        let password = parsed
            .password()
            .map(|p| percent_decode_str(p).decode_utf8_lossy().into_owned());
        // Only when there is one: `set_password` refuses a URL with no host,
        // and a socket URL (`postgresql:///db?host=/var/run/postgresql`) is
        // one libpq accepts and carries no password to remove.
        if password.is_some() {
            parsed
                .set_password(None)
                .map_err(|()| BackupError::InvalidUrl)?;
        }
        Ok(Self {
            url: parsed.to_string(),
            password,
        })
    }
}

#[cfg(test)]
#[path = "connection_tests.rs"]
mod tests;
