use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

use crate::domain::TradeDirection;

/// A DAMM v2 swap; amounts and reserves (after the swap) in the pool's own A/B order.
/// ⚠️ That order is not a sort of the mints: never a canonical pair key, and SQL
/// `LEAST` on base58 text disagrees with `Pubkey::min` on raw bytes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2SwapEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    /// Position in the chain, see [`crate::domain::EventPosition`].
    pub slot: u64,
    pub transaction_index: Option<u32>,
    pub event_index: u16,
    pub trade_direction: TradeDirection,
    pub amount_a: u64,
    pub amount_b: u64,
    pub reserve_a_after: u64,
    pub reserve_b_after: u64,
    pub next_sqrt_price: u128,
    pub claiming_fee: u64,
    pub protocol_fee: u64,
    pub compounding_fee: u64,
    pub referral_fee: u64,
    pub fee_token_is_a: bool,
}

impl MeteoraDammV2SwapEvent {
    /// Total fee of the swap, in the token `fee_token_is_a` designates.
    pub fn fee_total(&self) -> u64 {
        self.claiming_fee
            .saturating_add(self.protocol_fee)
            .saturating_add(self.compounding_fee)
            .saturating_add(self.referral_fee)
    }
}
