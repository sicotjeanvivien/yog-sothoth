//! Update-reward-funder events repository: inserts farm funding-right transfers.
//!
//! Write-only — the indexer is the sole consumer today.

use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        MeteoraDammV2UpdateRewardFunderEvent, MeteoraDammV2UpdateRewardFunderEventRepository,
    },
};

use crate::repositories::helper::map_sqlx_error;

pub struct PgMeteoraDammV2UpdateRewardFunderEventRepository {
    pool: PgPool,
}

impl PgMeteoraDammV2UpdateRewardFunderEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MeteoraDammV2UpdateRewardFunderEventRepository
    for PgMeteoraDammV2UpdateRewardFunderEventRepository
{
    async fn insert(&self, event: &MeteoraDammV2UpdateRewardFunderEvent) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_update_reward_funder_events (
                pool_address, signature,
                reward_index, old_funder, new_funder,
                timestamp
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (signature, reward_index, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.signature.to_string(),
            event.reward_index as i16,
            event.old_funder.to_string(),
            event.new_funder.to_string(),
            event.timestamp,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }
}
