//! Directional swap volume per pool over a window, valued at trade time.

use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

/// One pool's swap volume per direction over a window, in USD.
#[derive(Debug, Clone, PartialEq)]
pub struct PoolSwapFlow {
    /// The pool these volumes are for.
    pub pool_address: Pubkey,

    /// USD volume of `a_to_b` swaps (trader sent token A).
    /// ⚠️ `None` unless every hour was valued, both directions together: one
    /// coalesced alone turns a missing price into a perfect imbalance.
    pub volume_a_to_b_usd: Option<Decimal>,

    /// USD volume of `b_to_a` swaps (trader sent token B).
    pub volume_b_to_a_usd: Option<Decimal>,
}
