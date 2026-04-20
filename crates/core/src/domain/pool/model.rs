use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;

use crate::domain::Protocol;

/// A discovered pool — identity and stable metadata.
///
/// Yog-Sothoth observes entire protocols, so pools are discovered on the fly
/// as they appear in the transaction stream. This struct stores what we know
/// about a pool independently of its state at any given moment (that's what
/// [`PoolMetric`] is for).
///
/// Rows are upserted on every parsed event: `first_seen_at` is set once on
/// first observation, `last_seen_at` is refreshed on every subsequent event.
///
/// # Mint ordering
///
/// Mints are stored in **byte-wise order of the raw 32-byte pubkey**
/// (the default `Ord` impl of `Pubkey`), not in base58 string order.
/// This ensures the same pool always yields the same `(token_a, token_b)`
/// regardless of swap direction.
///
/// This differs from the Meteora SDK's canonical A/B ordering — adjust
/// at query time if alignment is needed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pool {
    /// On-chain address of the AMM pool.
    pub pool_address: Pubkey,

    /// Protocol this pool belongs to (DAMM v2, DAMM v1, DLMM).
    pub protocol: Protocol,

    /// Mint of token A, as defined by the protocol's account ordering.
    pub token_a_mint: Pubkey,

    /// Mint of token B, as defined by the protocol's account ordering.
    pub token_b_mint: Pubkey,

    /// When Yog-Sothoth first observed this pool in the transaction stream.
    pub first_seen_at: DateTime<Utc>,

    /// Last time any event touched this pool.
    pub last_seen_at: DateTime<Utc>,
}
