//! Withdraw-dead-liquidity-reward events repository: inserts the reward share of
//! permanently locked liquidity returned to the funder.
//!
//! Write-only — the indexer is the sole consumer today.

use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        InsertOutcome, MeteoraDammV2WithdrawDeadLiquidityRewardEvent,
        MeteoraDammV2WithdrawDeadLiquidityRewardEventRepository,
    },
};

use crate::repositories::helper::{convert_u64_to_i64, map_sqlx_error};

pub struct PgMeteoraDammV2WithdrawDeadLiquidityRewardEventRepository {
    pool: PgPool,
}

impl PgMeteoraDammV2WithdrawDeadLiquidityRewardEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MeteoraDammV2WithdrawDeadLiquidityRewardEventRepository
    for PgMeteoraDammV2WithdrawDeadLiquidityRewardEventRepository
{
    async fn insert(
        &self,
        event: &MeteoraDammV2WithdrawDeadLiquidityRewardEvent,
    ) -> RepositoryResult<InsertOutcome> {
        let result = sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_withdraw_dead_liquidity_reward_events (
                pool_address, signature,
                reward_mint, amount,
                timestamp,
                slot, event_index, transaction_index
            )
            VALUES ($1, $2, $3, $4, $5,
                $6, $7, $8
            )
            ON CONFLICT (signature, event_index, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.signature.to_string(),
            event.reward_mint.to_string(),
            convert_u64_to_i64(event.amount, "amount")?,
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
