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

impl LiquidityMoveKind {
    /// ⚠️ The labels `analytic_liquidity_moves.kind` holds: another would match no stored move.
    pub const fn as_str(self) -> &'static str {
        match self {
            LiquidityMoveKind::Create => "create",
            LiquidityMoveKind::Add => "add",
            LiquidityMoveKind::Remove => "remove",
        }
    }

    /// The kind `label` names, `None` for a label [`Self::as_str`] never writes.
    pub fn from_label(label: &str) -> Option<Self> {
        [Self::Create, Self::Add, Self::Remove]
            .into_iter()
            .find(|kind| kind.as_str() == label)
    }
}

/// The lock a move's transaction puts on a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiquidityMoveLock {
    None,
    Vesting,
    Permanent,
}

impl LiquidityMoveLock {
    /// ⚠️ The labels `analytic_liquidity_moves.lock` holds, checked by migration 043.
    pub const fn as_str(self) -> &'static str {
        match self {
            LiquidityMoveLock::None => "none",
            LiquidityMoveLock::Vesting => "vesting",
            LiquidityMoveLock::Permanent => "permanent",
        }
    }

    /// The lock `label` names, `None` for a label [`Self::as_str`] never writes.
    pub fn from_label(label: &str) -> Option<Self> {
        [Self::None, Self::Vesting, Self::Permanent]
            .into_iter()
            .find(|lock| lock.as_str() == label)
    }
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
    /// `None` for a move valued before its lock was measured.
    pub lock: Option<LiquidityMoveLock>,
    pub amount_usd: Decimal,
}

/// Which moves a list reads: one protocol's, or one pool's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiquidityMoveScope {
    Protocol(Protocol),
    Pool(Pubkey),
}

/// Which moves a list reads: its scope's, of one kind when it keeps one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiquidityMoveList {
    pub scope: LiquidityMoveScope,
    pub kind: Option<LiquidityMoveKind>,
}

impl LiquidityMoveList {
    /// Every move of `scope`, whatever its kind.
    pub fn every_kind(scope: LiquidityMoveScope) -> Self {
        Self { scope, kind: None }
    }
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

/// The key of the last move of a page, and the list it was read from — its scope and kind,
/// window, order and instant: a page of another list, or read after a freeze, would not follow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiquidityMoveCursor {
    pub list: LiquidityMoveList,
    pub window: Window,
    pub as_of: DateTime<Utc>,
    pub order: LiquidityMoveOrder,
    pub amount_usd: Decimal,
    pub at: DateTime<Utc>,
    pub signature: String,
    pub event_index: i32,
}
