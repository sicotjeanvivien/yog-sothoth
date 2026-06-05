//! Metadata worker — discovers new mints and enriches them.
//!
//! Every `metadata_poll_interval` (10s by default):
//!   1. read the set of mints present in `pools` but absent from
//!      `token_metadata` (`list_missing_mints`);
//!   2. fetch their identity from Helius DAS in chunks of at most
//!      `DAS_BATCH_MAX`;
//!   3. upsert each successfully-fetched row.
//!
//! # Resilience
//!
//! `yog-context` is a comfort daemon: nothing it does is critical to
//! ingestion. A failure of a single tick must not bring down the
//! daemon, because a stopped enrichment daemon means the dashboard
//! gradually loses its token names — much worse than missing one
//! poll cycle.
//!
//! The loop therefore absorbs:
//!   - HTTP errors against Helius (`SourceError::Http`);
//!   - response-decoding errors (`SourceError::Decode`);
//!   - persistence errors when upserting an individual row.
//!
//! Each is logged and the tick continues (for upsert errors) or is
//! skipped (for source-level errors). The `run` return type stays
//! `Result<(), WorkerError>` so the worker still plugs into
//! `handle_task_result` — only a truly anomalous, non-recoverable
//! situation would return `Err`, but in practice this implementation
//! does not.

use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use yog_core::domain::{TokenMetadata, TokenMetadataRepository};

use super::metadata_metrics::MetadataWorkerMetrics;
use crate::error::WorkerError;
use crate::source::{FetchedMetadata, MetadataSource};

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
                _ = ticker.tick() => {
                    self.run_one_cycle().await;
                }
                _ = shutdown.cancelled() => {
                    info!("shutdown requested — metadata worker stopping");
                    return Ok(());
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

        // Single call — the source handles chunking and partial failures
        // internally.
        let fetched = match self.source.fetch_metadata(&missing).await {
            Ok(fetched) => fetched,
            Err(e) => {
                // Reserved for hard failures (misconfiguration etc.).
                // Best-effort partial results don't go through this path.
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

    /// Upsert every fetched metadata row, logging individual failures
    /// without aborting the rest of the batch.
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
#[path = "metadata_tests.rs"]
mod tests;
