use async_trait::async_trait;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use std::str::FromStr;
use yog_core::{
    domain::{ClaimPositionFeeEvent, ClaimPositionFeeEventRepository, Protocol},
    RepositoryError, RepositoryResult,
};

use crate::infra::db::{
    convert_i64_to_u64, convert_string_to_pubkey, convert_u64_to_i64,
    repository_utils::map_sqlx_error,
};

pub(crate) struct PgClaimPositionFeeEventRepository {
    pool: PgPool,
}

impl PgClaimPositionFeeEventRepository {
    pub(crate) fn new(pool: PgPool) -> Self {
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
            event.signature,
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
        let rows = sqlx::query!(
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
            .map(|row| {
                Ok(ClaimPositionFeeEvent {
                    pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
                    protocol: Protocol::from_str(&row.protocol).map_err(|e| {
                        RepositoryError::Integrity(format!("invalid protocol: {e}"))
                    })?,
                    signature: row.signature,
                    timestamp: row.timestamp,
                    position: convert_string_to_pubkey(row.position, "position")?,
                    owner: convert_string_to_pubkey(row.owner, "owner")?,
                    fee_a_claimed: convert_i64_to_u64(row.fee_a_claimed, "fee_a_claimed")?,
                    fee_b_claimed: convert_i64_to_u64(row.fee_b_claimed, "fee_b_claimed")?,
                })
            })
            .collect::<RepositoryResult<Vec<_>>>()
    }
}
