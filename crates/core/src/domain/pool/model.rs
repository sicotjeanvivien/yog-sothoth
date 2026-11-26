use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;

use crate::domain::Protocol;

/// A pool discovered in the transaction stream: identity and stable metadata.
/// `first_seen_at` is set once, `last_seen_at` refreshed by every event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pool {
    /// On-chain address of the AMM pool.
    pub pool_address: Pubkey,

    /// Protocol this pool belongs to.
    pub protocol: Protocol,

    /// Mint of token A. `None` until resolved by yog-context.
    pub token_a_mint: Option<Pubkey>,

    /// Mint of token B. `None` until resolved by yog-context.
    pub token_b_mint: Option<Pubkey>,

    /// Base fee in bps, from the pool account; `None` until yog-context reads it.
    /// ⚠️ For a fee scheduler this is the genesis **cliff**, not the fee paid now
    /// (`base_fee_numerator_at`).
    pub fee_bps: Option<Decimal>,

    /// When Yog-Sothoth first observed this pool in the transaction stream.
    pub first_seen_at: DateTime<Utc>,

    /// Last time any event touched this pool.
    pub last_seen_at: DateTime<Utc>,
}
