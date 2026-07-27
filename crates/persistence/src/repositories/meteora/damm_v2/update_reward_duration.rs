//! Update-reward-duration events repository: inserts farm slot re-pacings.
//!
//! Write-only — the indexer is the sole consumer today.

use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        MeteoraDammV2UpdateRewardDurationEvent, MeteoraDammV2UpdateRewardDurationEventRepository,
    },
};

use crate::repositories::helper::{convert_u64_to_i64, map_sqlx_error};

pub struct PgMeteoraDammV2UpdateRewardDurationEventRepository {
    pool: PgPool,
}

impl PgMeteoraDammV2UpdateRewardDurationEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MeteoraDammV2UpdateRewardDurationEventRepository
    for PgMeteoraDammV2UpdateRewardDurationEventRepository
{
    async fn insert(&self, event: &MeteoraDammV2UpdateRewardDurationEvent) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_update_reward_duration_events (
                pool_address, signature,
                reward_index, old_reward_duration, new_reward_duration,
                timestamp
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (signature, reward_index, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.signature.to_string(),
            event.reward_index as i16,
            convert_u64_to_i64(event.old_reward_duration, "old_reward_duration")?,
            convert_u64_to_i64(event.new_reward_duration, "new_reward_duration")?,
            event.timestamp,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }
}
