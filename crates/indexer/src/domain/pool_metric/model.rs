use chrono::{DateTime, Utc};

/// A pool state snapshot — DB representation.
/// One row per transaction.
#[derive(Debug, Clone)]
pub(crate) struct PoolMetric {
    /// Pool address (base58).
    pub(crate) pool_address: String,
    /// Reserve of token A in native units.
    pub(crate) reserve_a: u64,
    /// Reserve of token B in native units.
    pub(crate) reserve_b: u64,
    /// Current price as Q64 fixed-point.
    pub(crate) price_q64: u128,
    /// Price impact of the triggering transaction in basis points.
    pub(crate) price_impact_bps: Option<u32>,
    /// Pool imbalance in basis points.
    pub(crate) imbalance_bps: Option<u32>,
    /// DAMM v2 specific — dynamic fee at time of event.
    pub(crate) fee_bps: Option<u32>,
    /// DLMM specific — active bin at time of event.
    pub(crate) active_bin_id: Option<i32>,
    /// Snapshot timestamp.
    pub(crate) timestamp: DateTime<Utc>,
}
