use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

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
pub struct MeteoraDammV2ClaimPositionFeeEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    pub position: Pubkey,
    pub owner: Pubkey,
    pub fee_a_claimed: u64,
    pub fee_b_claimed: u64,
}
