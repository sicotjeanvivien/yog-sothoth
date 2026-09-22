//! Price worker — periodically prices every known mint.
//!
//! Every `price_interval` (30s by default):
//!   1. read the set of mints we have metadata for
//!      (`list_known_mints`);
//!   2. ask Jupiter for their USD price in chunks of at most
//!      `JUPITER_BATCH_MAX`;
//!   3. assemble `TokenPrice` rows, drop the ones the price column
//!      cannot hold and the ones that repeat the last row kept for
//!      their mint, and `insert_batch` the rest in a single
//!      round-trip.
//!
//! Most ticks write nothing: four prices in five come back identical
//! to the last one stored, and `KeptPrices` is what says so. The
//! series then grows at the rate of the prices rather than at the
//! rate of this loop.
//!
//! # Resilience
//!
//! Same policy as the metadata worker: HTTP/decoding errors against
//! Jupiter, and persistence errors on the batch insert, are absorbed
//! in the loop and logged. A failed tick simply means one missing
//! 30-second sample — invisible at the dashboard level. The daemon
//! must not fall over on a Jupiter hiccup.

use super::price_metrics::PriceWorkerMetrics;
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use yog_core::domain::{KeptPrices, TokenMetadataRepository, TokenPrice, TokenPriceRepository};

use crate::error::WorkerError;
use crate::source::{FetchedPrice, PriceSource};

