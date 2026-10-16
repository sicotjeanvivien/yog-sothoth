//! Liquidity added and removed per pool over a window, in USD, with current TVL.

use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

/// One pool's liquidity flow over a window, valued at trade time, and its
/// *current* TVL; the two directions stay apart so a detector can net them.
#[derive(Debug, Clone, PartialEq)]
pub struct PoolLiquidityFlow {
    /// The pool these flows are for.
    pub pool_address: Pubkey,

    /// USD value added in the window.
    /// ⚠️ `None` unless every hour was valued: a sub-total would under-state a drain.
    pub added_usd: Option<Decimal>,

    /// USD value removed in the window; `None` together with `added_usd`.
    pub removed_usd: Option<Decimal>,

    /// The pool's current TVL; `None` when it cannot be valued, and the pool is skipped.
    pub tvl_usd: Option<Decimal>,
}
