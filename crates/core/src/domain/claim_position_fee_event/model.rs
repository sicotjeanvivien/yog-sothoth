use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;

use crate::domain::Protocol;

/// LP claim of accumulated trading fees on a position.
///
/// Emitted on-chain by `claim_position_fee`. The `fee_*_claimed` fields are
/// absolute amounts transferred in this specific claim — the protocol does
/// not expose a "since-last-claim" delta, only the current transfer.
///
/// # Conventions
///
/// `fee_a_claimed` / `fee_b_claimed` align with the canonical pool ordering
/// — see [`crate::domain::SwapEvent`] for details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimPositionFeeEvent {
    pub pool_address: Pubkey,
    pub protocol: Protocol,
    pub signature: String,
    pub timestamp: DateTime<Utc>,

    pub position: Pubkey,
    pub owner: Pubkey,

    /// Amount of token A fees transferred to the owner in this claim.
    pub fee_a_claimed: u64,

    /// Amount of token B fees transferred to the owner in this claim.
    pub fee_b_claimed: u64,
}
