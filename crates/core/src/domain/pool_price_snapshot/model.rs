//! Pool price snapshot read model.
//!
//! A pool's *current* price inputs from both sources: the on-chain side
//! (the raw `sqrt_price` of the last swap — decoded to a spot price by the
//! consumer, since the Q64.64 interpretation is protocol-specific) and the
//! oracle side (each token's most recent USD price observation, with its
//! fetch instant so the consumer can gate on staleness). The read model
//! feeding the price-oracle-deviation detector. Pure domain type — no
//! persistence backend leaks in here.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use crate::domain::Protocol;

/// Current price inputs for one pool, from both the chain and the oracle.
///
/// Only pools that *can* be compared appear as snapshots: mints and decimals
/// resolved, at least one swap observed, and a price observation for both
/// tokens — hence no `Option` fields. Staleness, however, is the consumer's
/// call: `last_swap_at` and the two `fetched_at` instants are carried so it
/// can decide how old is too old.
#[derive(Debug, Clone, PartialEq)]
pub struct PoolPriceSnapshot {
    /// The pool this snapshot is for.
    pub pool_address: Pubkey,

    /// Protocol of the pool — dictates how `sqrt_price` is decoded.
    pub protocol: Protocol,

    /// Raw Q64.64 `sqrt_price` of the pool's most recent swap.
    pub sqrt_price: u128,

    /// When that swap happened — the age of the spot price.
    pub last_swap_at: DateTime<Utc>,

    /// Token A's on-chain decimals.
    pub decimals_a: u8,

    /// Token B's on-chain decimals.
    pub decimals_b: u8,

    /// Most recent oracle USD price of token A.
    pub price_a_usd: Decimal,

    /// When token A's price was fetched — the age of the oracle price.
    pub price_a_fetched_at: DateTime<Utc>,

    /// Most recent oracle USD price of token B.
    pub price_b_usd: Decimal,

    /// When token B's price was fetched.
    pub price_b_fetched_at: DateTime<Utc>,
}
