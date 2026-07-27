//! Protocol fee claim events repository: inserts new operator claims.
use crate::repositories::helper::{convert_u64_to_i64, map_sqlx_error};
use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{MeteoraDammV2ClaimProtocolFeeEvent, MeteoraDammV2ClaimProtocolFeeEventRepository},
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
    async fn insert(&self, event: &MeteoraDammV2ClaimProtocolFeeEvent) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_claim_protocol_fee_events (
                pool_address, signature,
                token_a_amount, token_b_amount,
                timestamp
            )
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (signature, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.signature.to_string(),
            convert_u64_to_i64(event.token_a_amount, "token_a_amount")?,
            convert_u64_to_i64(event.token_b_amount, "token_b_amount")?,
            event.timestamp,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }
}
