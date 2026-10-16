use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// An admin opens a farming reward slot: its token, its funder and the length of
/// a funding window. Tokens and rate arrive with the fund-reward event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2InitializeRewardEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    /// Position in the chain, see [`crate::domain::EventPosition`].
    pub slot: u64,
    pub transaction_index: Option<u32>,
    pub event_index: u16,
    pub reward_mint: Pubkey,
    pub funder: Pubkey,
    pub creator: Pubkey,
    pub reward_index: u8,
    /// Length of a funding window, in seconds.
    pub reward_duration: u64,
}
