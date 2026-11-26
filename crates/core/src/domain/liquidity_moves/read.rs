use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use super::LiquidityMoveKind;
use crate::domain::{Protocol, Window};

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
/// window, order and instant: a page of another list would not follow.
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
