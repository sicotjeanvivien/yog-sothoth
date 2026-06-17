//! Pool-account resolver worker — fills account-derived pool properties.
//!
//! The indexer records a pool by address with `NULL` mints (it can't infer
//! them reliably from the transaction) and a `NULL` `fee_bps` for any pool
//! whose genesis `InitializePool` event it never saw. Every `poll_interval`
//! this worker:
//!   1. lists pools missing any account-derived property (`list_unresolved`:
//!      a NULL mint, a NULL fee, or a NULL fee-split percent);
//!   2. fetches and decodes each pool's on-chain account (the authoritative
//!      source for mints, base fee and fee-split percents) via
//!      `PoolAccountSource`;
//!   3. writes the resolved mints + fee + percents back (`set_pool_account`).
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

use yog_core::domain::PoolAccountResolver;

use crate::error::WorkerError;
use crate::source::PoolAccountSource;

/// Max pools resolved per tick. Matches `getMultipleAccounts`' 100-key cap,
/// so a tick is a single RPC round-trip.
const RESOLVE_BATCH_MAX: i64 = 100;

pub struct PoolAccountWorker {
    repository: Arc<dyn PoolAccountResolver>,
    source: Arc<dyn PoolAccountSource>,
    poll_interval: std::time::Duration,
}

impl PoolAccountWorker {
    pub fn new(
        repository: Arc<dyn PoolAccountResolver>,
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
        info!("PoolAccountWorker started");
        let mut ticker = tokio::time::interval(self.poll_interval);
        loop {
            tokio::select! {
                _ = ticker.tick() => self.run_one_cycle().await,
                _ = shutdown.cancelled() => {
                    info!("shutdown requested — pool-account worker stopping");
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
                warn!(error = %e, "pool-account worker: list_unresolved failed");
                return;
            }
        };
        if unresolved.is_empty() {
            debug!("pool-account worker: no unresolved pools — sleeping");
            return;
        }

        let resolved = match self.source.fetch_accounts(&unresolved).await {
            Ok(resolved) => resolved,
            Err(e) => {
                warn!(error = %e, "pool-account worker: source returned a hard error");
                return;
            }
        };

        let mut ok = 0usize;
        for r in &resolved {
            match self
                .repository
                .set_pool_account(
                    &r.pool,
                    &r.token_a_mint,
                    &r.token_b_mint,
                    r.fee_bps,
                    r.protocol_fee_percent,
                    r.partner_fee_percent,
                    r.referral_fee_percent,
                )
                .await
            {
                Ok(()) => ok += 1,
                Err(e) => {
                    warn!(pool = %r.pool, error = %e, "pool-account worker: set_pool_account failed")
                }
            }
        }

        debug!(
            requested = unresolved.len(),
            resolved = resolved.len(),
            written = ok,
            elapsed_s = start.elapsed().as_secs_f64(),
            "pool-account worker: cycle done",
        );
    }
}

#[cfg(test)]
#[path = "pool_account_tests.rs"]
mod tests;
