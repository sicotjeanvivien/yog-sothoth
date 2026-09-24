//! `pg_restore`, run as a subprocess to check an archive while it is produced.

use std::process::Stdio;

use tokio::{
    io::AsyncWriteExt,
    process::{Child, ChildStdin, Command},
    task::JoinHandle,
};

use super::{PgTools, read_tail};
use crate::error::BackupError;

impl PgTools {
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
        let mut child = Command::new(&self.pg_restore)
            .arg("--file=/dev/null")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|source| BackupError::Spawn {
                program: self.pg_restore.display().to_string(),
                source,
            })?;
        let stdin = child.stdin.take();
        let stderr = tokio::spawn(read_tail(child.stderr.take().expect("stderr is piped")));
        Ok(ReadabilityCheck {
            child,
            stdin,
            stderr,
        })
    }
}

/// A `pg_restore` reading the archive as `pg_dump` writes it. Dropping it
/// kills `pg_restore`.
///
/// Its exit status is the verdict. Should it stop reading early, the broken
/// pipe that follows is not a failure of its own: feeding stops, and the exit
/// status says why it stopped.
pub struct ReadabilityCheck {
    child: Child,
    stdin: Option<ChildStdin>,
    stderr: JoinHandle<String>,
}

impl ReadabilityCheck {
    pub async fn feed(&mut self, chunk: &[u8]) {
        if let Some(stdin) = self.stdin.as_mut()
            && stdin.write_all(chunk).await.is_err()
        {
            self.stdin = None;
        }
    }

    /// Close the input and wait for the verdict. Fails with `CheckWait` or
    /// `Unreadable`.
    pub async fn finish(mut self) -> Result<(), BackupError> {
        drop(self.stdin.take());
        let status = self.child.wait().await.map_err(BackupError::CheckWait)?;
        if status.success() {
            return Ok(());
        }
        let stderr = self.stderr.await.unwrap_or_default();
        Err(BackupError::Unreadable { status, stderr })
    }
}
