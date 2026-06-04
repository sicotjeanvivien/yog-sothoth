//! Position fee claim events repository: inserts new claims and lists
//! them by pool.
//!
//! Static SQL on the read (single query, no traversal mode); the row
//! shape and the mapping to domain live at the bottom of the module.
mod rows;

use crate::repositories::helper::{convert_u64_to_i64, map_sqlx_error};
use async_trait::async_trait;
use rows::ClaimPositionFeeEventRow;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{ClaimPositionFeeEvent, ClaimPositionFeeEventRepository},
};

pub struct PgClaimPositionFeeEventRepository {
    pool: PgPool,
}

impl PgClaimPositionFeeEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ClaimPositionFeeEventRepository for PgClaimPositionFeeEventRepository {
    async fn insert(&self, event: &ClaimPositionFeeEvent) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO position_fee_claims (
                pool_address, protocol, signature,
                position, owner,
                fee_a_claimed, fee_b_claimed,
                timestamp
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (signature, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.protocol.as_str(),
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
    ) -> RepositoryResult<Vec<ClaimPositionFeeEvent>> {
        let rows = sqlx::query_as!(
            ClaimPositionFeeEventRow,
            r#"
            SELECT pool_address, protocol, signature,
                   position, owner,
                   fee_a_claimed, fee_b_claimed,
                   timestamp
            FROM position_fee_claims
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
            .map(ClaimPositionFeeEvent::try_from)
            .collect()
    }
}
