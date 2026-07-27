//! Wire mirror of `cp-amm::EvtSetPoolStatus` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtSetPoolStatus`].
pub fn discriminator_set_pool_status() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtSetPoolStatus")
}

/// Mirror of `cp-amm::EvtSetPoolStatus`.
///
/// Emitted when a pool's status flag is changed (e.g. enabled/disabled).
/// `status` is the raw on-chain status byte — not interpreted here.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtSetPoolStatus {
    pub pool: Pubkey,
    pub status: u8,
}
