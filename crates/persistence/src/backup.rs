//! Backing up the database: `pg_dump` takes a dump, `pg_restore` checks it, both run as
//! subprocesses that must be installed where the caller runs (`yog-archive`'s image).
//! A dump is a [`DumpStream`] read chunk by chunk, a check a [`ReadabilityCheck`] fed the
//! same chunks; dropping either kills the program behind it.

mod dump_stream;
mod pg_tools;
mod readability_check;

pub use dump_stream::DumpStream;
pub use pg_tools::PgTools;
pub use readability_check::ReadabilityCheck;

use tokio::io::{AsyncRead, AsyncReadExt};

/// How much of a program's complaint is kept in an error: enough to name
/// the problem, not so much that a failure reason becomes a log dump.
const MESSAGE_TAIL: usize = 600;

/// Reads a child's stream to its end, keeping the last [`MESSAGE_TAIL`] characters.
/// ⚠️ Run while the child works: a full stderr pipe would block it.
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
