//! A protocol's TVL by token, at its last reading: the liquidity each token holds in its pools.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use crate::domain::{Protocol, TokenMetadata};
use crate::tools::Page;

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

/// The order of the ranking, each descending then by mint; an unknown locked part last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TokenTvlOrder {
    #[default]
    Tvl,
    Locked,
}

/// The protocol's own reading, the tokens' denominator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenTvlTotal {
    pub tvl_usd: Option<Decimal>,
    pub locked_usd: Option<Decimal>,
}

/// A page of the ranking of the reading at `at`, the close of its hour.
#[derive(Debug, Clone)]
pub struct TokenTvlPage {
    pub at: DateTime<Utc>,
    /// The first vesting observed: the locked parts leave out those locked before it.
    pub covered_since: Option<DateTime<Utc>>,
    pub total: TokenTvlTotal,
    pub page: Page<TokenTvl>,
}

/// The last token of a page, in the ranking of `protocol`'s reading at `at`, in `order`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenTvlCursor {
    pub protocol: Protocol,
    pub at: DateTime<Utc>,
    pub order: TokenTvlOrder,
    /// The order's value, `None` for an unknown locked part.
    pub value: Option<Decimal>,
    pub mint: Pubkey,
}
