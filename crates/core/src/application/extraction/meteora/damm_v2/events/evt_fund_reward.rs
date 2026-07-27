//! Wire mirror of `cp-amm::EvtFundReward` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtFundReward`].
pub fn discriminator_fund_reward() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtFundReward")
}

/// Mirror of `cp-amm::EvtFundReward`.
///
/// The economic core of the farm: the funder deposits `amount` reward tokens
/// into a slot and the program **recomputes the emission rate** over the slot's
/// configured duration.
///
/// ## Rate scale — Q64.64
///
/// `pre_reward_rate` / `post_reward_rate` are reward base units per second in
/// **Q64.64 fixed point**: divide by `2^64` to read them as a plain rate. On a
/// freshly opened slot this holds exactly:
///
/// ```text
/// post_reward_rate == (amount << 64) / reward_duration
/// ```
///
/// ## Carry-forward
///
/// Funding an already-running slot does not discard what is left of the current
/// window: the program folds the undistributed remainder into the new window, so
/// `post_reward_rate` reflects `amount + leftover`, not `amount` alone. cp-amm
/// exposes this only through the rate pair — there is no explicit
/// `carry_forward` field on the event. The leftover is therefore recoverable as
/// `(post_reward_rate * duration >> 64) - amount`.
///
/// `amount` is what the funder sent; `transfer_fee_excluded_amount_in` is what
/// actually landed in the vault. They differ only for Token-2022 mints charging
/// a transfer fee.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtFundReward {
    pub pool: Pubkey,
    pub funder: Pubkey,
    pub mint_reward: Pubkey,
    pub reward_index: u8,
    pub amount: u64,
    pub transfer_fee_excluded_amount_in: u64,
    /// Unix timestamp (seconds) at which the current emission window ends.
    pub reward_duration_end: u64,
    pub pre_reward_rate: u128,
    pub post_reward_rate: u128,
}
