//! Wire mirror of `cp-amm::EvtInitializeReward` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtInitializeReward`].
pub fn discriminator_initialize_reward() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtInitializeReward")
}

/// Mirror of `cp-amm::EvtInitializeReward`.
///
/// Emitted when an admin **opens a reward slot** on a pool: it declares which
/// token will be distributed (`reward_mint`), who is allowed to fund it
/// (`funder`), which of the pool's slots is being opened (`reward_index`) and
/// the length of a funding window in seconds (`reward_duration`).
///
/// Opening a slot distributes nothing on its own — the tokens and the emission
/// rate arrive with [`EvtFundReward`], which typically follows in the same
/// transaction.
///
/// `funder` and `creator` are frequently the same wallet, so a fixture cannot
/// discriminate their order — it comes from the cp-amm source.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtInitializeReward {
    pub pool: Pubkey,
    pub reward_mint: Pubkey,
    pub funder: Pubkey,
    pub creator: Pubkey,
    pub reward_index: u8,
    pub reward_duration: u64,
}
