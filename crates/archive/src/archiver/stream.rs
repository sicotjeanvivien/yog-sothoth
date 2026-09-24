//! The archive itself: `pg_dump`'s output read chunk by chunk, fed to
//! `pg_restore` for the check and written to the open upload.

use object_store::WriteMultipart;
use tokio_util::sync::CancellationToken;
use yog_bootstrap::SecretUrl;
use yog_persistence::PgTools;

use super::{Interrupted, RunFailure};

/// Parts uploaded concurrently before the reader waits. With
/// [`PART_SIZE`](super::PART_SIZE), the upload holds at most ~24 MiB (two in
/// flight, one filling).
const PARTS_IN_FLIGHT: usize = 2;

/// Dump into the open upload while `pg_restore` checks the same bytes, and
/// return how many were written.
///
/// `dump` and `check` are locals: **any return drops them, which kills
/// `pg_dump` and `pg_restore`** before the caller aborts the upload. That
/// order is what `a_stop_mid_dump_kills_pg_dump…` checks.
pub(super) async fn stream(
    tools: &PgTools,
    url: &SecretUrl,
    writer: &mut WriteMultipart,
    cancel: &CancellationToken,
) -> Result<u64, Interrupted> {
    let mut dump = tools.start_dump(url).map_err(RunFailure::dump_failed)?;
    let mut check = tools.start_check().map_err(RunFailure::unreadable)?;

    let mut bytes: u64 = 0;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let read = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(Interrupted::Cancelled),
            read = dump.read(&mut buf) => read,
        };
        let n = read.map_err(RunFailure::dump_failed)?;
        if n == 0 {
            break;
        }
        check.feed(&buf[..n]).await;
        writer
            .wait_for_capacity(PARTS_IN_FLIGHT)
            .await
            .map_err(|e| RunFailure::store_failed(format!("a part was refused: {e}")))?;
        writer.write(&buf[..n]);
        bytes += n as u64;
    }

    // `dump` moves into `finish`; a stop drops that future, and `dump` with
    // it, when the `select!` returns.
    tokio::select! {
        biased;
        () = cancel.cancelled() => return Err(Interrupted::Cancelled),
        finished = dump.finish() => finished.map_err(RunFailure::dump_failed)?,
    }
    check.finish().await.map_err(RunFailure::unreadable)?;
    Ok(bytes)
}
