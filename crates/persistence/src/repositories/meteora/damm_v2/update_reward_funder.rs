//! Update-reward-funder events repository: inserts farm funding-right transfers.
//!
//! Write-only — the indexer is the sole consumer today.

use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        InsertOutcome, MeteoraDammV2UpdateRewardFunderEvent,
        MeteoraDammV2UpdateRewardFunderEventRepository,
    },
};

use crate::repositories::helper::{convert_u64_to_i64, map_sqlx_error};

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
    async fn insert(
        &self,
        event: &MeteoraDammV2UpdateRewardFunderEvent,
    ) -> RepositoryResult<InsertOutcome> {
        let result = sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_update_reward_funder_events (
                pool_address, signature,
                reward_index, old_funder, new_funder,
                timestamp,
                slot, event_index, transaction_index
            )
            VALUES ($1, $2, $3, $4, $5, $6,
                $7, $8, $9
            )
            ON CONFLICT (signature, event_index, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.signature.to_string(),
            event.reward_index as i16,
            event.old_funder.to_string(),
            event.new_funder.to_string(),
            event.timestamp,
            convert_u64_to_i64(event.slot, "slot")?,
            i32::from(event.event_index),
            event.transaction_index.map(i64::from),
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(InsertOutcome::from_rows_affected(result.rows_affected()))
    }
}
