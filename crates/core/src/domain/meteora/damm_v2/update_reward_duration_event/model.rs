use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// An admin re-paces a reward slot: later fundings divide by the new duration,
/// the running window is not re-rated. Layout from the cp-amm source alone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2UpdateRewardDurationEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    /// Position in the chain, see [`crate::domain::EventPosition`].
    pub slot: u64,
    pub transaction_index: Option<u32>,
    pub event_index: u16,
    pub reward_index: u8,
    /// Window length before the change, in seconds.
    pub old_reward_duration: u64,
    /// Window length after the change, in seconds.
    pub new_reward_duration: u64,
}
