//! Reward claim events repository: inserts new claims.
use crate::repositories::helper::{convert_u64_to_i64, map_sqlx_error};
use async_trait::async_trait;
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
}
