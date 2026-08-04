//! Position fee claim events repository: inserts new claims.
use crate::repositories::helper::{convert_u64_to_i64, map_sqlx_error};
use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        InsertOutcome, MeteoraDammV2ClaimPositionFeeEvent,
        MeteoraDammV2ClaimPositionFeeEventRepository,
    },
};

pub struct PgMeteoraDammV2ClaimPositionFeeEventRepository {
    pool: PgPool,
}

impl PgMeteoraDammV2ClaimPositionFeeEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MeteoraDammV2ClaimPositionFeeEventRepository
    for PgMeteoraDammV2ClaimPositionFeeEventRepository
{
    async fn insert(
        &self,
        event: &MeteoraDammV2ClaimPositionFeeEvent,
    ) -> RepositoryResult<InsertOutcome> {
        let result = sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_claim_position_fee_events (
                pool_address, signature,
                position, owner,
                fee_a_claimed, fee_b_claimed,
                timestamp,
                slot, event_index, transaction_index
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7,
                $8, $9, $10
            )
            ON CONFLICT (signature, event_index, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.signature.to_string(),
            event.position.to_string(),
            event.owner.to_string(),
            convert_u64_to_i64(event.fee_a_claimed, "fee_a_claimed")?,
            convert_u64_to_i64(event.fee_b_claimed, "fee_b_claimed")?,
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
