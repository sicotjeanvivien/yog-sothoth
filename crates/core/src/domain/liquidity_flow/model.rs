//! Liquidity flow read model.
//!
//! Directional liquidity movement for a pool (added vs removed), aggregated
//! over a time window and valued in USD (trade-time), paired with the pool's
//! *current* TVL. Derived from the hourly liquidity continuous aggregate;
//! the read-model that feeds the TVL-drain detector. Pure domain type — no
//! persistence backend leaks in here.

use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

/// Per-pool liquidity flow, in USD, over a window — plus current TVL.
///
/// The two directions are kept separate so a detector can net them (churn
/// from LPs rebalancing cancels out). Both are trade-time valued, i.e.
/// priced at the token price as-of each underlying hourly bucket. `tvl_usd`
/// is the pool's *current* TVL (live snapshot valuation), the denominator's
/// other half in a drain ratio.
#[derive(Debug, Clone, PartialEq)]
pub struct PoolLiquidityFlow {
    /// The pool these flows are for.
    pub pool_address: Pubkey,

    /// USD value added to the pool in the window (`add` events).
    ///
    /// `None` when the window was not entirely valuable — some hour in it
    /// carried an amount whose token had no usable price. **Unknown, not
    /// zero**: summing only the valuable hours would publish a sub-total
    /// dressed as a total, which under-estimates the drain and loses the
    /// signal rather than faking one. `None` together with `removed_usd`.
    pub added_usd: Option<Decimal>,

    /// USD value removed from the pool in the window (`remove` events).
    /// `None` under the same condition, and always alongside `added_usd`.
    pub removed_usd: Option<Decimal>,

    /// The pool's current TVL in USD. `None` when the pool cannot be
    /// valued (unknown token price, unresolved mints, or no reconstructed
    /// state) — a detector must skip such pools rather than guess.
    pub tvl_usd: Option<Decimal>,
}
