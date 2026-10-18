//! Metadata worker: every poll, lists the mints missing from `token_metadata` of the recent pools
//! (of every pool once an hour), fetches their identity from Helius DAS and upserts each row.
//! A failed tick or row is logged and stepped over: a stopped enrichment would lose token names.

use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use yog_core::domain::{MISSING_MINT_DISCOVERY_WINDOW, TokenMetadata, TokenMetadataRepository};

use crate::application::source::{FetchedMetadata, MetadataSource};
use crate::error::WorkerError;

mod metrics;

pub(crate) use metrics::MetadataWorkerMetrics;

/// How long the worker searches only the recent pools before it sweeps all of them again.
const FULL_SWEEP_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3600);

/// Which pools a tick reads the mints of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    RecentPools,
    AllPools,
}

impl Scope {
    fn as_label(self) -> &'static str {
        match self {
            Self::RecentPools => "recent_pools",
            Self::AllPools => "all_pools",
        }
    }
}

pub struct MetadataWorker {
    repository: Arc<dyn TokenMetadataRepository>,
    source: Arc<dyn MetadataSource>,
    poll_interval: std::time::Duration,
    /// When the last sweep the source answered started; `None` sweeps at the first tick.
    last_full_sweep: Option<tokio::time::Instant>,
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
        info!("MetadataWorker started");

        let mut ticker = tokio::time::interval(self.poll_interval);

        loop {
            tokio::select! {
            // ⚠️ `biased`: with the default `Burst`, a cycle that outruns the cadence leaves `tick()`
            // ready, and an unbiased pick would start a new cycle after a stop was asked for.
            biased;

                _ = shutdown.cancelled() => {
                    info!("shutdown requested — metadata worker stopping");
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
        let started_at = tokio::time::Instant::now();
        let scope = self.scope_due();
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
            Err(e) => {
                warn!(
                    error = %e,
                    scope = scope.as_label(),
                    "metadata worker: listing missing mints failed",
                );
                MetadataWorkerMetrics::record_tick(
                    "list_failed",
                    scope.as_label(),
                    start.elapsed().as_secs_f64(),
                );
                return;
            }
        };
        MetadataWorkerMetrics::set_missing_mints(scope.as_label(), missing.len());
        if missing.is_empty() {
            self.answered(scope, started_at);
            debug!(
                scope = scope.as_label(),
                "metadata worker: no missing mints — sleeping"
            );
            MetadataWorkerMetrics::record_tick(
                "no_work",
                scope.as_label(),
                start.elapsed().as_secs_f64(),
            );

            return;
        }

        debug!(
            count = missing.len(),
            scope = scope.as_label(),
            "metadata worker: enriching mints"
        );

        // One call: the source chunks and absorbs partial failures.
        let fetched = match self.source.fetch_metadata(&missing).await {
            Ok(fetched) => fetched,
            Err(e) => {
                // Hard failures only; partial results do not come here.
                warn!(
                    error = %e,
                    scope = scope.as_label(),
                    "metadata worker: source returned a hard error",
                );
                MetadataWorkerMetrics::record_tick(
                    "source_hard_error",
                    scope.as_label(),
                    start.elapsed().as_secs_f64(),
                );
                return;
            }
        };

        self.answered(scope, started_at);
        self.upsert_all(fetched).await;
        MetadataWorkerMetrics::record_tick("ok", scope.as_label(), start.elapsed().as_secs_f64());
    }

    /// ⚠️ Only once the source answered: a sweep lost to a failure would leave the old pools'
    /// mints an hour more without metadata.
    fn answered(&mut self, scope: Scope, started_at: tokio::time::Instant) {
        if scope == Scope::AllPools {
            self.last_full_sweep = Some(started_at);
        }
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
                    debug!(mint = %metadata.mint, "metadata worker: upserted");
                }
                Err(e) => {
                    MetadataWorkerMetrics::record_upsert("error");
                    warn!(
                        mint = %metadata.mint,
                        error = %e,
                        "metadata worker: upsert failed",
                    );
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "metadata/metadata_tests.rs"]
mod tests;
