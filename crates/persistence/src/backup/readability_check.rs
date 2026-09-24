//! `pg_restore` reading the archive as it is produced.

use tokio::{
    io::AsyncWriteExt,
    process::{Child, ChildStdin},
    task::JoinHandle,
};

use super::read_tail;
use crate::error::BackupError;

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
    /// Take over a `pg_restore` started with piped stdin and stderr, and
    /// `kill_on_drop` — [`PgTools::start_check`](super::PgTools::start_check).
    pub(super) fn new(mut child: Child) -> Self {
        let stdin = child.stdin.take();
        let stderr = tokio::spawn(read_tail(child.stderr.take().expect("stderr is piped")));
        Self {
            child,
            stdin,
            stderr,
        }
    }

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
