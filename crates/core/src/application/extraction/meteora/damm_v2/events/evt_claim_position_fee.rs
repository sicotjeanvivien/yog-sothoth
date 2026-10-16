//! Wire mirror of `cp-amm::EvtClaimPositionFee` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtClaimPositionFee`].
pub fn discriminator_claim_position_fee() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtClaimPositionFee")
}

/// Mirror of `cp-amm::EvtClaimPositionFee`: an LP claims its position's fees,
/// the amounts transferred by this claim (not a running total).
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtClaimPositionFee {
    pub pool: Pubkey,
    pub position: Pubkey,
    pub owner: Pubkey,
    pub fee_a_claimed: u64,
    pub fee_b_claimed: u64,
}
