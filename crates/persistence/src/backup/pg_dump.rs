//! `pg_dump`, run as a subprocess and handed back as a stream of bytes.
//!
//! The dump is `pg_dump`'s: it is the only tool that exports a TimescaleDB
//! database faithfully, and this crate launches it rather than replacing it.
//! What that costs is a version contract — `pg_dump` refuses a server of a
//! newer major than itself — which is what [`PgTools::ensure_matches`]
//! checks before every dump.

use std::{path::PathBuf, process::Stdio};

use tokio::{
    io::AsyncReadExt,
    process::{Child, ChildStdout, Command},
    task::JoinHandle,
};

use super::{DumpConnection, ServerVersions, read_tail};
use crate::error::BackupError;

/// The `pg_dump` and `pg_restore` to run.
///
/// Plain names resolve on `PATH`; a full path lets a host with several
/// Postgres clients point at the right major, and lets tests substitute
/// fakes.
pub struct PgTools {
    pub(super) pg_dump: PathBuf,
    pub(super) pg_restore: PathBuf,
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
    /// [`PgDump::read`].
    ///
    /// `--no-password` makes a missing or wrong password fail at once instead
    /// of waiting on a prompt nobody will answer. Fails only with `Spawn`.
    pub fn start_dump(&self, connection: &DumpConnection) -> Result<PgDump, BackupError> {
        let mut command = Command::new(&self.pg_dump);
        command
            .arg("--format=custom")
            .arg("--no-password")
            .arg("--dbname")
            .arg(&connection.url)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(password) = &connection.password {
            command.env("PGPASSWORD", password);
        }
        let mut child = command.spawn().map_err(|source| BackupError::Spawn {
            program: self.pg_dump.display().to_string(),
            source,
        })?;
        let stdout = child.stdout.take().expect("stdout is piped");
        let stderr = tokio::spawn(read_tail(child.stderr.take().expect("stderr is piped")));
        Ok(PgDump {
            child,
            stdout,
            stderr,
        })
    }
}

/// A running `pg_dump`.
///
/// **Dropping it kills `pg_dump`.** That is the whole of how a caller gives
/// up on a dump — a stop, a bucket that refuses a part, a check that fails —
/// and so the only way it can leave none running behind it.
pub struct PgDump {
    child: Child,
    stdout: ChildStdout,
    /// Read while the dump runs, so that a full stderr pipe can never block it.
    stderr: JoinHandle<String>,
}

impl PgDump {
    /// The next bytes of the archive into `buf`; `0` once it is complete.
    /// Cancel-safe: dropping the future loses no byte already read.
    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize, BackupError> {
        self.stdout.read(buf).await.map_err(BackupError::Read)
    }

    /// Wait for `pg_dump` to exit. A failure carries the end of its stderr,
    /// which is where it says why (a refused password, a missing role).
    pub async fn finish(mut self) -> Result<(), BackupError> {
        let status = self.child.wait().await.map_err(BackupError::DumpWait)?;
        if status.success() {
            return Ok(());
        }
        let stderr = self.stderr.await.unwrap_or_default();
        Err(BackupError::DumpFailed { status, stderr })
    }
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
#[path = "pg_dump_tests.rs"]
mod tests;
