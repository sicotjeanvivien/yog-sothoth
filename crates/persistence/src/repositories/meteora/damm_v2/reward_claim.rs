//! Reward claim events repository: inserts new claims and lists
//! them by pool.
//!
//! Static SQL on the read; the row shape and the mapping to domain
//! live at the bottom of the module.
mod rows;

use crate::repositories::helper::{convert_u64_to_i64, map_sqlx_error};
use async_trait::async_trait;
use rows::MeteoraDammV2ClaimRewardEventRow;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{MeteoraDammV2ClaimRewardEvent, MeteoraDammV2ClaimRewardEventRepository},
};

pub struct PgMeteoraDammV2ClaimRewardEventRepository {
    pool: PgPool,
}

impl PgMeteoraDammV2ClaimRewardEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MeteoraDammV2ClaimRewardEventRepository for PgMeteoraDammV2ClaimRewardEventRepository {
    async fn insert(&self, event: &MeteoraDammV2ClaimRewardEvent) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_claim_reward_events (
                pool_address, signature,
                position, owner,
                mint_reward, reward_index, total_reward,
                timestamp
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (signature, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
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
    ) -> RepositoryResult<Vec<MeteoraDammV2ClaimRewardEvent>> {
        let rows = sqlx::query_as!(
            MeteoraDammV2ClaimRewardEventRow,
            r#"
            SELECT pool_address, signature,
                   position, owner,
                   mint_reward, reward_index, total_reward,
                   timestamp
            FROM meteora_damm_v2_claim_reward_events
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
            .map(MeteoraDammV2ClaimRewardEvent::try_from)
            .collect()
    }
}
