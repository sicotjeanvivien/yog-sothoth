//! A pool's current price inputs from the chain (`sqrt_price` of the last swap)
//! and the oracle (each token's latest USD price), for the deviation detector.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use crate::domain::Protocol;

/// Current price inputs of one comparable pool; staleness is the consumer's
/// call, from the three instants carried.
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
