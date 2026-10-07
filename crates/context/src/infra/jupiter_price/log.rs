//! The Jupiter price client's log lines, one function each.

use std::time::Duration;

use tracing::{info, warn};

use crate::error::SourceError;

/// A chunk was rate-limited, and is retried after `delay`.
pub(super) fn rate_limited(attempt: u32, delay: Duration, chunk_size: usize) {
    warn!(
        attempt,
        delay_ms = delay.as_millis() as u64,
        chunk_size,
        "jupiter_price: rate-limited, backing off before retry",
    );
}

/// A chunk failed for good: its mints are in neither list of the answer.
pub(super) fn chunk_failed(error: &SourceError, chunk_size: usize) {
    warn!(
        error = %error,
        chunk_size,
        "jupiter_price: chunk failed, continuing",
    );
}

/// A chunk came back without a single price, and is read as degraded.
pub(super) fn degraded_answer(chunk_size: usize) {
    warn!(
        chunk_size,
        "jupiter_price: chunk answered without a single price — \
         read as a degraded answer, its mints keep their schedule",
    );
}

/// The stop came: the mints past `finished` are abandoned, those of a chunk
/// in flight included.
pub(super) fn stopped(finished: usize, total: usize) {
    info!(
        finished,
        abandoned = total - finished,
        "jupiter_price: stop asked — the chunk in flight is dropped, no further chunk is sent",
    );
}
