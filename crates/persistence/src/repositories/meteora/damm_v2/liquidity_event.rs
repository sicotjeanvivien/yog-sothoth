//! Liquidity events repository: inserts new events.

use crate::repositories::helper::{convert_u64_to_i64, convert_u128_to_bigdecimal, map_sqlx_error};
use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{InsertOutcome, MeteoraDammV2LiquidityEvent, MeteoraDammV2LiquidityEventRepository},
};

pub struct PgMeteoraDammV2LiquidityEventRepository {
    pool: PgPool,
}

impl PgMeteoraDammV2LiquidityEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MeteoraDammV2LiquidityEventRepository for PgMeteoraDammV2LiquidityEventRepository {
    async fn insert(&self, event: &MeteoraDammV2LiquidityEvent) -> RepositoryResult<InsertOutcome> {
        let result = sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_liquidity_events (
                pool_address, signature,
                liquidity_event_kind, amount_a, amount_b, liquidity_delta,
                reserve_a_after, reserve_b_after,
                position, owner,
                timestamp,
                slot, event_index, transaction_index
            )
            VALUES (
                $1, $2,
                $3, $4, $5, $6,
                $7, $8,
                $9, $10,
                $11,
                $12, $13, $14
            )
            ON CONFLICT (signature, event_index, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.signature.to_string(),
            event.liquidity_event_kind.as_str(),
            convert_u64_to_i64(event.amount_a, "amount_a")?,
            convert_u64_to_i64(event.amount_b, "amount_b")?,
            convert_u128_to_bigdecimal(event.liquidity_delta, "liquidity_delta"),
            convert_u64_to_i64(event.reserve_a_after, "reserve_a_after")?,
            convert_u64_to_i64(event.reserve_b_after, "reserve_b_after")?,
            event.position.to_string(),
            event.owner.to_string(),
            event.timestamp,
            convert_u64_to_i64(event.slot, "slot")?,
            i32::from(event.event_index),
            event.transaction_index.map(i64::from),
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(InsertOutcome::from_rows_affected(result.rows_affected()))
    }
}
