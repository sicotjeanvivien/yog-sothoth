//! Which price observations earn a row in the series.
//!
//! The price worker asks a source for every known mint on a fixed cadence, so
//! the series grows at the rate of the *worker* rather than at the rate of the
//! *prices*: measured on 22 September 2026, 70 % of the rows written in 24 h
//! repeated the previous row of the same mint, and 82 % over a busier window.
//!
//! Deciding that an observation says nothing new is a product judgement about
//! freshness — the same kind of judgement as [`FreshnessStatus`], and for the
//! same reason it lives here rather than in `persistence` or in the worker.
//!
//! [`FreshnessStatus`]: crate::domain::FreshnessStatus

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use rust_decimal::{Decimal, RoundingStrategy};
use solana_pubkey::Pubkey;

use crate::domain::{PRICE_STORAGE_SCALE, PriceProvider, TokenPrice};

/// How old the most recent observation may be and still count as a current
/// price — the 15 minutes of `yog_price_max_age_latest()`.
///
/// **A mirror of migration 005, not a preference.** The SQL function is the
/// source of truth; every `pool_current_tvl` and latest-price read is bounded
/// by it, and a price older than this stops valuing anything. It is restated
/// here because `PRICE_SERIES_MAX_GAP` is only correct *relative to it*, and
/// a floor whose reason lives in another language is a floor nobody can check.
///
/// Migrations are forward-only, so this value can only change by a new one
/// redefining the function — and that migration has to revisit this constant.
/// Nothing enforces that today.
pub const PRICE_MAX_AGE_LATEST: Duration = Duration::minutes(15);

/// The widest gap allowed between two kept observations of the same mint.
///
/// Chosen **below** [`PRICE_MAX_AGE_LATEST`]. Without this floor, a mint whose
/// price never moves would keep one row for ever: three hours later its last
/// observation is three hours old, outside that bound *and* outside the
/// one-hour as-of window of `yog_price_max_age_asof()`, and every USD figure
/// derived from it turns NULL. Suppressing a repeated row must never suppress
/// the price itself.
///
/// ⚠️ **A forced row does not land at the floor, it lands at the first *tick*
/// on or after it.** Taken literally the floor would therefore promise
/// `ceil(gap / interval) × interval`, which is *not* monotonic in the cadence —
/// 600 s divides ten minutes and spaces rows by ten, 480 s does not and spaces
/// them by sixteen, past the staleness bound, at a cadence nobody would call
/// dangerous. [`KeptPrices::new`] removes that dependency instead of bounding
/// it: it subtracts one tick, so the row lands at or before this constant
/// whatever the cadence.
const PRICE_SERIES_MAX_GAP: Duration = Duration::minutes(10);

/// The last observation kept for each mint — the tail of the written series.
///
/// Held by the price worker, which is the only writer of `token_prices`, so
/// this is the whole truth about what the table's latest row holds. It is not
/// a cache of the database: nothing else produces the information, and reading
/// it back would be asking Postgres what we just told it.
///
/// It is bounded by `token_metadata`, which the worker already loads in full
/// on every tick (`list_known_mints`), so it needs no eviction: it cannot
/// outgrow something the process holds anyway. It starts empty on every boot,
/// which costs exactly one full batch at the first tick — a row too many,
/// never one too few.
#[derive(Debug)]
pub struct KeptPrices {
    last: HashMap<Pubkey, KeptPrice>,
    max_gap: Duration,
}

/// One kept observation, reduced to what the decision reads.
#[derive(Debug, Clone)]
struct KeptPrice {
    /// The price **as the column holds it** — see [`KeptPrices::worth_keeping`].
    price_usd: Decimal,
    price_provider: PriceProvider,
    fetched_at: DateTime<Utc>,
}

