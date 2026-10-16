//! Wire mirror of `cp-amm::EvtClaimProtocolFee` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtClaimProtocolFee`].
pub fn discriminator_claim_protocol_fee() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtClaimProtocolFee")
}

/// Mirror of `cp-amm::EvtClaimProtocolFee`: the operator withdraws Meteora's share.
/// ⚠️ `EvtClaimProtocolFee2` is a plain `emit!` log, not an event_cpi: never captured.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtClaimProtocolFee {
    pub pool: Pubkey,
    pub token_a_amount: u64,
    pub token_b_amount: u64,
}
