//! Reward claim events repository: inserts new claims and lists
//! them by pool.
//!
//! Static SQL on the read; the row shape and the mapping to domain
//! live at the bottom of the module.
mod rows;

use crate::repositories::helper::{convert_u64_to_i64, map_sqlx_error};
use async_trait::async_trait;
use rows::ClaimRewardEventRow;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{ClaimRewardEvent, ClaimRewardEventRepository},
};

pub struct PgClaimRewardEventRepository {
    pool: PgPool,
}

impl PgClaimRewardEventRepository {
    pub fn new(pool: PgPool) -> Self {
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
            event.signature.to_string(),
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
        let rows = sqlx::query_as!(
            ClaimRewardEventRow,
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

        rows.into_iter().map(ClaimRewardEvent::try_from).collect()
    }
}
