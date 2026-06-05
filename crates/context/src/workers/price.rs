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

use yog_core::domain::{TokenMetadataRepository, TokenPrice, TokenPriceRepository};

use crate::error::WorkerError;
use crate::source::{FetchedPrice, PriceSource};

/// Worker that records a USD price for every known mint on a fixed
/// interval.
pub struct PriceWorker {
    metadata_repository: Arc<dyn TokenMetadataRepository>,
    price_repository: Arc<dyn TokenPriceRepository>,
    source: Arc<dyn PriceSource>,
    interval: std::time::Duration,
}

impl PriceWorker {
    pub fn new(
        metadata_repository: Arc<dyn TokenMetadataRepository>,
        price_repository: Arc<dyn TokenPriceRepository>,
        source: Arc<dyn PriceSource>,
        interval: std::time::Duration,
    ) -> Self {
        Self {
            metadata_repository,
            price_repository,
            source,
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

        let fetched = match self.source.fetch_prices(&mints).await {
            Ok(fetched) => fetched,
            Err(e) => {
                warn!(error = %e, "price worker: source returned a hard error");
                return;
            }
        };

        if fetched.is_empty() {
            debug!("price worker: no prices to insert");
            return;
        }

        let now = Utc::now();
        let to_insert: Vec<TokenPrice> = fetched
            .into_iter()
            .map(
                |FetchedPrice {
                     mint,
                     price_provider,
                     price_usd,
                 }| TokenPrice {
                    mint,
                    price_usd,
                    price_provider,
                    confidence: None,
                    fetched_at: now,
                },
            )
            .collect();

        let inserted = to_insert.len();
        if let Err(e) = self.price_repository.insert_batch(&to_insert).await {
            warn!(error = %e, "price worker: insert_batch failed");
            return;
        }

        debug!(count = inserted, "price worker: prices inserted");
    }
}

#[cfg(test)]
#[path = "price_tests.rs"]
mod tests;
