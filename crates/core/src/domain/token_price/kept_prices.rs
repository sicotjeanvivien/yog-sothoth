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

/// The widest gap allowed between two kept observations of the same mint.
///
/// Chosen **below** the 15 minutes of `yog_price_max_age_latest()` (migration
/// 005), which is how old the most recent observation may be and still count
/// as a current price. Without this floor, a mint whose price never moves
/// would keep one row for ever: three hours later its last observation is
/// three hours old, outside that bound *and* outside the one-hour as-of window
/// of `yog_price_max_age_asof()`, and every USD figure derived from it turns
/// NULL. Suppressing a repeated row must never suppress the price itself.
///
/// The margin is deliberate. The worker ticks every 30 s
/// (`CONTEXT_PRICE_INTERVAL_SECS`), so a forced row lands at worst 10 min 30 s
/// after the previous one — 4 min 30 s of slack under the 15-minute bound, and
/// six rows an hour where the as-of window asks for one.
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
#[derive(Debug, Default)]
pub struct KeptPrices {
    last: HashMap<Pubkey, KeptPrice>,
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
    /// Whether this observation earns a row.
    ///
    /// True when the mint has never been priced, when the price moved, when
    /// the provenance changed, or when the last kept row has reached
    /// `PRICE_SERIES_MAX_GAP`.
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
    pub fn worth_keeping(&self, candidate: &TokenPrice) -> bool {
        let Some(last) = self.last.get(&candidate.mint) else {
            return true;
        };

        last.price_usd != at_storage_scale(candidate.price_usd)
            || last.price_provider != candidate.price_provider
            || candidate.fetched_at - last.fetched_at >= PRICE_SERIES_MAX_GAP
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
