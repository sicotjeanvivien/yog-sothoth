use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// The operator withdraws Meteora's protocol share of a pool's fees, in the
/// pool's own A/B order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2ClaimProtocolFeeEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    /// Position in the chain, see [`crate::domain::EventPosition`].
    pub slot: u64,
    pub transaction_index: Option<u32>,
    pub event_index: u16,
    pub token_a_amount: u64,
    pub token_b_amount: u64,
}
