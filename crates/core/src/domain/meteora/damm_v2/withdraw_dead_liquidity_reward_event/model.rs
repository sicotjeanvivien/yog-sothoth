use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// A funder **reclaiming the reward share of dead liquidity**.
///
/// Dead liquidity is liquidity permanently locked with no owner left to claim
/// against it. Rewards still accrue to it, and nobody can ever collect them;
/// this event returns that share to the funder.
///
/// Distinct from
/// [`crate::domain::MeteoraDammV2WithdrawIneligibleRewardEvent`], which covers
/// rewards accrued while the pool had *no eligible liquidity at all*. Same three
/// fields, different on-chain fact — hence a separate type and table.
///
/// **`amount` is always > 0**: cp-amm only emits this event inside
/// `if dead_liquidity_reward > 0`, unlike the ineligible-reward variant which
/// emits even for a zero amount.
///
/// Layout from the cp-amm source; no on-chain fixture captured for this event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2WithdrawDeadLiquidityRewardEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    pub reward_mint: Pubkey,
    pub amount: u64,
}
