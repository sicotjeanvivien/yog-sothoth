//! Pool-account resolver worker: for each protocol, reads the accounts of the pools its resolver
//! queues, decodes them in `core` (routed on the owner), and writes the satellite and `pools`.
//! ⚠️ The only writer of account-derived pool properties: the indexer raises `needs_refresh`
//! instead. It runs before metadata and price enrichment, which key off the mints it resolves.

use std::sync::Arc;
use std::time::Instant;

use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use yog_core::application::decode_pool_account;
use yog_core::domain::{PoolAccountResolver, PoolRepository};

use crate::application::source::PoolAccountSource;
use crate::error::WorkerError;

/// Max pools resolved per tick: `getMultipleAccounts`' 100-key cap, so one RPC round-trip.
const RESOLVE_BATCH_MAX: i64 = 100;

pub struct PoolAccountWorker {
    /// One per protocol, each owning its queue and its satellite; the worker only iterates.
    resolvers: Vec<Arc<dyn PoolAccountResolver>>,
    /// The neutral `pools` registry, shared by every protocol.
    pool_repository: Arc<dyn PoolRepository>,
    source: Arc<dyn PoolAccountSource>,
    poll_interval: std::time::Duration,
}

impl PoolAccountWorker {
    pub fn new(
        resolvers: Vec<Arc<dyn PoolAccountResolver>>,
        pool_repository: Arc<dyn PoolRepository>,
        source: Arc<dyn PoolAccountSource>,
        poll_interval: std::time::Duration,
    ) -> Self {
        Self {
            resolvers,
            pool_repository,
            source,
            poll_interval,
        }
    }

    pub async fn run(self, shutdown: CancellationToken) -> Result<(), WorkerError> {
        info!("PoolAccountWorker started");
        let mut ticker = tokio::time::interval(self.poll_interval);
        loop {
            tokio::select! {
            // ⚠️ `biased`: with the default `Burst`, a cycle that outruns the cadence leaves `tick()`
            // ready, and an unbiased pick would start a new cycle after a stop was asked for.
            biased;

                _ = shutdown.cancelled() => {
                    info!("shutdown requested — pool-account worker stopping");
                    return Ok(());
                }
                _ = ticker.tick() => self.run_one_cycle().await,
            }
        }
    }

    /// One pass over every protocol; a failure for one does not stop the others.
    async fn run_one_cycle(&self) {
        for resolver in &self.resolvers {
            self.run_one_protocol(resolver.as_ref()).await;
        }
    }

    async fn run_one_protocol(&self, resolver: &dyn PoolAccountResolver) {
        let protocol = resolver.protocol();
        let start = Instant::now();

        let unresolved = match resolver.list_unresolved(RESOLVE_BATCH_MAX).await {
            Ok(pools) => pools,
            Err(e) => {
                warn!(protocol = %protocol, error = %e, "pool-account worker: list_unresolved failed");
                return;
            }
        };
        if unresolved.is_empty() {
            debug!(protocol = %protocol, "pool-account worker: no unresolved pools");
            return;
        }

        let accounts = match self.source.fetch_accounts(&unresolved).await {
            Ok(accounts) => accounts,
            Err(e) => {
                warn!(protocol = %protocol, error = %e, "pool-account worker: source returned a hard error");
                return;
            }
        };

        let mut decoded = 0usize;
        let mut rejected = 0usize;
        let mut ok = 0usize;
        for account in &accounts {
            // Routed on the program id: a pool that changed hands is rejected rather than decoded
            // at the wrong layout. Every rejection is logged, none being routine on this queue.
            let decoded_account = match decode_pool_account(&account.program_id, &account.data) {
                Ok(decoded_account) => decoded_account,
                Err(rejection) => {
                    warn!(
                        protocol = %protocol,
                        pool = %account.pool_address,
                        reason = %rejection,
                        "pool-account worker: account rejected",
                    );
                    rejected += 1;
                    continue;
                }
            };
            decoded += 1;

            if decoded_account.protocol() != protocol {
                warn!(
                    protocol = %protocol,
                    decoded_as = %decoded_account.protocol(),
                    pool = %account.pool_address,
                    "pool-account worker: queue and account disagree on protocol — skipping",
                );
                continue;
            }

            // ⚠️ Satellite first, registry last: the registry write lowers `needs_refresh`, so a
            // failure before it leaves the pool queued. No transaction: both halves are idempotent.
            if let Err(e) = resolver
                .set_pool_account(&account.pool_address, &decoded_account.properties)
                .await
            {
                warn!(protocol = %protocol, pool = %account.pool_address, error = %e,
                      "pool-account worker: set_pool_account failed");
                continue;
            }

            match self
                .pool_repository
                .set_registry_properties(&account.pool_address, &decoded_account.registry)
                .await
            {
                Ok(()) => ok += 1,
                Err(e) => {
                    warn!(protocol = %protocol, pool = %account.pool_address, error = %e,
                          "pool-account worker: set_registry_properties failed")
                }
            }
        }

        debug!(
            protocol = %protocol,
            requested = unresolved.len(),
            fetched = accounts.len(),
            decoded,
            rejected,
            written = ok,
            elapsed_s = start.elapsed().as_secs_f64(),
            "pool-account worker: cycle done",
        );
    }
}

#[cfg(test)]
#[path = "pool_account/pool_account_tests.rs"]
mod tests;
