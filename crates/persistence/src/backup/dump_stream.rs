//! A dump being produced, read chunk by chunk.

use tokio::{
    io::AsyncReadExt,
    process::{Child, ChildStdout},
    task::JoinHandle,
};

use super::read_tail;
use crate::error::BackupError;

/// A dump being produced by `pg_dump`, read chunk by chunk.
///
/// **Dropping it kills `pg_dump`.** That is the whole of how a caller gives
/// up on a dump — a stop, a bucket that refuses a part, a check that fails —
/// and so the only way it can leave none running behind it.
pub struct DumpStream {
    child: Child,
    stdout: ChildStdout,
    /// Read while the dump runs, so that a full stderr pipe can never block it.
    stderr: JoinHandle<String>,
}

impl DumpStream {
    /// Take over a `pg_dump` started with piped stdout and stderr, and
    /// `kill_on_drop` — [`PgTools::start_dump`](super::PgTools::start_dump).
    pub(super) fn new(mut child: Child) -> Self {
        let stdout = child.stdout.take().expect("stdout is piped");
        let stderr = tokio::spawn(read_tail(child.stderr.take().expect("stderr is piped")));
        Self {
            child,
            stdout,
            stderr,
        }
    }

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

#[cfg(test)]
#[path = "dump_stream_tests.rs"]
mod tests;
