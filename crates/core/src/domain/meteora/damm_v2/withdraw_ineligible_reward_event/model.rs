use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// A funder reclaims the rewards accrued while no liquidity was in range,
/// after the window ends.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2WithdrawIneligibleRewardEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    /// Position in the chain, see [`crate::domain::EventPosition`].
    pub slot: u64,
    pub transaction_index: Option<u32>,
    pub event_index: u16,
    pub reward_mint: Pubkey,
    /// Reward base units returned to the funder; zero is legitimate.
    pub amount: u64,
}
