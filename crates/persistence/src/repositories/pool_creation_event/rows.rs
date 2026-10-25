use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use yog_core::{RepositoryError, domain::MeteoraDammV2InitializePoolEvent};

use crate::repositories::helper::{
    convert_bigdecimal_to_u128, convert_i16_to_u8, convert_i32_to_u16, convert_i64_to_u32,
    convert_i64_to_u64, convert_optional, convert_string_to_pubkey, convert_string_to_signature,
};

/// A row of `meteora_damm_v2_initialize_pool_events`, as the lists read it.
pub(super) struct MeteoraDammV2InitializePoolEventRow {
    pub(super) pool_address: String,
    pub(super) signature: String,
    pub(super) timestamp: DateTime<Utc>,
    pub(super) slot: i64,
    pub(super) transaction_index: Option<i64>,
    pub(super) event_index: i32,
    pub(super) token_a_mint: String,
    pub(super) token_b_mint: String,
    pub(super) creator: String,
    pub(super) payer: String,
    pub(super) alpha_vault: String,
    pub(super) sqrt_min_price: BigDecimal,
    pub(super) sqrt_max_price: BigDecimal,
    pub(super) sqrt_price: BigDecimal,
    pub(super) liquidity: BigDecimal,
    pub(super) activation_type: i16,
    pub(super) activation_point: i64,
    pub(super) collect_fee_mode: i16,
    pub(super) pool_type: i16,
    pub(super) token_a_flag: i16,
    pub(super) token_b_flag: i16,
    pub(super) token_a_amount: i64,
    pub(super) token_b_amount: i64,
    pub(super) total_amount_a: i64,
    pub(super) total_amount_b: i64,
    pub(super) pool_fees_raw: Vec<u8>,
}

impl TryFrom<MeteoraDammV2InitializePoolEventRow> for MeteoraDammV2InitializePoolEvent {
    type Error = RepositoryError;

    fn try_from(row: MeteoraDammV2InitializePoolEventRow) -> Result<Self, Self::Error> {
        Ok(MeteoraDammV2InitializePoolEvent {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            signature: convert_string_to_signature(row.signature, "signature")?,
            timestamp: row.timestamp,
            slot: convert_i64_to_u64(row.slot, "slot")?,
            transaction_index: convert_optional(
                row.transaction_index,
                "transaction_index",
                convert_i64_to_u32,
            )?,
            event_index: convert_i32_to_u16(row.event_index, "event_index")?,
            token_a_mint: convert_string_to_pubkey(row.token_a_mint, "token_a_mint")?,
            token_b_mint: convert_string_to_pubkey(row.token_b_mint, "token_b_mint")?,
            creator: convert_string_to_pubkey(row.creator, "creator")?,
            payer: convert_string_to_pubkey(row.payer, "payer")?,
            alpha_vault: convert_string_to_pubkey(row.alpha_vault, "alpha_vault")?,
            sqrt_min_price: convert_bigdecimal_to_u128(row.sqrt_min_price, "sqrt_min_price")?,
            sqrt_max_price: convert_bigdecimal_to_u128(row.sqrt_max_price, "sqrt_max_price")?,
            sqrt_price: convert_bigdecimal_to_u128(row.sqrt_price, "sqrt_price")?,
            liquidity: convert_bigdecimal_to_u128(row.liquidity, "liquidity")?,
            activation_type: convert_i16_to_u8(row.activation_type, "activation_type")?,
            activation_point: convert_i64_to_u64(row.activation_point, "activation_point")?,
            collect_fee_mode: convert_i16_to_u8(row.collect_fee_mode, "collect_fee_mode")?,
            pool_type: convert_i16_to_u8(row.pool_type, "pool_type")?,
            token_a_flag: convert_i16_to_u8(row.token_a_flag, "token_a_flag")?,
            token_b_flag: convert_i16_to_u8(row.token_b_flag, "token_b_flag")?,
            token_a_amount: convert_i64_to_u64(row.token_a_amount, "token_a_amount")?,
            token_b_amount: convert_i64_to_u64(row.token_b_amount, "token_b_amount")?,
            total_amount_a: convert_i64_to_u64(row.total_amount_a, "total_amount_a")?,
            total_amount_b: convert_i64_to_u64(row.total_amount_b, "total_amount_b")?,
            pool_fees_raw: row.pool_fees_raw,
        })
    }
}
