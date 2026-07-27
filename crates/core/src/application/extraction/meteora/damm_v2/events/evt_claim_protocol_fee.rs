//! Wire mirror of `cp-amm::EvtClaimProtocolFee` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtClaimProtocolFee`].
pub fn discriminator_claim_protocol_fee() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtClaimProtocolFee")
}

/// Mirror of `cp-amm::EvtClaimProtocolFee`.
///
/// Emitted when the protocol operator withdraws Meteora's accrued **protocol**
/// share of trading fees from a pool (distinct from [`EvtClaimPositionFee`],
/// which is an LP claiming *their position's* fees). `token_a_amount` /
/// `token_b_amount` are the absolute amounts withdrawn in this claim, aligned
/// with the canonical pool ordering.
///
/// This is the `emit_cpi!` variant (`ix_claim_protocol_fee`), the one carried
/// as a self-CPI inner instruction and thus decodable here. cp-amm also has an
/// `EvtClaimProtocolFee2` (`ix_claim_protocol_fee2`) with a different schema
/// (single `token_mint` + `amount` + receiver) emitted via a plain `emit!`
/// *log* — not an event_cpi — so it is **not** captured by this pipeline (and
/// cp-amm itself notes that log "could be truncated. should not rely on this").
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtClaimProtocolFee {
    pub pool: Pubkey,
    pub token_a_amount: u64,
    pub token_b_amount: u64,
}
