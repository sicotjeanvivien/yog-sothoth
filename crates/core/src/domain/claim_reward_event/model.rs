use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

use crate::domain::Protocol;

/// LP claim of farming rewards.
///
/// Distinct from [`crate::domain::ClaimPositionFeeEvent`]: a "reward" is a
/// separate token distributed by the pool (set up via `initialize_reward` /
/// `fund_reward`), whereas a "position fee" is the trader fee accrued on the
/// position itself.
///
/// A pool can have multiple concurrent reward streams; `reward_index`
/// disambiguates within the pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimRewardEvent {
    // ── Identification ──────────────────────────────────────────────────────
    pub pool_address: Pubkey,
    pub protocol: Protocol,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,

    pub position: Pubkey,
    pub owner: Pubkey,

    /// Mint of the reward token.
    pub mint_reward: Pubkey,

    /// Index of the reward stream within the pool (0-based).
    pub reward_index: u8,

    /// Total amount of reward token transferred to the owner in this claim.
    pub total_reward: u64,
}
