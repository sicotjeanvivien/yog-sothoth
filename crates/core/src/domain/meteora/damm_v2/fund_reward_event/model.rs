use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// A funder **depositing rewards into a farm slot** and (re)setting its
/// emission rate.
///
/// This is the economic core of the rewards family: the slot declared by
/// [`crate::domain::MeteoraDammV2InitializeRewardEvent`] distributes nothing
/// until it is funded here. Product angle: a pool whose liquidity rests on
/// emissions is *mercenary* — it leaves when the farm stops. Comparing the
/// emission rate against fee revenue separates "yield inflated by emissions"
/// from "organic yield", and `reward_duration_end` approaching is a leading
/// indicator of liquidity flight.
///
/// # Rate scale — Q64.64
///
/// `pre_reward_rate` / `post_reward_rate` are reward base units per second in
/// **Q64.64 fixed point**: divide by `2^64` to read them as a plain rate. On a
/// freshly opened slot this holds exactly:
///
/// ```text
/// post_reward_rate == (amount << 64) / reward_duration
/// ```
///
/// # Carry-forward
///
/// Funding an already-running slot folds the undistributed remainder of the
/// current window into the new one, so `post_reward_rate` reflects
/// `amount + leftover`. cp-amm exposes this only through the rate pair — there
/// is no explicit `carry_forward` field on the event. The leftover is
/// recoverable as `(post_reward_rate * duration >> 64) - amount`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2FundRewardEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    pub funder: Pubkey,
    pub mint_reward: Pubkey,
    pub reward_index: u8,
    /// Reward base units the funder sent.
    pub amount: u64,
    /// What actually landed in the vault — differs from `amount` only for
    /// Token-2022 mints charging a transfer fee.
    pub transfer_fee_excluded_amount_in: u64,
    /// Unix timestamp (seconds) at which the current emission window ends.
    pub reward_duration_end: u64,
    /// Emission rate before this funding, Q64.64. Zero on a slot's first fund.
    pub pre_reward_rate: u128,
    /// Emission rate after this funding, Q64.64.
    pub post_reward_rate: u128,
}
