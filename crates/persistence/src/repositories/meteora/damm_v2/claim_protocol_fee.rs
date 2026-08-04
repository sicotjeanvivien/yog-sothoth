//! Protocol fee claim events repository: inserts new operator claims.
use crate::repositories::helper::{convert_u64_to_i64, map_sqlx_error};
use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        InsertOutcome, MeteoraDammV2ClaimProtocolFeeEvent,
        MeteoraDammV2ClaimProtocolFeeEventRepository,
    },
};

pub struct PgMeteoraDammV2ClaimProtocolFeeEventRepository {
    pool: PgPool,
}

impl PgMeteoraDammV2ClaimProtocolFeeEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MeteoraDammV2ClaimProtocolFeeEventRepository
    for PgMeteoraDammV2ClaimProtocolFeeEventRepository
{
    async fn insert(
        &self,
        event: &MeteoraDammV2ClaimProtocolFeeEvent,
    ) -> RepositoryResult<InsertOutcome> {
        let result = sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_claim_protocol_fee_events (
                pool_address, signature,
                token_a_amount, token_b_amount,
                timestamp,
                slot, event_index, transaction_index
            )
            VALUES ($1, $2, $3, $4, $5,
                $6, $7, $8
            )
            ON CONFLICT (signature, event_index, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.signature.to_string(),
            convert_u64_to_i64(event.token_a_amount, "token_a_amount")?,
            convert_u64_to_i64(event.token_b_amount, "token_b_amount")?,
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
