use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// A funder deposits rewards into a farm slot and resets its emission rate, in
/// **Q64.64** base units per second; funding a running slot folds its leftover
/// into the new rate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2FundRewardEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    /// Position in the chain, see [`crate::domain::EventPosition`].
    pub slot: u64,
    pub transaction_index: Option<u32>,
    pub event_index: u16,
    pub funder: Pubkey,
    pub mint_reward: Pubkey,
    pub reward_index: u8,
    /// Reward base units the funder sent.
    pub amount: u64,
    /// What landed in the vault, less a transfer fee.
    pub transfer_fee_excluded_amount_in: u64,
    /// Unix timestamp (seconds) at which the current emission window ends.
    pub reward_duration_end: u64,
    /// Emission rate before this funding, Q64.64. Zero on a slot's first fund.
    pub pre_reward_rate: u128,
    /// Emission rate after this funding, Q64.64.
    pub post_reward_rate: u128,
}
