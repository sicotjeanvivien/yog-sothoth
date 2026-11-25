//! A protocol's TVL by token, at its last reading: the liquidity each token holds in its pools.

use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use crate::domain::TokenMetadata;

/// One token at a reading: the valued sides of its pools, and the part of them locked.
/// ⚠️ A pool counts for both its tokens: the tokens add up to twice the protocol's TVL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenTvl {
    pub mint: Pubkey,
    /// `None` until yog-context resolves the mint.
    pub metadata: Option<TokenMetadata>,
    pub tvl_usd: Decimal,
    /// `None` for a protocol without locks, or when none of its locked pools is valued.
    pub locked_usd: Option<Decimal>,
    /// Its pools, valued or not.
    pub pools: i64,
}
