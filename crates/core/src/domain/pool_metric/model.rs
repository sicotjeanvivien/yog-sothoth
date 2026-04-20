use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;

/// A pool state snapshot — DB representation.
/// One row per indexed transaction.
#[derive(Debug, Clone)]
pub struct PoolMetric {
    /// On-chain address of the AMM pool.
    pub pool_address: Pubkey,

    /// Solana transaction signature that triggered this state update.
    pub signature: String,

    /// Reserve of token A in native units.
    pub reserve_a: u64,

    /// Reserve of token B in native units.
    pub reserve_b: u64,

    /// Current price as Q64 fixed-point (encoded u128).
    pub price_q64: u128,

    /// Price impact of the triggering swap in basis points.
    /// None for add/remove liquidity events.
    pub price_impact_bps: Option<i32>,

    /// Reserve imbalance in basis points.
    pub imbalance_bps: Option<i32>,

    /// DAMM v2: dynamic fee rate in effect at time of event.
    /// None for other protocols.
    pub current_fee_bps: Option<i32>,

    /// Fee amount collected in token A on this event (native units).
    pub fees_collected_a: Option<u64>,

    /// Fee amount collected in token B on this event (native units).
    pub fees_collected_b: Option<u64>,

    /// Token A volume on this event (native units).
    /// None for add/remove liquidity events.
    pub volume_a: Option<u64>,

    /// Token B volume on this event (native units).
    /// None for add/remove liquidity events.
    pub volume_b: Option<u64>,

    /// DLMM: active bin ID at time of event.
    /// None for DAMM v2 and DAMM v1.
    pub active_bin_id: Option<i32>,

    /// DLMM: bin step in basis points — constant per pool.
    /// None for DAMM v2 and DAMM v1.
    pub bin_step: Option<i16>,

    /// Snapshot timestamp.
    pub timestamp: DateTime<Utc>,
}
