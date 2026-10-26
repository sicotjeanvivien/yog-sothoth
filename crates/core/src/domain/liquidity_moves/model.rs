//! The valued liquidity moves: each one, and what they bring and take, summed by protocol.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use crate::domain::{Protocol, Window};

/// The liquidity deposited at the pools' creation, added and removed over a period or a bucket.
/// The USD sums skip the moves not valued, which `moves - moves_valued` counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiquidityFlows {
    pub created_usd: Decimal,
    pub added_usd: Decimal,
    pub removed_usd: Decimal,
    pub moves: i64,
    pub moves_valued: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiquidityMoveKind {
    /// A pool's creation, which deposits its first liquidity.
    Create,
    Add,
    Remove,
}

/// One valued liquidity move, with the identity of its pool's tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiquidityMove {
    pub protocol: Protocol,
    pub at: DateTime<Utc>,
    pub signature: String,
    /// With `signature` and `at`, the key of the move.
    pub event_index: i32,
    pub pool_address: Pubkey,
    pub token_a_symbol: Option<String>,
    pub token_b_symbol: Option<String>,
    pub owner: Pubkey,
    pub kind: LiquidityMoveKind,
    pub amount_usd: Decimal,
}

/// Which moves a list reads: one protocol's, or one pool's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiquidityMoveScope {
    Protocol(Protocol),
    Pool(Pubkey),
}

/// The order of a list of liquidity moves; each breaks its ties the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LiquidityMoveOrder {
    /// Worth the most first.
    #[default]
    Amount,
    /// The most recent first.
    At,
}

/// The key of the last move of a page, and the list it was read from — its scope, window,
/// order and instant: a page of another list, or read after a freeze, would not follow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiquidityMoveCursor {
    pub scope: LiquidityMoveScope,
    pub window: Window,
    pub as_of: DateTime<Utc>,
    pub order: LiquidityMoveOrder,
    pub amount_usd: Decimal,
    pub at: DateTime<Utc>,
    pub signature: String,
    pub event_index: i32,
}
