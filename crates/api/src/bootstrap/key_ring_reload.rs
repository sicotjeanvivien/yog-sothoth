//! The key ring's rereads, on a period, until the stop.

use std::{convert::Infallible, time::Duration};

use tokio_util::sync::CancellationToken;

use crate::application::KeyRing;

/// How often the ring rereads the keys: a revocation takes effect within it.
pub(super) const KEY_RING_RELOAD: Duration = Duration::from_secs(10);

/// Reload `ring` every `period` until `shutdown`. The `Result` is for [`yog_bootstrap::Stop::settle`].
pub(super) async fn reload_key_ring(
    ring: KeyRing,
    period: Duration,
    shutdown: CancellationToken,
) -> Result<(), Infallible> {
    loop {
        tokio::select! {
            () = shutdown.cancelled() => return Ok(()),
            () = tokio::time::sleep(period) => {}
        }
        // ⚠️ Raced too: a read waiting on a saturated pool would otherwise hold the stop.
        tokio::select! {
            () = shutdown.cancelled() => return Ok(()),
            _ = ring.reload() => {}
        }
    }
}

#[cfg(test)]
#[path = "key_ring_reload_tests.rs"]
mod tests;
