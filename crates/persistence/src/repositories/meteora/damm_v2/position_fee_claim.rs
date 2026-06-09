//! Position fee claim events repository: inserts new claims and lists
//! them by pool.
//!
//! Static SQL on the read (single query, no traversal mode); the row
//! shape and the mapping to domain live at the bottom of the module.
mod rows;

use crate::repositories::helper::{convert_u64_to_i64, map_sqlx_error};
use async_trait::async_trait;
use rows::MeteoraDammV2ClaimPositionFeeEventRow;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{MeteoraDammV2ClaimPositionFeeEvent, MeteoraDammV2ClaimPositionFeeEventRepository},
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
    async fn insert(&self, event: &MeteoraDammV2ClaimPositionFeeEvent) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_claim_position_fee_events (
                pool_address, signature,
                position, owner,
                fee_a_claimed, fee_b_claimed,
                timestamp
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (signature, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.signature.to_string(),
            event.position.to_string(),
            event.owner.to_string(),
            convert_u64_to_i64(event.fee_a_claimed, "fee_a_claimed")?,
            convert_u64_to_i64(event.fee_b_claimed, "fee_b_claimed")?,
            event.timestamp,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn find_by_pool(
        &self,
        pool_address: &Pubkey,
        limit: i64,
    ) -> RepositoryResult<Vec<MeteoraDammV2ClaimPositionFeeEvent>> {
        let rows = sqlx::query_as!(
            MeteoraDammV2ClaimPositionFeeEventRow,
            r#"
            SELECT pool_address, signature,
                   position, owner,
                   fee_a_claimed, fee_b_claimed,
                   timestamp
            FROM meteora_damm_v2_claim_position_fee_events
            WHERE pool_address = $1
            ORDER BY timestamp DESC
            LIMIT $2
            "#,
            pool_address.to_string(),
            limit,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(MeteoraDammV2ClaimPositionFeeEvent::try_from)
            .collect()
    }
}
