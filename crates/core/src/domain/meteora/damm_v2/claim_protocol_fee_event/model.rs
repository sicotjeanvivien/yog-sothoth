use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// Protocol operator withdrawing Meteora's accrued **protocol** share of a
/// pool's trading fees.
///
/// Emitted on-chain by `claim_protocol_fee` (the `emit_cpi!` variant). Distinct
/// from [`crate::domain::MeteoraDammV2ClaimPositionFeeEvent`], which is an LP
/// claiming *their position's* fees — this is the protocol's own cut leaving
/// the pool. `token_a_amount` / `token_b_amount` are the absolute amounts
/// withdrawn in this claim, aligned with the canonical pool ordering.
///
/// Note: cp-amm also has a `claim_protocol_fee2` instruction emitting a
/// differently-shaped `EvtClaimProtocolFee2` (single `token_mint` + `amount`)
/// via a plain `emit!` *log* rather than an event_cpi; that variant is not
/// captured by the current extraction pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2ClaimProtocolFeeEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    pub token_a_amount: u64,
    pub token_b_amount: u64,
}
