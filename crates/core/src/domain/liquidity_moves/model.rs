use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use crate::domain::Protocol;

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