/// Worker that records a USD price for every known mint on a fixed
/// interval.
pub struct PriceWorker {
    metadata_repository: Arc<dyn TokenMetadataRepository>,
    price_repository: Arc<dyn TokenPriceRepository>,
    source: Arc<dyn PriceSource>,
    interval: std::time::Duration,
    /// The tail of the written series, per mint — what makes a tick able to
    /// tell a price that moved from one that merely came round again.
    kept: KeptPrices,
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
            kept: KeptPrices::default(),
        }
    }

    /// Run the interval loop until the shutdown token is triggered.
    ///
    /// The first tick fires immediately (tokio's `interval` yields
    /// at once), so a fresh price sample lands as soon as the daemon
    /// starts rather than after the first interval.
    pub async fn run(mut self, shutdown: CancellationToken) -> Result<(), WorkerError> {
        info!("PriceWorker started");

        let mut ticker = tokio::time::interval(self.interval);

        loop {
            tokio::select! {
                // ⚠️ **`biased`: the stop must win a tie, and here it is the
                // tie that is measured.** The ticker keeps tokio's default
                // `MissedTickBehavior::Burst`, so a cycle that outruns the
                // cadence leaves `tick()` **already ready** when the loop comes
                // back round — and this worker's cycle took 10.7–19.9 s against
                // a rate-limiting Jupiter on 14 September 2026, for a 30 s
                // cadence. An unbiased `select!` would then pick pseudo-randomly
                // between a new tick and a stop already asked for: about one
                // stop in two starting a fresh 900-mint fetch, burning the
                // shutdown grace and dying mid-`INSERT`. Same reason the
                // indexer's own loops are biased.
                biased;

                _ = shutdown.cancelled() => {
                    info!("shutdown requested — price worker stopping");
                    return Ok(());
                }
                _ = ticker.tick() => {
                    self.run_one_cycle().await;
                }
            }
        }
    }

    /// One pricing cycle. Absorbs every recoverable error so a hiccup
    /// never stops the worker.
    async fn run_one_cycle(&mut self) {
        let start = Instant::now();
        let mints = match self.metadata_repository.list_known_mints().await {
            Ok(mints) => mints,
            Err(e) => {
                warn!(error = %e, "price worker: list_known_mints failed");
                PriceWorkerMetrics::record_tick("list_failed", start.elapsed().as_secs_f64());

                return;
            }
        };
        PriceWorkerMetrics::set_known_mints(mints.len());

        if mints.is_empty() {
            debug!("price worker: no known mints yet — sleeping");
            // Both gauges move together or the ratio the README tells you to
            // alert on (`priced / known`) divides a stale numerator by 0 and
            // reads +Inf on a cold start.
            PriceWorkerMetrics::set_priced_mints(0);
            PriceWorkerMetrics::record_tick("no_work", start.elapsed().as_secs_f64());
            return;
        }

        debug!(count = mints.len(), "price worker: pricing mints");

        let fetched = match self.source.fetch_prices(&mints).await {
            Ok(fetched) => fetched,
            Err(e) => {
                warn!(error = %e, "price worker: source returned a hard error");
                // Third early return, same rule as the other two: the numerator
                // must never outlive the denominator it was measured against.
                // Unreachable with the current Jupiter client (it absorbs
                // per-chunk failures and returns Ok(partial)), which is exactly
                // why it would rot silently in the next PriceSource.
                PriceWorkerMetrics::set_priced_mints(0);
                PriceWorkerMetrics::record_tick("source_hard_error", start.elapsed().as_secs_f64());
                return;
            }
        };

        let now = Utc::now();
        let priced: Vec<TokenPrice> = fetched
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

        // A price the `NUMERIC(38, 18)` column cannot hold is dropped HERE
        // rather than left for the database to refuse.
        //
        // Not a nicety: `insert_batch` sends the whole tick as ONE statement,
        // and `ON CONFLICT DO NOTHING` covers neither the `CHECK` of migration
        // 009 (`23514`, a price that rounds to zero) nor the column type's own
        // overflow (`22003`, a price at or above 10^20). Either one aborts the
        // insert for *every other mint*, every tick, for as long as that mint
        // stays in the known set — and migration 005 established that the
        // resulting as-of gap never heals. The constraint is the guarantee;
        // this filter is what keeps it from ever firing. See
        // `TokenPrice::is_storable` for why the test is neither `> 0` nor
        // one-sided.
        let (mut to_insert, rejected): (Vec<TokenPrice>, Vec<TokenPrice>) =
            priced.into_iter().partition(TokenPrice::is_storable);

        if !rejected.is_empty() {
            PriceWorkerMetrics::record_rejected(rejected.len());
            warn!(
                count = rejected.len(),
                mints = ?rejected.iter().map(|p| p.mint.to_string()).collect::<Vec<_>>(),
                "price worker: dropped prices the price column cannot hold"
            );
        }

        // Coverage of this tick: how many of the mints we asked for yielded a
        // price we actually kept. Counted after the storability filter, because
        // the gauge answers "what can be valued downstream" — a price rejected
        // there is as absent, downstream, as one the source never returned. Set
        // before the empty check so a tick that keeps nothing reports 0 rather
        // than leaving the gauge on its last value.
        //
        // ⚠️ And counted BEFORE the redundancy filter below, for the same
        // question: a price suppressed as unchanged is still valued downstream,
        // by the row that already says it. Moving this line down would read as
        // a coverage collapse to under a fifth — `priced / known` is the ratio
        // the README tells you to alert on, and it would be lit for ever.
        PriceWorkerMetrics::set_priced_mints(to_insert.len());

        if to_insert.is_empty() {
            debug!("price worker: no prices to insert");
            // `no_prices` was declared in the outcome label set from the start
            // but never emitted — a tick that priced nothing looked, in the
            // metrics, exactly like a tick that never happened.
            PriceWorkerMetrics::record_tick("no_prices", start.elapsed().as_secs_f64());
            return;
        }

        // Second filter, and the one that decides the SIZE of the series: a
        // price that repeats the last row kept for its mint earns no row of its
        // own. `KeptPrices` carries the rule — including the floor that writes
        // a motionless price anyway before it ages out of migration 005's
        // windows, and the rounding to the column's scale without which the
        // comparison would never find two prices equal.
        //
        // `retain` rather than a second `partition`: unlike `rejected` above,
        // which is logged mint by mint, nothing here reads the suppressed rows
        // — only how many there were. Partitioning would move four fifths of
        // the tick into a Vec built to be dropped.
        let before = to_insert.len();
        to_insert.retain(|price| self.kept.worth_keeping(price));
        let unchanged = before - to_insert.len();

        // Recorded on every tick that reaches the rule, zero included, so that
        // "nothing was suppressed" is a measurement. Its ratio to
        // `inserted_total` is the redundancy the database query used to be
        // needed for.
        PriceWorkerMetrics::record_unchanged(unchanged);

        if to_insert.is_empty() {
            debug!(
                count = unchanged,
                "price worker: every price repeats the last one kept"
            );
            // NOT `no_prices`: that outcome means the source valued nothing,
            // which is an anomaly worth alerting on. Once redundant rows are
            // suppressed, most ticks write nothing at all — labelling them the
            // same way would leave that alert permanently lit.
            PriceWorkerMetrics::record_tick("unchanged", start.elapsed().as_secs_f64());
            return;
        }

        let inserted = to_insert.len();
        if let Err(e) = self.price_repository.insert_batch(&to_insert).await {
            warn!(error = %e, "price worker: insert_batch failed");
            PriceWorkerMetrics::record_tick("insert_failed", start.elapsed().as_secs_f64());
            return;
        }

        // Only now, and never before the insert: a batch that failed left no
        // row, and remembering it would hold the real price back until the
        // floor fires — a gap in a series nothing backfills.
        self.kept.record(&to_insert);

        PriceWorkerMetrics::record_inserted(inserted);
        debug!(count = inserted, "price worker: prices inserted");
        PriceWorkerMetrics::record_tick("ok", start.elapsed().as_secs_f64());
    }
}

#[cfg(test)]
#[path = "price_tests.rs"]
mod tests;
