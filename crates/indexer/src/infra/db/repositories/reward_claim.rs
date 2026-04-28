use async_trait::async_trait;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use std::str::FromStr;
use yog_core::{
    domain::{ClaimRewardEvent, ClaimRewardEventRepository, Protocol},
    RepositoryError, RepositoryResult,
};

use crate::infra::db::{
    convert_i64_to_u64, convert_string_to_pubkey, convert_u64_to_i64,
    repository_utils::map_sqlx_error,
};

pub(crate) struct PgClaimRewardEventRepository {
    pool: PgPool,
}

impl PgClaimRewardEventRepository {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ClaimRewardEventRepository for PgClaimRewardEventRepository {
    async fn insert(&self, event: &ClaimRewardEvent) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO reward_claims (
                pool_address, protocol, signature,
                position, owner,
                mint_reward, reward_index, total_reward,
                timestamp
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (signature, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.protocol.as_str(),
            event.signature,
            event.position.to_string(),
            event.owner.to_string(),
            event.mint_reward.to_string(),
            event.reward_index as i16,
            convert_u64_to_i64(event.total_reward, "total_reward")?,
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
    ) -> RepositoryResult<Vec<ClaimRewardEvent>> {
        let rows = sqlx::query!(
            r#"
            SELECT pool_address, protocol, signature,
                   position, owner,
                   mint_reward, reward_index, total_reward,
                   timestamp
            FROM reward_claims
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
                let reward_index: u8 = u8::try_from(row.reward_index).map_err(|_| {
                    RepositoryError::Integrity(format!(
                        "invalid reward_index: {}",
                        row.reward_index
                    ))
                })?;
                Ok(ClaimRewardEvent {
                    pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
                    protocol: Protocol::from_str(&row.protocol).map_err(|e| {
                        RepositoryError::Integrity(format!("invalid protocol: {e}"))
                    })?,
                    signature: row.signature,
                    timestamp: row.timestamp,
                    position: convert_string_to_pubkey(row.position, "position")?,
                    owner: convert_string_to_pubkey(row.owner, "owner")?,
                    mint_reward: convert_string_to_pubkey(row.mint_reward, "mint_reward")?,
                    reward_index,
                    total_reward: convert_i64_to_u64(row.total_reward, "total_reward")?,
                })
            })
            .collect::<RepositoryResult<Vec<_>>>()
    }
}