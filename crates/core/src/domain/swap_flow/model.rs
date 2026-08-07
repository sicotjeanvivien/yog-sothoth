//! Swap flow read model.
//!
//! Directional swap volume for a pool, aggregated over a time window and
//! valued in USD (trade-time). Derived from the hourly swap continuous
//! aggregate; the read-model that feeds the flow-imbalance detector. Pure
//! domain type — no persistence backend leaks in here.

use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

/// Per-pool directional swap volume, in USD, over a window.
///
/// The two directions are kept separate so a detector can measure the
/// imbalance between them. Both legs are trade-time valued, i.e. priced at
/// the token price as-of each underlying hourly bucket.
#[derive(Debug, Clone, PartialEq)]
pub struct PoolSwapFlow {
    /// The pool these volumes are for.
    pub pool_address: Pubkey,

    /// USD volume of `a_to_b` swaps in the window (trader sent token A).
    ///
    /// `None` when the window was not entirely valuable — some hour in it
    /// carried an amount whose token had no usable price. That is **unknown,
    /// not zero**: a detector must skip the pool rather than read it as an
    /// absence of flow. The two directions are `None` together, never one
    /// without the other; coalescing them independently is what used to turn a
    /// missing price into a perfect `-1.0` imbalance.
    pub volume_a_to_b_usd: Option<Decimal>,

    /// USD volume of `b_to_a` swaps in the window (trader sent token B).
    /// `None` under the same condition, and always alongside the other
    /// direction.
    pub volume_b_to_a_usd: Option<Decimal>,
}
