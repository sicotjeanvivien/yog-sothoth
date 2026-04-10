use chrono::{DateTime, Utc};

/// A pool state snapshot — DB representation.
/// One row per indexed transaction.
#[derive(Debug, Clone)]
pub(crate) struct PoolMetric {
    /// Pool address (base58).
    pub(crate) pool_address: String,
    /// Solana transaction signature that triggered this state update.
    pub(crate) signature: String,
    /// Reserve of token A in native units.
    pub(crate) reserve_a: u64,
    /// Reserve of token B in native units.
    pub(crate) reserve_b: u64,
    /// Current price as Q64 fixed-point (encoded u128).
    pub(crate) price_q64: u128,
    /// Price impact of the triggering swap in basis points.
    /// None for add/remove liquidity events.
    pub(crate) price_impact_bps: Option<i32>,
    /// Reserve imbalance in basis points.
    pub(crate) imbalance_bps: Option<i32>,
    /// DAMM v2: dynamic fee rate in effect at time of event.
    /// None for other protocols.
    pub(crate) current_fee_bps: Option<i32>,
    /// Fee amount collected in token A on this event (native units).
    pub(crate) fees_collected_a: Option<u64>,
    /// Fee amount collected in token B on this event (native units).
    pub(crate) fees_collected_b: Option<u64>,
    /// Token A volume on this event (native units).
    /// None for add/remove liquidity events.
    pub(crate) volume_a: Option<u64>,
    /// Token B volume on this event (native units).
    /// None for add/remove liquidity events.
    pub(crate) volume_b: Option<u64>,
    /// DLMM: active bin ID at time of event.
    /// None for DAMM v2 and DAMM v1.
    pub(crate) active_bin_id: Option<i32>,
    /// DLMM: bin step in basis points — constant per pool.
    /// None for DAMM v2 and DAMM v1.
    pub(crate) bin_step: Option<i16>,
    /// Snapshot timestamp.
    pub(crate) timestamp: DateTime<Utc>,
}