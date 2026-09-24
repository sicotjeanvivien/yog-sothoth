//! `pg_dump` and `pg_restore`, run as subprocesses.
//!
//! The dump is `pg_dump`'s: it is the only tool that exports a TimescaleDB
//! database faithfully, and this crate launches it rather than replacing it.
//! What that costs is a version contract — `pg_dump` refuses a server of a
//! newer major than itself — which is why [`PgTools::pg_dump_major`] exists
//! and why the archiver compares it with the server's before every dump.

use std::{path::PathBuf, process::Stdio};

use percent_encoding::percent_decode_str;
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::{Child, ChildStdin, Command},
    task::JoinHandle,
};
use yog_bootstrap::SecretUrl;

/// How much of `pg_restore`'s complaint is kept in an outcome: enough to name
/// the problem, not so much that a failure reason becomes a log dump.
const MESSAGE_TAIL: usize = 600;

#[derive(Debug, Error)]
pub(crate) enum DumpError {
    /// The connection string does not parse. The value is not quoted: it
    /// carries the password.
    #[error("DATABASE_URL_ARCHIVE is not a valid URL")]
    Connection,

    #[error("cannot run `{program}`: {source}")]
    Spawn {
        program: String,
        source: std::io::Error,
    },

    #[error("cannot read the major version from `{program} --version`: {output}")]
    Version { program: String, output: String },
}

/// A connection string split the way libpq wants it without leaking it: the
/// URL with no password, passed as an argument, and the password alone,
/// passed as `PGPASSWORD`.
///
/// An argument is readable by any user of the host in `/proc/<pid>/cmdline`
/// for as long as `pg_dump` runs; a process's environment is readable only by
/// its owner. Deliberately not `Debug`: it holds the password in clear.
pub(crate) struct Connection {
    url: String,
    password: Option<String>,
}

impl Connection {
    pub(crate) fn from_secret(url: &SecretUrl) -> Result<Self, DumpError> {
        let mut parsed = url::Url::parse(url.expose()).map_err(|_| DumpError::Connection)?;
        let password = parsed
            .password()
            .map(|p| percent_decode_str(p).decode_utf8_lossy().into_owned());
        // Only when there is one: `set_password` refuses a URL with no host,
        // and a socket URL (`postgresql:///db?host=/var/run/postgresql`) is
        // one libpq accepts and carries no password to remove.
        if password.is_some() {
            parsed
                .set_password(None)
                .map_err(|()| DumpError::Connection)?;
        }
        Ok(Self {
            url: parsed.to_string(),
            password,
        })
    }
}

/// The two Postgres client programs the archiver runs.
#[derive(Debug, Clone)]
pub(crate) struct PgTools {
    pub(crate) pg_dump: PathBuf,
    pub(crate) pg_restore: PathBuf,
}

impl PgTools {
    /// The major version of the configured `pg_dump`, read from
    /// `pg_dump --version` (`pg_dump (PostgreSQL) 16.14`, possibly followed
    /// by a distribution suffix).
    pub(crate) async fn pg_dump_major(&self) -> Result<u32, DumpError> {
        let program = self.pg_dump.display().to_string();
        let output = Command::new(&self.pg_dump)
            .arg("--version")
            .output()
            .await
            .map_err(|source| DumpError::Spawn {
                program: program.clone(),
                source,
            })?;
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        parse_major(&text).ok_or(DumpError::Version {
            program,
            output: text.trim().to_string(),
        })
    }

    /// Start `pg_dump` in custom format, writing the archive to its stdout.
    ///
    /// `--no-password` makes a missing or wrong password fail at once instead
    /// of waiting on a prompt nobody will answer. `kill_on_drop` means an
    /// archiver that gives up on a run cannot leave a dump running behind it.
    pub(crate) fn spawn_dump(&self, connection: &Connection) -> Result<Child, DumpError> {
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
        command.spawn().map_err(|source| DumpError::Spawn {
            program: self.pg_dump.display().to_string(),
            source,
        })
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
    pub(crate) fn start_check(&self) -> Result<ReadabilityCheck, String> {
        let mut child = Command::new(&self.pg_restore)
            .arg("--file=/dev/null")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("cannot run `{}`: {e}", self.pg_restore.display()))?;
        let stdin = child.stdin.take();
        let stderr = tokio::spawn(read_tail(child.stderr.take().expect("stderr is piped")));
        Ok(ReadabilityCheck {
            child,
            stdin,
            stderr,
        })
    }
}

/// A `pg_restore` reading the archive as `pg_dump` writes it.
///
/// Its exit status is the verdict. Should it stop reading early, the broken
/// pipe that follows is not a failure of its own: feeding stops, and the exit
/// status says why it stopped.
pub(crate) struct ReadabilityCheck {
    child: Child,
    stdin: Option<ChildStdin>,
    stderr: JoinHandle<String>,
}

impl ReadabilityCheck {
    pub(crate) async fn feed(&mut self, chunk: &[u8]) {
        if let Some(stdin) = self.stdin.as_mut()
            && stdin.write_all(chunk).await.is_err()
        {
            self.stdin = None;
        }
    }

    /// Close the input and wait for the verdict.
    pub(crate) async fn finish(mut self) -> Result<(), String> {
        drop(self.stdin.take());
        let status = self
            .child
            .wait()
            .await
            .map_err(|e| format!("`pg_restore --file=/dev/null` did not finish: {e}"))?;
        if status.success() {
            return Ok(());
        }
        let stderr = self.stderr.await.unwrap_or_default();
        Err(format!(
            "`pg_restore --file=/dev/null` exited with {status}: {stderr}"
        ))
    }
}

/// Read a child's stream to its end and keep the last [`MESSAGE_TAIL`]
/// characters. Run while the child works, so that a full stderr pipe can
/// never block it.
pub(crate) async fn read_tail(mut stream: impl AsyncRead + Unpin) -> String {
    let mut all = Vec::new();
    let _ = stream.read_to_end(&mut all).await;
    tail(&String::from_utf8_lossy(&all))
}

fn tail(text: &str) -> String {
    let text = text.trim();
    let start = text
        .char_indices()
        .rev()
        .nth(MESSAGE_TAIL - 1)
        .map_or(0, |(i, _)| i);
    text[start..].to_string()
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
#[path = "dump_tests.rs"]
mod tests;
