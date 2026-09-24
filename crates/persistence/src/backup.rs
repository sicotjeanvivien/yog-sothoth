//! Backing up the database: the facts a dump depends on, `pg_dump` to take
//! it, `pg_restore` to check it.
//!
//! This is the crate's second way of reaching Postgres. Everything else goes
//! through a `sqlx` pool; a dump goes through the Postgres client programs,
//! run as subprocesses, because `pg_dump` is the only tool that exports a
//! TimescaleDB database faithfully. They must be installed where the caller
//! runs — only `yog-archive` calls this module, and its image carries them.
//!
//! The public surface shows no process: a dump is a [`PgDump`] read chunk by
//! chunk, a check is a [`ReadabilityCheck`] fed the same chunks, and giving
//! up on either is dropping it, which kills the program behind it. What a
//! failure means for a run — refused, failed, unreadable — is the caller's
//! decision, made from the [`BackupError`](crate::BackupError) each step
//! documents.

mod connection;
mod database_info;
mod pg_dump;
mod pg_restore;

pub use connection::DumpConnection;
pub use database_info::{PgDatabaseInfo, ServerVersions};
pub use pg_dump::{PgDump, PgTools};
pub use pg_restore::ReadabilityCheck;

use tokio::io::{AsyncRead, AsyncReadExt};

/// How much of a program's complaint is kept in an error: enough to name
/// the problem, not so much that a failure reason becomes a log dump.
const MESSAGE_TAIL: usize = 600;

/// Read a child's stream to its end and keep the last [`MESSAGE_TAIL`]
/// characters. Run while the child works, so that a full stderr pipe can
/// never block it.
async fn read_tail(mut stream: impl AsyncRead + Unpin) -> String {
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

#[cfg(test)]
#[path = "backup_tests.rs"]
mod tests;
