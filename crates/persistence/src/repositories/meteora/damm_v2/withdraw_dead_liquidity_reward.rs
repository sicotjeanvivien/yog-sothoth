//! Withdraw-dead-liquidity-reward events repository: inserts the reward share of
//! permanently locked liquidity returned to the funder.
//!
//! Write-only — the indexer is the sole consumer today.

use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        MeteoraDammV2WithdrawDeadLiquidityRewardEvent,
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
    ) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_withdraw_dead_liquidity_reward_events (
                pool_address, signature,
                reward_mint, amount,
                timestamp
            )
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (signature, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.signature.to_string(),
            event.reward_mint.to_string(),
            convert_u64_to_i64(event.amount, "amount")?,
            event.timestamp,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }
}
