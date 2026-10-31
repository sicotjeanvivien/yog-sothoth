//! How often a candidate mint is asked of the price source: a tick's length follows the mints it
//! asks, and past the staleness bound every price it was due to rewrite stops valuing anything.

use chrono::Duration;
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use crate::domain::PRICE_CANDIDATE_WINDOW;

/// Asked one tick in this many.
/// ⚠️ These ticks must pass the floor of [`KeptPrices`](crate::domain::KeptPrices), or a motionless
/// price waits for the next turn, past the staleness bound.
const LOW_VALUE_EVERY_TICKS: u64 = 3;

/// Asked one tick in this many: a mint still without a price past its grace almost never gets one.
const NEVER_PRICED_EVERY_TICKS: u64 = 105;

/// What tells the classes of [`PriceCandidateClass`] apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PriceCandidateParams {
    /// How recent an event or a price keeps a mint a candidate.
    pub window: Duration,
    /// How long a mint known without a price stays [`PriceCandidateClass::Regular`].
    pub never_priced_grace: Duration,
    /// Below this, at its last price, what a mint's pools hold makes it low value.
    pub low_value_below_usd: Decimal,
}

/// The parameters of the price worker.
pub const PRICE_CANDIDATE_PARAMS: PriceCandidateParams = PriceCandidateParams {
    window: PRICE_CANDIDATE_WINDOW,
    never_priced_grace: Duration::hours(1),
    low_value_below_usd: Decimal::TEN,
};

/// What sets how often a candidate is asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceCandidateClass {
    Regular,
    LowValue,
    NeverPriced,
}

impl PriceCandidateClass {
    /// ⚠️ A mint never priced has no value to judge, so that rule comes first.
    pub fn of(never_priced_past_grace: bool, below_low_value: bool) -> Self {
        if never_priced_past_grace {
            Self::NeverPriced
        } else if below_low_value {
            Self::LowValue
        } else {
            Self::Regular
        }
    }

    fn every_ticks(self) -> u64 {
        match self {
            Self::Regular => 1,
            Self::LowValue => LOW_VALUE_EVERY_TICKS,
            Self::NeverPriced => NEVER_PRICED_EVERY_TICKS,
        }
    }
}

/// A mint worth asking, and how often.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PriceCandidate {
    pub mint: Pubkey,
    pub class: PriceCandidateClass,
}

impl PriceCandidate {
    /// Whether tick number `tick` asks this mint: once per cycle of its class, at a turn drawn
    /// from its address, which spreads a class over its ticks, at boot too.
    pub fn is_turn(&self, tick: u64) -> bool {
        let every = self.class.every_ticks();

        tick % every == address_slot(&self.mint) % every
    }
}

/// ⚠️ The first bytes: a vanity mint shares its last ones (`…pump`) with thousands of others.
fn address_slot(mint: &Pubkey) -> u64 {
    let mut head = [0u8; 8];
    head.copy_from_slice(&mint.to_bytes()[..8]);

    u64::from_le_bytes(head)
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
