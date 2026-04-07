use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Snapshot of an AMM pool's state at a given point in time.
///
/// Reserves are in native token units.
/// Price is stored as a Q64 fixed-point integer (Meteora native format):
///   actual_price = price_q64 / 2^64
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolState {
    /// Pool address (base58).
    pub address: String,
    /// Reserve of token A, in native units.
    pub reserve_a: u64,
    /// Reserve of token B, in native units.
    pub reserve_b: u64,
    /// Current price as Q64 fixed-point.
    /// Convert to f64 for display only: price_q64 as f64 / (1u128 << 64) as f64
    pub price_q64: u128,
    /// Snapshot timestamp.
    pub timestamp: DateTime<Utc>,
}