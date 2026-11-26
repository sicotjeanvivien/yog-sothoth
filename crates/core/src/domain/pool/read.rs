use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use super::Pool;
use crate::domain::{PoolCurrentState, Protocol, TokenMetadata};

/// The USD value of each side of a pool's reserves, by the rule of its TVL: a side whose
/// price is missing or stale is `None`, and `tvl_usd` the sum of the sides valued.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PoolValue {
    pub value_a_usd: Option<Decimal>,
    pub value_b_usd: Option<Decimal>,
    pub tvl_usd: Option<Decimal>,
}

/// A pool of the catalog: its identity, its tokens' identity, its state and its value now.
/// `state` is `None` before the first event that moved the pool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolEntry {
    pub pool: Pool,
    pub token_a: Option<TokenMetadata>,
    pub token_b: Option<TokenMetadata>,
    pub state: Option<PoolCurrentState>,
    pub value: PoolValue,
}

/// Which pools a list reads: every protocol's, one protocol's, or those of a token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolScope {
    All,
    Protocol(Protocol),
    /// The pools where the mint is either side.
    Token(Pubkey),
}

/// The key of the last pool of a page, and the list it was read from: a page of another
/// scope would not follow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolCursor {
    pub scope: PoolScope,
    pub first_seen_at: DateTime<Utc>,
    pub pool_address: Pubkey,
}