impl KeptPrices {
    /// Build the rule for a worker ticking every `tick_interval`.
    ///
    /// **The cadence is not optional, and that is the point.** The floor is
    /// decided one tick early — `PRICE_SERIES_MAX_GAP - tick_interval` — so the
    /// forced row lands at or before `PRICE_SERIES_MAX_GAP` rather than at the
    /// first tick past it. Without that subtraction the real spacing is
    /// `ceil(gap / interval) × interval`, a quantity that jumps over the
    /// staleness bound at cadences no one would flag: 480 s spaces rows by 16
    /// minutes while both 300 s and 600 s stay at 10. There is then no ceiling
    /// to enforce and no ragged set of safe values to document — the worst
    /// spacing becomes `max(tick_interval, PRICE_SERIES_MAX_GAP)`, which is to
    /// say that the cadence bounds freshness exactly as it did before any of
    /// this existed.
    ///
    /// A cadence at or past the floor leaves nothing to subtract, so every tick
    /// writes and the rule goes inert — the safe direction.
    ///
    /// ⚠️ It is the *configured* cadence, not the observed one. A cycle that
    /// persistently overruns its period (this worker's took 10.7–19.9 s on
    /// 14 September 2026, and 85 s against 5 028 mints on 22 September) widens
    /// the spacing by the overrun. The five minutes between
    /// `PRICE_SERIES_MAX_GAP` and [`PRICE_MAX_AGE_LATEST`] are what absorb it:
    /// a 200 s cycle against a 30 s cadence still lands at 600 s.
    pub fn new(tick_interval: core::time::Duration) -> Self {
        let tick = Duration::from_std(tick_interval).unwrap_or(PRICE_SERIES_MAX_GAP);

        Self {
            last: HashMap::new(),
            max_gap: (PRICE_SERIES_MAX_GAP - tick).max(Duration::zero()),
        }
    }

    /// Whether this observation earns a row.
    ///
    /// True when the mint has never been priced, when the price moved, when
    /// the provenance changed, or when the last kept row has reached the floor
    /// [`KeptPrices::new`] computed for this cadence.
    ///
    /// # Why the comparison rounds first
    ///
    /// `token_prices.price_usd` is `NUMERIC(38, 18)`, so **Postgres rounds
    /// every value on write**, and a source that answers with more decimals
    /// than that never sees its own number again. Jupiter is such a source:
    /// measured on 22 September 2026 over the 30 995 rows of the previous
    /// 24 h, **24 824 — 80 % — carry exactly 18 decimals**, the column's own
    /// scale. Comparing the incoming `Decimal` against the previous incoming
    /// `Decimal` would therefore almost never find equality, and this whole
    /// rule would suppress nothing at all.
    ///
    /// So both sides are rounded at [`PRICE_STORAGE_SCALE`] with
    /// `MidpointAwayFromZero` — Postgres's own `NUMERIC` rounding, for the
    /// same reason and with the same two constants as
    /// [`TokenPrice::is_storable`]. [`KeptPrices::record`] stores the rounded
    /// value, so what is remembered is what the table holds.
    ///
    /// # Why the provenance takes part
    ///
    /// A price that comes from a different source is a different observation
    /// even at the same number: `price_provider` is a column of the series,
    /// and a reader asking when we fell back would otherwise see the switch
    /// only at the next move of the market. Only [`PriceProvider::Jupiter`] has
    /// an implementation today; the other two variants are why this is written
    /// down rather than left to the future writer to notice.
    ///
    /// # Why `confidence` does not
    ///
    /// The fourth column `insert_batch` binds is left out on purpose. It is a
    /// source-reported precision hint about the price, not a second
    /// observation, and it is an `f32`: a provider whose confidence wobbles in
    /// its last digits would make every tick look like news and this rule
    /// inert — the same failure the rounding above exists to prevent, arriving
    /// through another column. Today the worker hardcodes `None`, so nothing
    /// is lost. A source that does report one (Helius DAS carries a
    /// `price_info` confidence) must decide what a *material* change of
    /// confidence is before adding it here; equality is not that decision.
    pub fn worth_keeping(&self, candidate: &TokenPrice) -> bool {
        let Some(last) = self.last.get(&candidate.mint) else {
            return true;
        };

        last.price_usd != at_storage_scale(candidate.price_usd)
            || last.price_provider != candidate.price_provider
            || candidate.fetched_at - last.fetched_at >= self.max_gap
    }

    /// Remember observations that have been written.
    ///
    /// The caller records **after** a successful insert, never before: a batch
    /// that failed left no row, and remembering it would hold the real price
    /// back until the next floor — a gap of up to 10 minutes in a series
    /// nothing backfills.
    pub fn record(&mut self, kept: &[TokenPrice]) {
        for price in kept {
            self.last.insert(
                price.mint,
                KeptPrice {
                    price_usd: at_storage_scale(price.price_usd),
                    price_provider: price.price_provider,
                    fetched_at: price.fetched_at,
                },
            );
        }
    }
}

/// A price as `token_prices.price_usd` holds it.
fn at_storage_scale(price: Decimal) -> Decimal {
    price.round_dp_with_strategy(PRICE_STORAGE_SCALE, RoundingStrategy::MidpointAwayFromZero)
}

#[cfg(test)]
#[path = "kept_prices_tests.rs"]
mod tests;
