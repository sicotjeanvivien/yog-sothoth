//! Price worker: every `price_interval`, or back to back when a tick outlasts it, lists the
//! candidate mints, keeps those whose turn it is (`PriceCandidate`) and worth asking
//! (`UnpricedMints`), fetches their USD price, drops the unstorable and the repeated (`KeptPrices`)
//! and inserts the rest in one statement. A failed tick is one missing sample.

use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use solana_pubkey::Pubkey;
use tokio_util::sync::CancellationToken;

use yog_core::domain::{
    KeptPrices, PRICE_CANDIDATE_PARAMS, PriceCandidate, PriceCandidateClass,
    TokenMetadataRepository, TokenPrice, TokenPriceRepository, UnpricedMints,
};

use crate::application::source::{FetchedPrice, PriceSource};
use crate::error::WorkerError;

mod log;
mod metrics;
mod tick_outcome;

pub(crate) use metrics::PriceWorkerMetrics;
use tick_outcome::TickOutcome;

/// Worker that records a USD price for the candidate mints worth asking, on a fixed interval.
pub struct PriceWorker {
    metadata_repository: Arc<dyn TokenMetadataRepository>,
    price_repository: Arc<dyn TokenPriceRepository>,
    source: Arc<dyn PriceSource>,
    interval: std::time::Duration,
    /// The tail of the written series, per mint: what tells a moved price from a repeated one.
    kept: KeptPrices,
    /// The mints the source last answered without a price, and when each is worth asking again.
    unpriced: UnpricedMints,
    /// The number of the next tick, from 0 at boot: what [`PriceCandidate::is_turn`] reads.
    tick: u64,
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
            tick: 0,
        }
    }

    /// Run the interval loop until shutdown; the first tick fires at once.
    pub async fn run(mut self, shutdown: CancellationToken) -> Result<(), WorkerError> {
        log::started(self.interval, &self.kept, &self.unpriced);

        let mut ticker = tokio::time::interval(self.interval);

        loop {
            tokio::select! {
                // ⚠️ `biased`: a cycle that outruns the cadence leaves `tick()` ready, and an
                // unbiased pick would start a new cycle after a stop was asked for.
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

    /// One pricing cycle, timed; its ending says what it was and whether the stop came first.
    async fn run_one_cycle(&mut self, shutdown: &CancellationToken) {
        let start = Instant::now();

        let outcome = self.price_once(shutdown).await;
        outcome.record(start, shutdown.is_cancelled());
    }

    /// The cycle itself, its ending told by [`TickOutcome`].
    /// ⚠️ A stop cuts the fetch short, not the cycle: what came back goes through the same filters
    /// and the same single insert.
    async fn price_once(&mut self, shutdown: &CancellationToken) -> TickOutcome {
        let tick = self.tick;
        self.tick = tick.wrapping_add(1);

        let candidates = match self
            .metadata_repository
            .list_price_candidates(&PRICE_CANDIDATE_PARAMS)
            .await
        {
            Ok(candidates) => candidates,
            Err(e) => return TickOutcome::ListFailed(e),
        };
        let candidate_mints: Vec<Pubkey> = candidates.iter().map(|c| c.mint).collect();
        self.unpriced.retain_only(&candidate_mints);

        // Only the mints whose turn it is and worth asking — see `UnpricedMints`.
        let asked_at = Utc::now();
        let mints: Vec<Pubkey> = candidates
            .iter()
            .filter(|c| c.is_turn(tick) && self.unpriced.is_due(&c.mint, asked_at))
            .map(|c| c.mint)
            .collect();
        PriceWorkerMetrics::set_candidate_mints(candidates.len());
        record_candidates_by_class(&candidates);
        PriceWorkerMetrics::set_requested_mints(mints.len());

        if candidates.is_empty() {
            return TickOutcome::NoCandidates;
        }
        if mints.is_empty() {
            return TickOutcome::NothingDue;
        }

        log::pricing(mints.len(), candidates.len());

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

        // ⚠️ Dropped here: the tick is ONE statement, and a price the column refuses (`23514`,
        // `22003`) would abort it for every mint. See `TokenPrice::is_storable`.
        let (mut to_insert, rejected): (Vec<TokenPrice>, Vec<TokenPrice>) =
            priced.into_iter().partition(TokenPrice::is_storable);

        if !rejected.is_empty() {
            PriceWorkerMetrics::record_rejected(rejected.len());
            log::unstorable(&rejected);
        }

        // ⚠️ Coverage sits between the filters: an unstorable price values nothing, an unchanged one
        // still does through the row kept; below the next filter it would read as a collapse.
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

fn record_candidates_by_class(candidates: &[PriceCandidate]) {
    for (class, label) in [
        (PriceCandidateClass::Regular, "regular"),
        (PriceCandidateClass::LowValue, "low_value"),
        (PriceCandidateClass::NeverPriced, "never_priced"),
    ] {
        let count = candidates.iter().filter(|c| c.class == class).count();
        PriceWorkerMetrics::set_candidates_by_class(label, count);
    }
}

#[cfg(test)]
#[path = "price/price_tests.rs"]
mod tests;
