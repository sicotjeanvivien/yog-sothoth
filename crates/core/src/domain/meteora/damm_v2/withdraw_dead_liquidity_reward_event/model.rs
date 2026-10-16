use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// A funder reclaims the rewards accrued to ownerless locked liquidity; `amount`
/// is never 0. Same fields as the ineligible-reward event, another fact.
/// Layout from the cp-amm source alone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2WithdrawDeadLiquidityRewardEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    /// Position in the chain, see [`crate::domain::EventPosition`].
    pub slot: u64,
    pub transaction_index: Option<u32>,
    pub event_index: u16,
    pub reward_mint: Pubkey,
    pub amount: u64,
}
