//! The Postgres client programs a backup runs, and how each is started.
//!
//! The dump is `pg_dump`'s: it is the only tool that exports a TimescaleDB
//! database faithfully, and this crate launches it rather than replacing it.
//! What that costs is a version contract — `pg_dump` refuses a server of a
//! newer major than itself — which is what [`PgTools::ensure_matches`]
//! checks before every dump.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

use tokio::process::{Child, Command};
use yog_bootstrap::SecretUrl;

use super::{DumpStream, ReadabilityCheck};
use crate::{ServerVersions, error::BackupError};

/// The `pg_dump` and `pg_restore` to run.
///
/// Plain names resolve on `PATH`; a full path lets a host with several
/// Postgres clients point at the right major, and lets tests substitute
/// fakes.
pub struct PgTools {
    pg_dump: PathBuf,
    pg_restore: PathBuf,
}

impl PgTools {
    pub fn new(pg_dump: PathBuf, pg_restore: PathBuf) -> Self {
        Self {
            pg_dump,
            pg_restore,
        }
    }

    /// Refuse a `pg_dump` whose major is not the server's.
    ///
    /// Reads `pg_dump --version` (`pg_dump (PostgreSQL) 16.14`, possibly
    /// followed by a distribution suffix). Fails with `Spawn`, `Version` or
    /// `MajorMismatch`, and in every case nothing has been dumped.
    pub async fn ensure_matches(&self, server: &ServerVersions) -> Result<(), BackupError> {
        let client = self.pg_dump_major().await?;
        if client != server.postgres_major {
            return Err(BackupError::MajorMismatch {
                client,
                server: server.postgres_major,
            });
        }
        Ok(())
    }

    async fn pg_dump_major(&self) -> Result<u32, BackupError> {
        let program = self.pg_dump.display().to_string();
        let output = Command::new(&self.pg_dump)
            .arg("--version")
            .output()
            .await
            .map_err(|source| BackupError::Spawn {
                program: program.clone(),
                source,
            })?;
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        parse_major(&text).ok_or(BackupError::Version {
            program,
            output: text.trim().to_string(),
        })
    }

    /// Start `pg_dump` in custom format, the archive coming out of
    /// [`DumpStream::read`].
    ///
    /// `pg_dump` opens **its own** connection: it is a separate program and
    /// cannot borrow a pool of ours. With the one that reads the server's
    /// versions, a run opens two, one after the other, every six hours — and
    /// holds none between runs, which is what lets a refusing database end a
    /// run in a signalled failure instead of stopping the process.
    ///
    /// The password does not travel with the URL: `pg_dump` receives the URL
    /// without it as an argument — readable by any user of the host in
    /// `/proc/<pid>/cmdline` — and the password alone in `PGPASSWORD`, which
    /// only the process's owner can read. [`SecretUrl::split_password`] does
    /// the split.
    ///
    /// `--no-password` makes a missing or wrong password fail at once instead
    /// of waiting on a prompt nobody will answer. Fails with `InvalidUrl`,
    /// which does not quote the value, or `Spawn`.
    pub fn start_dump(&self, url: &SecretUrl) -> Result<DumpStream, BackupError> {
        let (url, password) = url.split_password().ok_or(BackupError::InvalidUrl)?;
        let mut command = Command::new(&self.pg_dump);
        command
            .arg("--format=custom")
            .arg("--no-password")
            .arg("--dbname")
            .arg(&url)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(password) = &password {
            command.env("PGPASSWORD", password.expose());
        }
        Ok(DumpStream::new(spawn(&self.pg_dump, &mut command)?))
    }

    /// Start `pg_restore --file=/dev/null`, to be fed the archive while it
    /// is produced.
    ///
    /// Restoring **to a script file** makes `pg_restore` read and decompress
    /// every data block to write it out, which is what a check needs; the
    /// file is `/dev/null`. Measured on 23 September 2026 against real
    /// dumps: a dump cut in half, and one with 5,000 bytes zeroed in its data,
    /// both fail it (`end of file`, `could not uncompress data`); a 172 MB
    /// dump passes in about 2 s.
    ///
    /// ⚠️ **Not `--list`**, which the first version used: it reads the table
    /// of contents and exits, so both damaged dumps above passed it. The
    /// claim that `--list` read the whole archive rested on a test that could
    /// not fail — `(cat …; echo done)` echoes even when `cat` dies of the
    /// broken pipe.
    ///
    /// Fed the whole stream rather than a head of it for the same reason:
    /// the table of contents alone grows with every chunk (423 KiB for 48),
    /// so any fixed head would one day cut it.
    ///
    /// What it does not prove: that the dump restores **into a database** —
    /// constraints, extensions, versions. Only a restore proves that.
    ///
    /// Fails only with `Spawn`.
    pub fn start_check(&self) -> Result<ReadabilityCheck, BackupError> {
        let mut command = Command::new(&self.pg_restore);
        command
            .arg("--file=/dev/null")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        Ok(ReadabilityCheck::new(spawn(
            &self.pg_restore,
            &mut command,
        )?))
    }
}

/// Start a program that dies with the value holding it.
///
/// `kill_on_drop` is set **here**, for every program a backup starts, so that
/// no caller can forget it: dropping a [`DumpStream`] or a
/// [`ReadabilityCheck`] is how a run gives up on it.
fn spawn(program: &Path, command: &mut Command) -> Result<Child, BackupError> {
    command
        .kill_on_drop(true)
        .spawn()
        .map_err(|source| BackupError::Spawn {
            program: program.display().to_string(),
            source,
        })
}

fn parse_major(version_output: &str) -> Option<u32> {
    version_output
        .split("(PostgreSQL)")
        .nth(1)?
        .trim()
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

#[cfg(test)]
#[path = "pg_tools_tests.rs"]
mod tests;
