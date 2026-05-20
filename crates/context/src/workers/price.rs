//! Price worker — periodically prices every known mint.
//!
//! Every `price_interval` (30s by default):
//!   1. read the set of mints we have metadata for
//!      (`list_known_mints`);
//!   2. ask Jupiter for their USD price in chunks of at most
//!      `JUPITER_BATCH_MAX`;
//!   3. assemble `TokenPrice` rows and `insert_batch` them in a
//!      single round-trip.
//!
//! # Resilience
//!
//! Same policy as the metadata worker: HTTP/decoding errors against
//! Jupiter, and persistence errors on the batch insert, are absorbed
//! in the loop and logged. A failed tick simply means one missing
//! 30-second sample — invisible at the dashboard level. The daemon
//! must not fall over on a Jupiter hiccup.

use std::sync::Arc;

use chrono::Utc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use yog_core::domain::{PriceSource, TokenMetadataRepository, TokenPrice, TokenPriceRepository};

use crate::error::WorkerError;
use crate::source::{FetchedPrice, JUPITER_BATCH_MAX, JupiterPriceClient};

/// Worker that records a USD price for every known mint on a fixed
/// interval.
pub struct PriceWorker {
    metadata_repository: Arc<dyn TokenMetadataRepository>,
    price_repository: Arc<dyn TokenPriceRepository>,
    jupiter: JupiterPriceClient,
    interval: std::time::Duration,
}

impl PriceWorker {
    /// Build the worker.
    pub fn new(
        metadata_repository: Arc<dyn TokenMetadataRepository>,
        price_repository: Arc<dyn TokenPriceRepository>,
        jupiter: JupiterPriceClient,
        interval: std::time::Duration,
    ) -> Self {
        Self {
            metadata_repository,
            price_repository,
            jupiter,
            interval,
        }
    }

    /// Run the interval loop until the shutdown token is triggered.
    ///
    /// The first tick fires immediately (tokio's `interval` yields
    /// at once), so a fresh price sample lands as soon as the daemon
    /// starts rather than after the first interval.
    pub async fn run(self, shutdown: CancellationToken) -> Result<(), WorkerError> {
        info!("PriceWorker started");

        let mut ticker = tokio::time::interval(self.interval);

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    self.run_one_cycle().await;
                }
                _ = shutdown.cancelled() => {
                    info!("shutdown requested — price worker stopping");
                    return Ok(());
                }
            }
        }
    }

    /// One pricing cycle. Absorbs every recoverable error so a hiccup
    /// never stops the worker.
    async fn run_one_cycle(&self) {
        let mints = match self.metadata_repository.list_known_mints().await {
            Ok(mints) => mints,
            Err(e) => {
                warn!(error = %e, "price worker: list_known_mints failed");
                return;
            }
        };

        if mints.is_empty() {
            debug!("price worker: no known mints yet — sleeping");
            return;
        }

        debug!(count = mints.len(), "price worker: pricing mints");

        // Collect fetched prices across all chunks, then insert the
        // whole batch in one DB round-trip at the end.
        let now = Utc::now();
        let mut to_insert: Vec<TokenPrice> = Vec::with_capacity(mints.len());

        for chunk in mints.chunks(JUPITER_BATCH_MAX) {
            let fetched = match self.jupiter.fetch_prices_batch(chunk).await {
                Ok(fetched) => fetched,
                Err(e) => {
                    // Jupiter hiccup: log, drop this chunk, try the
                    // next one. The unfetched mints will be retried
                    // on the next tick.
                    warn!(error = %e, "price worker: Jupiter fetch failed");
                    continue;
                }
            };

            for FetchedPrice { mint, price_usd } in fetched {
                to_insert.push(TokenPrice {
                    mint,
                    price_usd,
                    price_source: PriceSource::Jupiter,
                    // Jupiter V3 does not expose a confidence value.
                    confidence: None,
                    fetched_at: now,
                });
            }
        }

        if to_insert.is_empty() {
            // Every chunk failed, or Jupiter priced none of the
            // known mints (very fresh launches, no recent trades).
            // Either way, nothing to write this tick.
            debug!("price worker: no prices to insert");
            return;
        }

        let inserted = to_insert.len();
        if let Err(e) = self.price_repository.insert_batch(&to_insert).await {
            warn!(error = %e, "price worker: insert_batch failed");
            return;
        }

        debug!(count = inserted, "price worker: prices inserted");
    }
}
