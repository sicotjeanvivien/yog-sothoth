//! What the price series has already recorded, and what a new observation has
//! to say to earn a row: without this rule the series grows at the rate of the
//! worker, not of the prices.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use rust_decimal::{Decimal, RoundingStrategy};
use solana_pubkey::Pubkey;

use crate::domain::{PRICE_STORAGE_SCALE, PriceProvider, TokenPrice};

/// How old the latest observation may be and still count as current.
/// ⚠️ A mirror of `yog_price_max_age_latest()` (migration 005): a migration
/// that redefines it must revisit this constant — nothing enforces that.
pub const PRICE_MAX_AGE_LATEST: Duration = Duration::minutes(15);

/// The widest gap between two kept observations of a mint at the cadence: below
/// [`PRICE_MAX_AGE_LATEST`] by at least a tick's length, or a motionless price stops valuing.
/// ⚠️ Under three ticks too: a low-value mint is seen one tick in three.
const PRICE_SERIES_MAX_GAP: Duration = Duration::seconds(510);

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
    /// ⚠️ The floor is decided one tick early, or the forced row lands *past* it.
    /// Above half the floor nothing is suppressed: [`Self::rewrites_at_most_every`] says so.
    pub fn new(tick_interval: core::time::Duration) -> Self {
        let tick = Duration::from_std(tick_interval).unwrap_or(PRICE_SERIES_MAX_GAP);

        Self {
            last: HashMap::new(),
            tick,
            max_gap: (PRICE_SERIES_MAX_GAP - tick).max(Duration::zero()),
        }
    }

    /// How long a motionless price waits at this cadence, stated at startup.
    pub fn rewrites_at_most_every(&self) -> Duration {
        // A zero cadence is refused at startup; this only avoids a panic.
        let tick = self.tick.num_seconds().max(1);

        Duration::seconds((self.max_gap.num_seconds() / tick + 1) * tick)
    }

    /// Whether this observation earns a row: a new mint, a moved price, another
    /// provenance, or a last row at the floor. `confidence` is ignored.
    /// ⚠️ Compared at [`PRICE_STORAGE_SCALE`]: raw values almost never compare equal.
    pub fn worth_keeping(&self, candidate: &TokenPrice) -> bool {
        let Some(last) = self.last.get(&candidate.mint) else {
            return true;
        };

        last.price_usd != at_storage_scale(candidate.price_usd)
            || last.price_provider != candidate.price_provider
            || candidate.fetched_at - last.fetched_at > self.max_gap
    }

    /// Remember observations that have been written.
    /// ⚠️ After a successful insert only, or a failed batch holds the price back.
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
