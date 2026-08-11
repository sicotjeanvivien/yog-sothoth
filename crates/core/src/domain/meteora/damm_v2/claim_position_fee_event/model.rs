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
/// `fee_a_claimed` / `fee_b_claimed` are in the pool's own
/// `(token_a, token_b)` order — see
/// [`crate::domain::MeteoraDammV2SwapEvent`] for what that order guarantees,
/// and for why it is not a sort of the mints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2ClaimPositionFeeEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    /// Position in the chain — see [`crate::domain::EventPosition`].
    pub slot: u64,
    pub transaction_index: Option<u32>,
    pub event_index: u16,
    pub position: Pubkey,
    pub owner: Pubkey,
    pub fee_a_claimed: u64,
    pub fee_b_claimed: u64,
}
