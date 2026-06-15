//! Pool-mint resolver worker — fills the mints of newly discovered pools.
//!
//! The indexer records a pool by address with `NULL` mints (it can't infer
//! them reliably from the transaction). Every `poll_interval` this worker:
//!   1. lists pools whose mints are still `NULL` (`list_unresolved`);
//!   2. fetches and decodes each pool's on-chain account (the authoritative
//!      mint source) via `PoolAccountSource`;
//!   3. writes the resolved mints back (`set_mints`).
//!
//! # Resilience
//!
//! Like the other yog-context workers, a single failed tick must not bring
//! the daemon down: source and per-pool persistence errors are logged and the
//! loop continues. It must run before metadata/price enrichment can do
//! anything — those key off the resolved mints.

use std::sync::Arc;
use std::time::Instant;

use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use yog_core::domain::PoolMintResolver;

use crate::error::WorkerError;
use crate::source::PoolAccountSource;

/// Max pools resolved per tick. Matches `getMultipleAccounts`' 100-key cap,
/// so a tick is a single RPC round-trip.
const RESOLVE_BATCH_MAX: i64 = 100;

pub struct PoolMintsWorker {
    repository: Arc<dyn PoolMintResolver>,
    source: Arc<dyn PoolAccountSource>,
    poll_interval: std::time::Duration,
}

impl PoolMintsWorker {
    pub fn new(
        repository: Arc<dyn PoolMintResolver>,
        source: Arc<dyn PoolAccountSource>,
        poll_interval: std::time::Duration,
    ) -> Self {
        Self {
            repository,
            source,
            poll_interval,
        }
    }

    pub async fn run(self, shutdown: CancellationToken) -> Result<(), WorkerError> {
        info!("PoolMintsWorker started");
        let mut ticker = tokio::time::interval(self.poll_interval);
        loop {
            tokio::select! {
                _ = ticker.tick() => self.run_one_cycle().await,
                _ = shutdown.cancelled() => {
                    info!("shutdown requested — pool-mints worker stopping");
                    return Ok(());
                }
            }
        }
    }

    async fn run_one_cycle(&self) {
        let start = Instant::now();
        let unresolved = match self.repository.list_unresolved(RESOLVE_BATCH_MAX).await {
            Ok(pools) => pools,
            Err(e) => {
                warn!(error = %e, "pool-mints worker: list_unresolved failed");
                return;
            }
        };
        if unresolved.is_empty() {
            debug!("pool-mints worker: no unresolved pools — sleeping");
            return;
        }

        let resolved = match self.source.fetch_mints(&unresolved).await {
            Ok(resolved) => resolved,
            Err(e) => {
                warn!(error = %e, "pool-mints worker: source returned a hard error");
                return;
            }
        };

        let mut ok = 0usize;
        for r in &resolved {
            match self
                .repository
                .set_mints(&r.pool, &r.token_a_mint, &r.token_b_mint)
                .await
            {
                Ok(()) => ok += 1,
                Err(e) => warn!(pool = %r.pool, error = %e, "pool-mints worker: set_mints failed"),
            }
        }

        debug!(
            requested = unresolved.len(),
            resolved = resolved.len(),
            written = ok,
            elapsed_s = start.elapsed().as_secs_f64(),
            "pool-mints worker: cycle done",
        );
    }
}

#[cfg(test)]
#[path = "pool_mints_tests.rs"]
mod tests;
