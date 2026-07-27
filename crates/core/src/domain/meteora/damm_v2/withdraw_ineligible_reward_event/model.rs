use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// A funder **reclaiming reward tokens that nobody could earn**.
///
/// Rewards accrue continuously once a slot is funded, but only in-range LPs are
/// eligible for them. Whatever accrued while the pool had **zero eligible
/// liquidity** can never be claimed by anyone; this event returns it to the
/// funder, and is only permitted once the emission window has ended.
///
/// Product angle: a large `amount` relative to what was funded means the farm
/// largely emitted into an empty pool — the incentive did not buy the liquidity
/// it paid for. Read alongside
/// [`crate::domain::MeteoraDammV2FundRewardEvent`] for the funded side.
///
/// Note: cp-amm has a structurally identical `EvtWithdrawDeadLiquidityReward`
/// (same three fields) for the reward share of permanently locked liquidity
/// with no owner to claim it. It is a *distinct* event with its own
/// discriminator and is not indexed yet — no fixture captured.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2WithdrawIneligibleRewardEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    pub reward_mint: Pubkey,
    /// Reward base units returned to the funder. Legitimately zero when the
    /// pool always had eligible liquidity — the instruction still runs.
    pub amount: u64,
}
