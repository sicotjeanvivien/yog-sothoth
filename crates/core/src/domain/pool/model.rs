use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;

use crate::domain::{PoolCurrentState, Protocol, TokenMetadata};

/// A pool discovered in the transaction stream: identity and stable metadata.
/// `first_seen_at` is set once, `last_seen_at` refreshed by every event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pool {
    /// On-chain address of the AMM pool.
    pub pool_address: Pubkey,

    /// Protocol this pool belongs to.
    pub protocol: Protocol,

    /// Mint of token A. `None` until resolved by yog-context.
    pub token_a_mint: Option<Pubkey>,

    /// Mint of token B. `None` until resolved by yog-context.
    pub token_b_mint: Option<Pubkey>,

    /// Base fee in bps, from the pool account; `None` until yog-context reads it.
    /// ⚠️ For a fee scheduler this is the genesis **cliff**, not the fee paid now
    /// (`base_fee_numerator_at`).
    pub fee_bps: Option<Decimal>,

    /// When Yog-Sothoth first observed this pool in the transaction stream.
    pub first_seen_at: DateTime<Utc>,

    /// Last time any event touched this pool.
    pub last_seen_at: DateTime<Utc>,
}

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
