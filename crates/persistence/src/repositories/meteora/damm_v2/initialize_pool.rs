//! Initialize-pool (genesis) events repository.
//!
//! Write-only — the indexer is the sole consumer today. The fee parameters
//! are persisted as a raw borsh blob (`pool_fees_raw`), undecoded.

use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{MeteoraDammV2InitializePoolEvent, MeteoraDammV2InitializePoolEventRepository},
};

use crate::repositories::helper::{convert_u64_to_i64, convert_u128_to_bigdecimal, map_sqlx_error};

pub struct PgMeteoraDammV2InitializePoolEventRepository {
    pool: PgPool,
}

impl PgMeteoraDammV2InitializePoolEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MeteoraDammV2InitializePoolEventRepository for PgMeteoraDammV2InitializePoolEventRepository {
    async fn insert(&self, event: &MeteoraDammV2InitializePoolEvent) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_initialize_pool_events (
                pool_address, signature,
                token_a_mint, token_b_mint, creator, payer, alpha_vault,
                sqrt_min_price, sqrt_max_price, sqrt_price, liquidity,
                activation_type, activation_point, collect_fee_mode, pool_type,
                token_a_flag, token_b_flag,
                token_a_amount, token_b_amount, total_amount_a, total_amount_b,
                pool_fees_raw,
                timestamp
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
                $16, $17, $18, $19, $20, $21, $22, $23
            )
            ON CONFLICT (signature, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.signature.to_string(),
            event.token_a_mint.to_string(),
            event.token_b_mint.to_string(),
            event.creator.to_string(),
            event.payer.to_string(),
            event.alpha_vault.to_string(),
            convert_u128_to_bigdecimal(event.sqrt_min_price, "sqrt_min_price"),
            convert_u128_to_bigdecimal(event.sqrt_max_price, "sqrt_max_price"),
            convert_u128_to_bigdecimal(event.sqrt_price, "sqrt_price"),
            convert_u128_to_bigdecimal(event.liquidity, "liquidity"),
            event.activation_type as i16,
            convert_u64_to_i64(event.activation_point, "activation_point")?,
            event.collect_fee_mode as i16,
            event.pool_type as i16,
            event.token_a_flag as i16,
            event.token_b_flag as i16,
            convert_u64_to_i64(event.token_a_amount, "token_a_amount")?,
            convert_u64_to_i64(event.token_b_amount, "token_b_amount")?,
            convert_u64_to_i64(event.total_amount_a, "total_amount_a")?,
            convert_u64_to_i64(event.total_amount_b, "total_amount_b")?,
            event.pool_fees_raw,
            event.timestamp,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }
}
