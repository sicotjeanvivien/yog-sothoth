//! The Postgres client programs a backup runs. ⚠️ `pg_dump` refuses a server of a newer
//! major than itself: [`PgTools::ensure_matches`] checks before every dump.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

use tokio::process::{Child, Command};
use yog_bootstrap::SecretUrl;

use super::{DumpStream, ReadabilityCheck};
use crate::{ServerVersions, error::BackupError};

/// The `pg_dump` and `pg_restore` to run: plain names resolve on `PATH`, a full
/// path picks a major or a test fake.
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

    /// Refuses a `pg_dump` whose major is not the server's, read from `pg_dump --version`
    /// (`pg_dump (PostgreSQL) 16.14`, maybe with a distribution suffix). Nothing is dumped yet.
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

    /// Starts `pg_dump` in custom format, on its own connection, the archive coming out of
    /// [`DumpStream::read`]. ⚠️ The password goes in `PGPASSWORD`, not the URL argument that
    /// `/proc/<pid>/cmdline` shows. `--no-password` fails at once rather than prompting.
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
        Ok(DumpStream::take_over(spawn(&self.pg_dump, &mut command)?))
    }

    /// Starts `pg_restore --file=/dev/null`, fed the whole archive while it is produced. It
    /// proves the archive reads, not that it restores into a database. ⚠️ Not `--list`: it reads
    /// only the table of contents, and a truncated or corrupt dump passes it.
    pub fn start_check(&self) -> Result<ReadabilityCheck, BackupError> {
        let mut command = Command::new(&self.pg_restore);
        command
            .arg("--file=/dev/null")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        Ok(ReadabilityCheck::take_over(spawn(
            &self.pg_restore,
            &mut command,
        )?))
    }
}

/// Starts a program that dies with the value holding it. ⚠️ `kill_on_drop` is set here
/// for every program: dropping a [`DumpStream`] or a [`ReadabilityCheck`] is how a run gives up.
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
