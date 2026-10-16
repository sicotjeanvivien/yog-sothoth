//! Wire mirror of `cp-amm::EvtClosePosition` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtClosePosition`].
pub fn discriminator_close_position() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtClosePosition")
}

/// Mirror of `cp-amm::EvtClosePosition`: the position account is torn down.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtClosePosition {
    pub pool: Pubkey,
    pub owner: Pubkey,
    pub position: Pubkey,
    pub position_nft_mint: Pubkey,
}
