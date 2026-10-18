//! Metadata worker: every poll, lists the mints missing from `token_metadata` of the recent pools
//! (of every pool once an hour), fetches their identity from Helius DAS and upserts each row.
//! A failed tick or row is logged and stepped over: a stopped enrichment would lose token names.

use std::sync::Arc;

use chrono::Utc;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use yog_core::domain::{MISSING_MINT_DISCOVERY_WINDOW, TokenMetadata, TokenMetadataRepository};

use crate::application::source::{FetchedMetadata, MetadataSource};
use crate::error::WorkerError;

mod log;
mod metrics;
mod scope;
mod tick_outcome;

pub(crate) use metrics::MetadataWorkerMetrics;
use scope::Scope;
use tick_outcome::TickOutcome;

/// How long the worker searches only the recent pools before it sweeps all of them again.
const FULL_SWEEP_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3600);

pub struct MetadataWorker {
    repository: Arc<dyn TokenMetadataRepository>,
    source: Arc<dyn MetadataSource>,
    poll_interval: std::time::Duration,
    /// When the last listed sweep started; `None` sweeps at the first tick.
    last_full_sweep: Option<Instant>,
}

impl MetadataWorker {
    /// Build the worker.
    pub fn new(
        repository: Arc<dyn TokenMetadataRepository>,
        source: Arc<dyn MetadataSource>,
        poll_interval: std::time::Duration,
    ) -> Self {
        Self {
            repository,
            source,
            poll_interval,
            last_full_sweep: None,
        }
    }

    pub async fn run(mut self, shutdown: CancellationToken) -> Result<(), WorkerError> {
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
                _ = ticker.tick() => {
                    self.run_one_cycle().await;
                }
            }
        }
    }

    fn scope_due(&self) -> Scope {
        match self.last_full_sweep {
            Some(at) if at.elapsed() < FULL_SWEEP_INTERVAL => Scope::RecentPools,
            _ => Scope::AllPools,
        }
    }

    async fn run_one_cycle(&mut self) {
        let start = Instant::now();
        let scope = self.scope_due();
        self.enrich_once(scope, start).await.record(scope, start);
    }

    /// The tick itself, its ending told by [`TickOutcome`].
    async fn enrich_once(&mut self, scope: Scope, start: Instant) -> TickOutcome {
        let listed = match scope {
            Scope::RecentPools => {
                self.repository
                    .list_missing_mints_of_recent_pools(MISSING_MINT_DISCOVERY_WINDOW)
                    .await
            }
            Scope::AllPools => self.repository.list_missing_mints_of_all_pools().await,
        };
        let missing = match listed {
            Ok(missing) => missing,
            Err(e) => return TickOutcome::ListFailed(e),
        };
        if scope == Scope::AllPools {
            self.last_full_sweep = Some(start);
        }
        MetadataWorkerMetrics::set_missing_mints(scope.as_label(), missing.len());
        if missing.is_empty() {
            return TickOutcome::NoWork;
        }

        log::enriching(missing.len(), scope);

        // One call: the source chunks and absorbs partial failures.
        let fetched = match self.source.fetch_metadata(&missing).await {
            Ok(fetched) => fetched,
            // Hard failures only; partial results do not come here.
            Err(e) => return TickOutcome::SourceFailed(e),
        };

        self.upsert_all(fetched).await;
        TickOutcome::Enriched
    }

    /// Upsert every fetched row, logging individual failures without aborting the batch.
    async fn upsert_all(&self, fetched: Vec<FetchedMetadata>) {
        let now = Utc::now();

        for item in fetched {
            let metadata = TokenMetadata {
                mint: item.mint,
                symbol: item.symbol,
                name: item.name,
                decimals: item.decimals,
                logo_uri: item.logo_uri,
                metadata_provider: item.metadata_provider,
                fetched_at: now,
                last_refresh_at: now,
            };

            match self.repository.upsert(&metadata).await {
                Ok(()) => {
                    MetadataWorkerMetrics::record_upsert("ok");
                    log::upserted(&metadata.mint);
                }
                Err(e) => {
                    MetadataWorkerMetrics::record_upsert("error");
                    log::upsert_failed(&metadata.mint, &e);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "metadata/metadata_tests.rs"]
mod tests;
