//! What the price series has already recorded, and what a new observation has
//! to say to earn a row: without this rule the series grows at the rate of the
//! worker, not of the prices.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use rust_decimal::{Decimal, RoundingStrategy};
use solana_pubkey::Pubkey;

use crate::domain::{PRICE_STORAGE_SCALE, PriceProvider, TokenPrice};

/// How old the latest observation may be and still count as current.
///
/// ⚠️ A mirror of `yog_price_max_age_latest()` (migration 005): a migration
/// that redefines it must revisit this constant — nothing enforces that.
pub const PRICE_MAX_AGE_LATEST: Duration = Duration::minutes(15);

/// The widest gap between two kept observations of a mint: below
/// [`PRICE_MAX_AGE_LATEST`], so that a price that never moves is rewritten
/// before it stops valuing anything.
const PRICE_SERIES_MAX_GAP: Duration = Duration::minutes(10);

/// The last observation kept for each mint. Held by the price worker, the only
/// writer of `token_prices`; empty at boot, which costs one full batch.
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
    /// ⚠️ The floor is decided one tick early, or the forced row would land at
    /// the first tick *past* it. Above a 300 s cadence nothing is suppressed,
    /// silently — [`Self::rewrites_at_most_every`] says so at startup. A tick
    /// that overruns its cadence widens the spacing; the margin up to
    /// [`PRICE_MAX_AGE_LATEST`] absorbs a cycle of up to 450 s.
    pub fn new(tick_interval: core::time::Duration) -> Self {
        let tick = Duration::from_std(tick_interval).unwrap_or(PRICE_SERIES_MAX_GAP);

        Self {
            last: HashMap::new(),
            tick,
            max_gap: (PRICE_SERIES_MAX_GAP - tick).max(Duration::zero()),
        }
    }

    /// The longest a motionless price goes unwritten at this cadence, stated by
    /// the price worker at startup.
    pub fn rewrites_at_most_every(&self) -> Duration {
        // A zero cadence is refused at startup; this only avoids a panic.
        let tick = self.tick.num_seconds().max(1);

        Duration::seconds((self.max_gap.num_seconds() / tick + 1) * tick)
    }

    /// Whether this observation earns a row: a new mint, a moved price, another
    /// provenance, or a last row at the floor.
    ///
    /// ⚠️ Both sides are rounded at [`PRICE_STORAGE_SCALE`] first: Postgres
    /// rounds on write and Jupiter sends more decimals, so raw values would
    /// almost never compare equal. `confidence` is left out: an `f32` that
    /// wobbles would make every tick look new.
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
    /// ⚠️ After a successful insert only: remembering a failed batch would hold
    /// the real price back until the next floor.
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
