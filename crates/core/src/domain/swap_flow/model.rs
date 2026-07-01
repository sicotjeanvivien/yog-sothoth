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
    pub volume_a_to_b_usd: Decimal,

    /// USD volume of `b_to_a` swaps in the window (trader sent token B).
    pub volume_b_to_a_usd: Decimal,
}
