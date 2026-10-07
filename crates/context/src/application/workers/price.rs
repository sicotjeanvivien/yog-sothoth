//! Price worker. Every `price_interval` (30 s by default), or back to back when
//! a tick outlasts it, it:
//!   1. lists the known mints and keeps those worth asking (`UnpricedMints`);
//!   2. asks the source for their USD price;
//!   3. drops the prices the column cannot hold and those that repeat the last
//!      row kept (`KeptPrices`), and inserts the rest in one statement.
//!
//! Errors are absorbed and logged: a failed tick is one missing sample, never a
//! stopped worker.

use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use solana_pubkey::Pubkey;
use tokio_util::sync::CancellationToken;

use yog_core::domain::{
    KeptPrices, TokenMetadataRepository, TokenPrice, TokenPriceRepository, UnpricedMints,
};

use crate::application::source::{FetchedPrice, PriceSource};
use crate::error::WorkerError;

mod log;
mod metrics;
mod tick_outcome;

pub(crate) use metrics::PriceWorkerMetrics;
use tick_outcome::TickOutcome;

/// Worker that records a USD price for the known mints worth asking, on a
/// fixed interval.
pub struct PriceWorker {
    metadata_repository: Arc<dyn TokenMetadataRepository>,
    price_repository: Arc<dyn TokenPriceRepository>,
    source: Arc<dyn PriceSource>,
    interval: std::time::Duration,
    /// The tail of the written series, per mint — what makes a tick able to
    /// tell a price that moved from one that merely came round again.
    kept: KeptPrices,
    /// The mints the source last answered without a price, and when each is
    /// worth asking again.
    unpriced: UnpricedMints,
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
            kept: KeptPrices::new(interval),
            unpriced: UnpricedMints::new(interval),
        }
    }

    /// Run the interval loop until the shutdown token is triggered. The first
    /// tick fires at once.
    pub async fn run(mut self, shutdown: CancellationToken) -> Result<(), WorkerError> {
        log::started(self.interval, &self.kept, &self.unpriced);

        let mut ticker = tokio::time::interval(self.interval);

        loop {
            tokio::select! {
                // ⚠️ `biased`: the stop must win a tie. A cycle that outruns the
                // cadence leaves `tick()` already ready, and an unbiased
                // `select!` would start a new cycle about one stop in two.
                biased;

                _ = shutdown.cancelled() => {
                    log::stopping();
                    return Ok(());
                }
                _ = ticker.tick() => {
                    self.run_one_cycle(&shutdown).await;
                }
            }
        }
    }

    /// One pricing cycle: time it, and let the ending say what it was — and
    /// whether the stop came before it ended.
    async fn run_one_cycle(&mut self, shutdown: &CancellationToken) {
        let start = Instant::now();

        let outcome = self.price_once(shutdown).await;
        outcome.record(start, shutdown.is_cancelled());
    }

    /// The cycle itself: it decides how it ended, and [`TickOutcome`] says so.
    ///
    /// ⚠️ A stop cuts the fetch short, not the cycle: what came back goes
    /// through the same filters and the same single insert.
    async fn price_once(&mut self, shutdown: &CancellationToken) -> TickOutcome {
        let known = match self.metadata_repository.list_known_mints().await {
            Ok(mints) => mints,
            Err(e) => return TickOutcome::ListFailed(e),
        };

        // Only the mints worth asking — see `UnpricedMints`.
        let asked_at = Utc::now();
        let mints: Vec<Pubkey> = known
            .iter()
            .filter(|mint| self.unpriced.is_due(mint, asked_at))
            .copied()
            .collect();
        PriceWorkerMetrics::set_known_mints(known.len());
        PriceWorkerMetrics::set_requested_mints(mints.len());

        if known.is_empty() {
            return TickOutcome::NoKnownMints;
        }
        if mints.is_empty() {
            return TickOutcome::NothingDue;
        }

        log::pricing(mints.len(), known.len());

        let answer = match self.source.fetch_prices(&mints, shutdown).await {
            Ok(answer) => answer,
            Err(e) => return TickOutcome::SourceFailed(e),
        };

        let now = Utc::now();

        // Before the filters: a price the column cannot hold is still a price.
        self.unpriced.record(
            answer.priced.iter().map(|price| &price.mint),
            &answer.unpriced,
            now,
        );
        let priced: Vec<TokenPrice> = answer
            .priced
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

        // ⚠️ Dropped here, not left to the database: the tick is ONE statement,
        // and a price the column refuses (`23514`, `22003`) would abort it for
        // every other mint, every tick. See `TokenPrice::is_storable`.
        let (mut to_insert, rejected): (Vec<TokenPrice>, Vec<TokenPrice>) =
            priced.into_iter().partition(TokenPrice::is_storable);

        if !rejected.is_empty() {
            PriceWorkerMetrics::record_rejected(rejected.len());
            log::unstorable(&rejected);
        }

        // ⚠️ Coverage sits between the two filters: an unstorable price values
        // nothing downstream, an unchanged one still does, through the row
        // already kept. Below the next filter, it would read as a collapse.
        PriceWorkerMetrics::set_priced_mints(to_insert.len());

        if to_insert.is_empty() {
            return TickOutcome::NoStorablePrice;
        }

        // A price that repeats the last row kept earns no row (`KeptPrices`).
        let before = to_insert.len();
        to_insert.retain(|price| self.kept.worth_keeping(price));
        let suppressed = before - to_insert.len();

        // Zero included, so that "nothing suppressed" is a measurement.
        PriceWorkerMetrics::record_unchanged(suppressed);

        if to_insert.is_empty() {
            return TickOutcome::AllUnchanged { suppressed };
        }

        let count = to_insert.len();
        if let Err(e) = self.price_repository.insert_batch(&to_insert).await {
            return TickOutcome::InsertFailed(e);
        }

        // ⚠️ After the insert only: see `KeptPrices::record`.
        self.kept.record(&to_insert);

        TickOutcome::Inserted { count }
    }
}

#[cfg(test)]
#[path = "price/price_tests.rs"]
mod tests;
