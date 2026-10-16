use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// Pool genesis: mints, initial AMM state and seeded amounts, in the program's
/// A/B order. ⚠️ `sqrt_price` and its bounds are expressed against that order:
/// never re-order the pair without inverting the price.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2InitializePoolEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    /// Position in the chain, see [`crate::domain::EventPosition`].
    pub slot: u64,
    pub transaction_index: Option<u32>,
    pub event_index: u16,

    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub creator: Pubkey,
    pub payer: Pubkey,
    pub alpha_vault: Pubkey,

    pub sqrt_min_price: u128,
    pub sqrt_max_price: u128,
    pub sqrt_price: u128,
    pub liquidity: u128,

    pub activation_type: u8,
    pub activation_point: u64,
    pub collect_fee_mode: u8,
    pub pool_type: u8,

    pub token_a_flag: u8,
    pub token_b_flag: u8,
    pub token_a_amount: u64,
    pub token_b_amount: u64,
    pub total_amount_a: u64,
    pub total_amount_b: u64,

    /// Raw borsh bytes of the on-chain `PoolFeeParameters`, never decoded.
    pub pool_fees_raw: Vec<u8>,
}
