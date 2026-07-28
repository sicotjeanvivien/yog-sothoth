//! Pool-account resolver worker — fills account-derived pool properties.
//!
//! The indexer records a pool by address with `NULL` mints (it can't infer them
//! reliably from the transaction) and a `NULL` `fee_bps` for any pool whose
//! genesis event it never saw. Every `poll_interval`, and **for each protocol**,
//! this worker:
//!   1. asks that protocol's resolver which of its pools are unresolved;
//!   2. fetches their raw accounts through the shared [`PoolAccountSource`];
//!   3. decodes each account in `core`, routed on its owner;
//!   4. writes the decoded properties back through the resolver.
//!
//! # Protocol-agnostic
//!
//! The worker names no protocol. It holds `Vec<Arc<dyn PoolAccountResolver>>`
//! and one shared account source, and the decoding routes itself on the
//! account's owner — the same shape as the indexer's `EventPersistor` over
//! `DomainEvent`. Adding a protocol means adding a resolver to the list; not a
//! line here changes.
//!
//! # Resilience
//!
//! Like the other yog-context workers, a single failed tick must not bring the
//! daemon down: source and per-pool persistence errors are logged and the loop
//! continues. A failure for one protocol does not skip the others. It must run
//! before metadata/price enrichment can do anything — those key off the resolved
//! mints.

use std::sync::Arc;
use std::time::Instant;

use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use yog_core::application::decode_pool_account;
use yog_core::domain::PoolAccountResolver;

use crate::error::WorkerError;
use crate::source::PoolAccountSource;

/// Max pools resolved per tick. Matches `getMultipleAccounts`' 100-key cap,
/// so a tick is a single RPC round-trip.
const RESOLVE_BATCH_MAX: i64 = 100;

pub struct PoolAccountWorker {
    /// One per protocol. Each owns its own queue and its own tables; the worker
    /// only iterates.
    resolvers: Vec<Arc<dyn PoolAccountResolver>>,
    source: Arc<dyn PoolAccountSource>,
    poll_interval: std::time::Duration,
}

impl PoolAccountWorker {
    pub fn new(
        resolvers: Vec<Arc<dyn PoolAccountResolver>>,
        source: Arc<dyn PoolAccountSource>,
        poll_interval: std::time::Duration,
    ) -> Self {
        Self {
            resolvers,
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

    /// One pass over every protocol. A failure for one is logged and the loop
    /// moves to the next — a broken resolver must not stop the others.
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
            // Decoding routes on the account's program id, so a pool that
            // changed hands — or a resolver whose queue over-reaches — is
            // rejected here rather than decoded at the wrong layout.
            //
            // Every rejection is logged. In this path none of them is routine:
            // the accounts belong to pools this very queue asked for, so a
            // rejection always means something is off — a stale row, a missing
            // decoder, or the program having changed its layout.
            let properties = match decode_pool_account(&account.program_id, &account.data) {
                Ok(properties) => properties,
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

            if properties.protocol() != protocol {
                warn!(
                    protocol = %protocol,
                    decoded_as = %properties.protocol(),
                    pool = %account.pool_address,
                    "pool-account worker: queue and account disagree on protocol — skipping",
                );
                continue;
            }

            match resolver
                .set_pool_account(&account.pool_address, &properties)
                .await
            {
                Ok(()) => ok += 1,
                Err(e) => {
                    warn!(protocol = %protocol, pool = %account.pool_address, error = %e,
                          "pool-account worker: set_pool_account failed")
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
#[path = "pool_account_tests.rs"]
mod tests;
