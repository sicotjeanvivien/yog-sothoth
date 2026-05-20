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

use chrono::Utc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use yog_core::domain::{TokenMetadata, TokenMetadataRepository};

use crate::error::WorkerError;
use crate::source::{DAS_BATCH_MAX, FetchedMetadata, HeliusDasClient};

/// Tag stored in `token_metadata.metadata_source` for rows produced
/// by this worker.
const METADATA_SOURCE_TAG: &str = "helius_das";

/// Worker that keeps `token_metadata` in sync with the mints seen in
/// `pools`.
pub struct MetadataWorker {
    repository: Arc<dyn TokenMetadataRepository>,
    helius: HeliusDasClient,
    poll_interval: std::time::Duration,
}

impl MetadataWorker {
    /// Build the worker.
    pub fn new(
        repository: Arc<dyn TokenMetadataRepository>,
        helius: HeliusDasClient,
        poll_interval: std::time::Duration,
    ) -> Self {
        Self {
            repository,
            helius,
            poll_interval,
        }
    }

    /// Run the poll/fetch/upsert loop until the shutdown token is
    /// triggered.
    ///
    /// The first tick fires immediately (tokio's `interval` yields at
    /// once), so the first batch of missing mints is enriched as soon
    /// as the daemon starts.
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

    /// One poll cycle. Absorbs every recoverable error so a hiccup
    /// never stops the worker.
    async fn run_one_cycle(&self) {
        let missing = match self.repository.list_missing_mints().await {
            Ok(missing) => missing,
            Err(e) => {
                // A DB read failure here is unusual (the API is
                // already reading the same DB) — log and retry next
                // tick.
                warn!(error = %e, "metadata worker: list_missing_mints failed");
                return;
            }
        };

        if missing.is_empty() {
            debug!("metadata worker: no missing mints — sleeping");
            return;
        }

        debug!(count = missing.len(), "metadata worker: enriching mints");

        // Chunk to respect the DAS batch limit. At the v0.1 scale the
        // queue fits in one chunk, but the loop is here for the day
        // the allowlist is lifted.
        for chunk in missing.chunks(DAS_BATCH_MAX) {
            let fetched = match self.helius.fetch_asset_batch(chunk).await {
                Ok(fetched) => fetched,
                Err(e) => {
                    // Helius failure: log and move on. The unfetched
                    // mints stay in the "missing" set and will be
                    // retried on the next tick.
                    warn!(error = %e, "metadata worker: DAS fetch failed");
                    continue;
                }
            };

            self.upsert_all(fetched).await;
        }
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
                metadata_source: METADATA_SOURCE_TAG.to_string(),
                fetched_at: now,
                last_refresh_at: now,
            };

            if let Err(e) = self.repository.upsert(&metadata).await {
                warn!(
                    mint = %metadata.mint,
                    error = %e,
                    "metadata worker: upsert failed",
                );
                continue;
            }

            debug!(mint = %metadata.mint, "metadata worker: upserted");
        }
    }
}
