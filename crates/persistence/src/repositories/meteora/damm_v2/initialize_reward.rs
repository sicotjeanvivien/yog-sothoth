//! Initialize-reward events repository: inserts newly opened farm reward slots.
//!
//! Write-only — the indexer is the sole consumer today.

use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{MeteoraDammV2InitializeRewardEvent, MeteoraDammV2InitializeRewardEventRepository},
};

use crate::repositories::helper::{convert_u64_to_i64, map_sqlx_error};

pub struct PgMeteoraDammV2InitializeRewardEventRepository {
    pool: PgPool,
}

impl PgMeteoraDammV2InitializeRewardEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MeteoraDammV2InitializeRewardEventRepository
    for PgMeteoraDammV2InitializeRewardEventRepository
{
    async fn insert(&self, event: &MeteoraDammV2InitializeRewardEvent) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_initialize_reward_events (
                pool_address, signature,
                reward_mint, funder, creator, reward_index, reward_duration,
                timestamp
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (signature, reward_index, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.signature.to_string(),
            event.reward_mint.to_string(),
            event.funder.to_string(),
            event.creator.to_string(),
            event.reward_index as i16,
            convert_u64_to_i64(event.reward_duration, "reward_duration")?,
            event.timestamp,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }
}
