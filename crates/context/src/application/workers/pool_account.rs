//! Pool-account resolver worker: for each protocol, reads the accounts of the pools its resolver
//! queues, decodes them in `core` (routed on the owner), and writes the satellite and `pools`.
//! ⚠️ The only writer of account-derived pool properties: the indexer raises `needs_refresh`
//! instead. Metadata and price enrichment key off the mints it resolves.

use std::sync::Arc;
use std::time::Instant;

use tokio_util::sync::CancellationToken;

use yog_core::application::decode_pool_account;
use yog_core::domain::{PoolAccountResolver, PoolRepository};

use crate::application::heartbeat::{source_health::SourceHealth, source_role::SourceRole};
use crate::application::source::PoolAccountSource;
use crate::error::WorkerError;

mod log;
mod metrics;

pub(crate) use metrics::PoolAccountWorkerMetrics;

/// Max pools resolved per tick: `getMultipleAccounts`' 100-key cap, so one RPC round-trip.
const RESOLVE_BATCH_MAX: i64 = 100;

pub struct PoolAccountWorker {
    /// One per protocol, each owning its queue and its satellite; the worker only iterates.
    resolvers: Vec<Arc<dyn PoolAccountResolver>>,
    /// The neutral `pools` registry, shared by every protocol.
    pool_repository: Arc<dyn PoolRepository>,
    source: Arc<dyn PoolAccountSource>,
    poll_interval: std::time::Duration,
    health: Arc<SourceHealth>,
}

impl PoolAccountWorker {
    pub fn new(
        resolvers: Vec<Arc<dyn PoolAccountResolver>>,
        pool_repository: Arc<dyn PoolRepository>,
        source: Arc<dyn PoolAccountSource>,
        poll_interval: std::time::Duration,
        health: Arc<SourceHealth>,
    ) -> Self {
        Self {
            resolvers,
            pool_repository,
            source,
            poll_interval,
            health,
        }
    }

    pub async fn run(self, shutdown: CancellationToken) -> Result<(), WorkerError> {
        log::started();
        let mut ticker = tokio::time::interval(self.poll_interval);
        loop {
            tokio::select! {
            // ⚠️ `biased`: with the default `Burst`, a cycle that outruns the cadence leaves `tick()`
            // ready, and an unbiased pick would start a new cycle after a stop was asked for.
            biased;

                _ = shutdown.cancelled() => {
                    log::stopping();
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
                log::list_unresolved_failed(protocol, &e);
                return;
            }
        };
        if unresolved.is_empty() {
            log::nothing_unresolved(protocol);
            return;
        }

        let accounts = match self.source.fetch_accounts(&unresolved).await {
            Ok(accounts) => {
                self.health.record_success(SourceRole::PoolAccount);
                accounts
            }
            Err(e) => {
                self.health
                    .record_failure(SourceRole::PoolAccount, e.to_string());
                log::source_failed(protocol, &e);
                return;
            }
        };

        let mut decoded = 0usize;
        let mut rejected = 0usize;
        let mut ok = 0usize;
        for account in &accounts {
            // Routed on the program id: a pool that changed hands is rejected rather than decoded
            // at the wrong layout. None being routine on this queue, every rejection is counted.
            let decoded_account = match decode_pool_account(&account.program_id, &account.data) {
                Ok(decoded_account) => decoded_account,
                Err(rejection) => {
                    log::rejected(protocol, &account.pool_address, &rejection);
                    PoolAccountWorkerMetrics::record_rejected(protocol, &rejection);
                    rejected += 1;
                    continue;
                }
            };
            decoded += 1;

            if decoded_account.protocol() != protocol {
                log::protocol_disagrees(
                    protocol,
                    decoded_account.protocol(),
                    &account.pool_address,
                );
                PoolAccountWorkerMetrics::record_protocol_mismatch(protocol);
                continue;
            }

            // ⚠️ Satellite first, registry last: the registry write lowers `needs_refresh`, so a
            // failure before it leaves the pool queued. No transaction: both halves are idempotent.
            match resolver
                .set_pool_account(&account.pool_address, &decoded_account.properties)
                .await
            {
                Ok(out_of_range) => {
                    for degradation in &decoded_account.degradations {
                        log::degraded(protocol, &account.pool_address, degradation);
                        PoolAccountWorkerMetrics::record_degraded(protocol, degradation);
                    }
                    if let Some(out_of_range) = out_of_range {
                        log::fee_scheduler_out_of_range(
                            protocol,
                            &account.pool_address,
                            out_of_range,
                        );
                        PoolAccountWorkerMetrics::record_fee_scheduler_out_of_range(protocol);
                    }
                }
                Err(e) => {
                    log::set_pool_account_failed(protocol, &account.pool_address, &e);
                    continue;
                }
            }

            match self
                .pool_repository
                .set_registry_properties(&account.pool_address, &decoded_account.registry)
                .await
            {
                Ok(()) => ok += 1,
                Err(e) => log::set_registry_properties_failed(protocol, &account.pool_address, &e),
            }
        }

        log::cycle_done(
            protocol,
            unresolved.len(),
            accounts.len(),
            decoded,
            rejected,
            ok,
            start.elapsed(),
        );
    }
}

#[cfg(test)]
#[path = "pool_account/pool_account_tests.rs"]
mod tests;
