//! Wire mirror of `cp-amm::EvtCreatePosition` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtCreatePosition`].
pub fn discriminator_create_position() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtCreatePosition")
}

/// Mirror of `cp-amm::EvtCreatePosition`: an LP opens an empty position, an NFT
/// (`position_nft_mint`) over the `position` PDA.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtCreatePosition {
    pub pool: Pubkey,
    pub owner: Pubkey,
    pub position: Pubkey,
    pub position_nft_mint: Pubkey,
}
