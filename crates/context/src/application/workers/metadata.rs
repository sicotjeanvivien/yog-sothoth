//! Metadata worker: every poll, lists the mints in `pools` missing from `token_metadata`, fetches
//! their identity from Helius DAS and upserts each row.
//! A failed tick or row is logged and stepped over: a stopped enrichment would lose token names.

use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use yog_core::domain::{TokenMetadata, TokenMetadataRepository};

use crate::application::source::{FetchedMetadata, MetadataSource};
use crate::error::WorkerError;

mod metrics;

pub(crate) use metrics::MetadataWorkerMetrics;

pub struct MetadataWorker {
    repository: Arc<dyn TokenMetadataRepository>,
    source: Arc<dyn MetadataSource>,
    poll_interval: std::time::Duration,
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
        }
    }

    pub async fn run(self, shutdown: CancellationToken) -> Result<(), WorkerError> {
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

    async fn run_one_cycle(&self) {
        let start = Instant::now();
        let missing = match self.repository.list_missing_mints().await {
            Ok(missing) => missing,
            Err(e) => {
                warn!(error = %e, "metadata worker: list_missing_mints failed");
                MetadataWorkerMetrics::record_tick("list_failed", start.elapsed().as_secs_f64());
                return;
            }
        };
        MetadataWorkerMetrics::set_missing_mints(missing.len());
        if missing.is_empty() {
            debug!("metadata worker: no missing mints — sleeping");
            MetadataWorkerMetrics::record_tick("no_work", start.elapsed().as_secs_f64());

            return;
        }

        debug!(count = missing.len(), "metadata worker: enriching mints");

        // One call: the source chunks and absorbs partial failures.
        let fetched = match self.source.fetch_metadata(&missing).await {
            Ok(fetched) => fetched,
            Err(e) => {
                // Hard failures only (misconfiguration); partial results do not come here.
                warn!(error = %e, "metadata worker: source returned a hard error");
                MetadataWorkerMetrics::record_tick(
                    "source_hard_error",
                    start.elapsed().as_secs_f64(),
                );
                return;
            }
        };

        self.upsert_all(fetched).await;
        MetadataWorkerMetrics::record_tick("ok", start.elapsed().as_secs_f64());
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
