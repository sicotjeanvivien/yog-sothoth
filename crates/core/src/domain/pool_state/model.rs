use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

/// Snapshot of an AMM pool's state at a given point in time.
///
/// Captured after each parsed event (swap, add/remove liquidity) so the
/// dashboard can reconstruct the pool's history without re-reading the chain.
///
/// Reserves are expressed in each token's native units (no decimal scaling).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolState {
    /// On-chain address of the AMM pool.
    pub address: Pubkey,

    /// Reserve of token A, in native units.
    pub reserve_a: u64,

    /// Reserve of token B, in native units.
    pub reserve_b: u64,

    /// Current price in Q64.64 fixed-point format (Meteora native).
    ///
    /// Represents the ratio `reserve_a / reserve_b` as a 128-bit integer
    /// where the binary point sits between bit 63 and bit 64.
    ///
    /// To convert for display (f64 precision only — never use for computation):
    /// ```
    /// let price_q64: u128 = 1 << 64; // represents 1.0
    /// let price: f64 = (price_q64 >> 64) as f64
    ///     + (price_q64 & u64::MAX as u128) as f64 / u64::MAX as f64;
    /// assert!((price - 1.0).abs() < 1e-9);
    /// ```
    pub price_q64: u128,

    /// Timestamp at which this snapshot was taken.
    pub timestamp: DateTime<Utc>,
}
