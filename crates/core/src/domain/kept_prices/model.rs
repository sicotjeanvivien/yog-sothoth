//! What the price series has already recorded, and what a new observation has
//! to say to earn a row of its own.
//!
//! The price worker asks for every price on a fixed cadence, so without this
//! rule the series grows at the rate of the worker rather than at the rate of
//! the prices. Deciding that an observation says nothing new is a product
//! judgement about freshness, like [`FreshnessStatus`], hence its place here.
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
/// ⚠️ **A mirror of migration 005, not a preference.** The SQL function is the
/// source of truth; this constant is restated because `PRICE_SERIES_MAX_GAP`
/// is only correct relative to it. A migration that redefines the function
/// must revisit it — nothing enforces that.
pub const PRICE_MAX_AGE_LATEST: Duration = Duration::minutes(15);

/// The widest gap allowed between two kept observations of the same mint.
///
/// Below [`PRICE_MAX_AGE_LATEST`], so that a price that never moves is still
/// rewritten before it ages out of the staleness windows of migration 005 and
/// every USD figure derived from it turns NULL. Suppressing a repeated row
/// must never suppress the price itself.
///
/// ⚠️ **A forced row lands at the first tick on or after the floor, not at the
/// floor.** Taken literally, the spacing would be `ceil(gap / cadence) ×
/// cadence`, which is not monotonic in the cadence. [`KeptPrices::new`]
/// subtracts one tick instead, so the row lands at or before this constant.
const PRICE_SERIES_MAX_GAP: Duration = Duration::minutes(10);

/// The last observation kept for each mint — the tail of the written series.
///
/// Held by the price worker, the only writer of `token_prices`: it is the whole
/// truth about the table's latest row, not a cache of it. It needs no eviction,
/// being bounded by `token_metadata`, which only grows as tokens are
/// discovered.
///
/// Starts empty on every boot, which costs one full batch at the first tick —
/// a row too many, never one too few.
#[derive(Debug)]
pub struct KeptPrices {
    last: HashMap<Pubkey, KeptPrice>,
    tick: Duration,
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
    /// The floor is decided one tick early — `PRICE_SERIES_MAX_GAP -
    /// tick_interval` — so the forced row lands at or before
    /// `PRICE_SERIES_MAX_GAP` whatever the cadence.
    ///
    /// ⚠️ **Above half the floor (300 s), the rule suppresses nothing**: two kept
    /// rows are at most 10 minutes apart, so every tick writes. Correct, but
    /// silent — [`Self::rewrites_at_most_every`] is what says so at startup.
    ///
    /// ⚠️ It reads the *configured* cadence, not the observed one. A tick that
    /// overruns its period widens the spacing by the overrun; the five minutes
    /// between the floor and [`PRICE_MAX_AGE_LATEST`] absorb a cycle of up to
    /// 450 s.
    pub fn new(tick_interval: core::time::Duration) -> Self {
        let tick = Duration::from_std(tick_interval).unwrap_or(PRICE_SERIES_MAX_GAP);

        Self {
            last: HashMap::new(),
            tick,
            max_gap: (PRICE_SERIES_MAX_GAP - tick).max(Duration::zero()),
        }
    }

    /// The longest a motionless price can go unwritten at this cadence: the
    /// first tick past the threshold, so up to one tick more than it. Stated by
    /// the price worker at startup, since an operator cannot derive it from the
    /// configuration.
    pub fn rewrites_at_most_every(&self) -> Duration {
        // A zero cadence is refused at startup; this only keeps the division
        // from panicking.
        let tick = self.tick.num_seconds().max(1);

        Duration::seconds((self.max_gap.num_seconds() / tick + 1) * tick)
    }

    /// Whether this observation earns a row: true when the mint has never been
    /// priced, when the price moved, when the provenance changed, or when the
    /// last kept row has reached the floor.
    ///
    /// ⚠️ **The comparison rounds first.** `token_prices.price_usd` is
    /// `NUMERIC(38, 18)` and Postgres rounds on write, while Jupiter sends more
    /// decimals than that: comparing the raw values would almost never find two
    /// prices equal. Both sides are rounded at [`PRICE_STORAGE_SCALE`] the way
    /// Postgres does, as [`TokenPrice::is_storable`] does.
    ///
    /// The provenance takes part, because a price from another source is
    /// another observation. `confidence` does not: an `f32` that wobbles would
    /// make every tick look new. A source that reports one must first decide
    /// what a material change of confidence is.
    pub fn worth_keeping(&self, candidate: &TokenPrice) -> bool {
        let Some(last) = self.last.get(&candidate.mint) else {
            return true;
        };

        last.price_usd != at_storage_scale(candidate.price_usd)
            || last.price_provider != candidate.price_provider
            || candidate.fetched_at - last.fetched_at > self.max_gap
    }

    /// Remember observations that have been written.
    ///
    /// ⚠️ **After a successful insert, never before**: a batch that failed left
    /// no row, and remembering it would hold the real price back until the next
    /// floor — a gap nothing backfills.
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
#[path = "model_tests.rs"]
mod tests;
